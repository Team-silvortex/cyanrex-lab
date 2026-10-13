//! Disposable TLS transport delegates every controlled request to ONE actual Source router.
use super::{
    bytes,
    native_io::{self, Control},
    request, Certificates,
};
use axum::{
    body::{Body, HttpBody},
    http::{HeaderMap, Method, Request, StatusCode, Uri},
    response::Response,
    Router,
};
use http_body_util::BodyExt;
use std::{
    collections::BTreeMap,
    future::Future,
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    os::fd::AsRawFd,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tokio::runtime::Handle;
use tower::ServiceExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Manifest,
    Chunk(u32),
}
#[derive(Clone, Copy)]
pub enum HoldAt {
    BeforeDispatch,
    Headers,
    BodyPrefix(usize),
}
struct Gate {
    state: Mutex<(bool, bool)>,
    changed: Condvar,
}
impl Gate {
    fn release(&self) {
        self.state.lock().unwrap().1 = true;
        self.changed.notify_all();
    }
    fn wait(&self, control: &Control) -> io::Result<()> {
        if control.stopped() {
            return Err(native_io::interrupted());
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut state = self.state.lock().unwrap();
        state.0 = true;
        self.changed.notify_all();
        while !state.1 && !control.stopped() {
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "bounded Source TLS hold",
                ));
            }
            state = self
                .changed
                .wait_timeout(state, Duration::from_millis(50))
                .unwrap()
                .0;
        }
        if control.stopped() {
            return Err(native_io::interrupted());
        }
        Ok(())
    }
}
pub struct Hold(Arc<Gate>);
impl Hold {
    pub fn release(&self) {
        self.0.release();
    }
    pub async fn wait_reached(&self) {
        tokio::time::timeout(Duration::from_secs(10), async {
            while !self.0.state.lock().unwrap().0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("actual Source TLS phase reaches its hold");
    }
}
impl Drop for Hold {
    fn drop(&mut self) {
        self.release();
    }
}

pub struct ObservedRequest {
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
    pub staged_before_request: Vec<bytes::Entry>,
    pub kind: Kind,
}
pub struct TlsObservation {
    pub received_bytes: usize,
    pub dispatched: usize,
    pub responded: usize,
    pub error: Option<String>,
}
struct State {
    router: Router,
    runtime: Handle,
    control: Control,
    root: Mutex<PathBuf>,
    requests: Mutex<Vec<ObservedRequest>>,
    statuses: Mutex<Vec<StatusCode>>,
    holds: Mutex<BTreeMap<Kind, (HoldAt, Arc<Gate>)>>,
    all_gates: Mutex<Vec<Arc<Gate>>>,
    dispatched: AtomicUsize,
    responded: AtomicUsize,
}
pub struct SourceTlsServer {
    pub origin: String,
    state: Arc<State>,
    process: Mutex<Option<Child>>,
    worker: Mutex<Option<JoinHandle<io::Result<()>>>>,
    error: Mutex<Option<String>>,
}
fn bounded<T, F: Future<Output = io::Result<T>>>(state: &State, future: F) -> io::Result<T> {
    let stopped = state.control.subscribe();
    state.runtime.block_on(async {
        tokio::select! {
            biased;
            _ = native_io::until_stopped(stopped) => Err(native_io::interrupted()),
            result = tokio::time::timeout(Duration::from_secs(20), future) => {
                result.map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "bounded Source TLS async work"))?
            },
        }
    })
}
fn send_response<W: Write + AsRawFd>(
    state: &State,
    outgoing: &mut W,
    response: Response,
    hold: Option<(HoldAt, Arc<Gate>)>,
) -> io::Result<()> {
    let (mut parts, mut body) = response.into_parts();
    if let Some((HoldAt::Headers, gate)) = &hold {
        gate.wait(&state.control)?;
    }
    let length = if let Some(value) = parts.headers.get("content-length") {
        value
            .to_str()
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
    } else {
        // Normal HTTP framing only, just as Hyper uses an exact body hint. Never skip real EOF.
        let length = body.size_hint().exact();
        if let Some(length) = length {
            parts
                .headers
                .insert("content-length", length.to_string().parse().unwrap());
        }
        length
    }
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Source TLS needs exact response framing",
        )
    })?;
    let start = format!(
        "HTTP/1.1 {} {}\r\n",
        parts.status.as_u16(),
        parts.status.canonical_reason().unwrap_or("Source")
    );
    native_io::write_all(outgoing, start.as_bytes(), &state.control)?;
    for (name, value) in &parts.headers {
        if name.as_str() == "connection" {
            continue;
        }
        native_io::write_all(outgoing, name.as_str().as_bytes(), &state.control)?;
        native_io::write_all(outgoing, b": ", &state.control)?;
        native_io::write_all(outgoing, value.as_bytes(), &state.control)?;
        native_io::write_all(outgoing, b"\r\n", &state.control)?;
    }
    native_io::write_all(outgoing, b"connection: close\r\n\r\n", &state.control)?;
    let mut prefix = match &hold {
        Some((HoldAt::BodyPrefix(length), _)) => Some(*length),
        _ => None,
    };
    if prefix == Some(0) {
        if let Some((HoldAt::BodyPrefix(_), gate)) = &hold {
            gate.wait(&state.control)?;
        }
        prefix = None;
    }
    let mut written = 0u64;
    loop {
        let frame = bounded(state, async {
            body.frame().await.transpose().map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "actual Source response body failed",
                )
            })
        })?;
        let Some(frame) = frame else {
            break;
        };
        let data = frame.into_data().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected Source response trailer",
            )
        })?;
        written = written
            .checked_add(data.len() as u64)
            .filter(|total| *total <= length)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Source response exceeds its length",
                )
            })?;
        if let Some(remaining) = prefix.as_mut() {
            let amount = (*remaining).min(data.len());
            native_io::write_all(outgoing, &data[..amount], &state.control)?;
            *remaining -= amount;
            if *remaining == 0 {
                if let Some((HoldAt::BodyPrefix(_), gate)) = &hold {
                    gate.wait(&state.control)?;
                }
                prefix = None;
            }
            native_io::write_all(outgoing, &data[amount..], &state.control)?;
        } else {
            native_io::write_all(outgoing, &data, &state.control)?;
        }
        // Keep this original owner-retaining Binary Bytes live across ALL writes and a hold.
        drop(data);
    }
    if written != length || prefix.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Source response ended before its exact length or hold",
        ));
    }
    state.responded.fetch_add(1, Ordering::SeqCst);
    Ok(())
}
fn serve<R: Read + AsRawFd, W: Write + AsRawFd>(
    mut incoming: R,
    mut outgoing: W,
    state: Arc<State>,
) -> io::Result<()> {
    let mut pending = Vec::new();
    loop {
        let mut observed = request::read(&mut incoming, &mut pending, &state.control)?;
        // Capture the selected root AFTER the request arrives, not before blocking for it.
        observed.staged_before_request = bytes::tree(&state.root.lock().unwrap());
        let hold = state.holds.lock().unwrap().get(&observed.kind).cloned();
        let mut request = Request::new(Body::from(observed.body.clone()));
        *request.method_mut() = observed.method.clone();
        *request.uri_mut() = observed.uri.clone();
        *request.headers_mut() = observed.headers.clone();
        state.requests.lock().unwrap().push(observed);
        if let Some((HoldAt::BeforeDispatch, gate)) = &hold {
            gate.wait(&state.control)?;
        }
        let router = state.router.clone();
        let response = bounded(&state, async {
            if state.control.stopped() {
                return Err(native_io::interrupted());
            }
            state.dispatched.fetch_add(1, Ordering::SeqCst);
            router
                .oneshot(request)
                .await
                .map_err(|_| io::Error::other("Source router dispatch failed"))
        })?;
        state.statuses.lock().unwrap().push(response.status());
        send_response(&state, &mut outgoing, response, hold)?;
    }
}
impl SourceTlsServer {
    pub fn start(certificates: &Certificates, router: Router, root: PathBuf, ip: Ipv4Addr) -> Self {
        let reserved = TcpListener::bind(SocketAddr::from((ip, 0))).unwrap();
        let address = reserved.local_addr().unwrap();
        drop(reserved);
        let process = Command::new("openssl")
            .args(["s_server", "-quiet", "-ign_eof", "-accept"])
            .arg(address.to_string())
            .arg("-cert")
            .arg(&certificates.certificate)
            .arg("-key")
            .arg(&certificates.key)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start disposable Source TLS server");
        // Own the child BEFORE any fallible pipe/flags/thread/readiness step.
        let server = Self {
            origin: format!("https://{address}"),
            state: Arc::new(State {
                router,
                runtime: Handle::current(),
                control: Control::new(),
                root: Mutex::new(root),
                requests: Mutex::new(Vec::new()),
                statuses: Mutex::new(Vec::new()),
                holds: Mutex::new(BTreeMap::new()),
                all_gates: Mutex::new(Vec::new()),
                dispatched: AtomicUsize::new(0),
                responded: AtomicUsize::new(0),
            }),
            process: Mutex::new(Some(process)),
            worker: Mutex::new(None),
            error: Mutex::new(None),
        };
        let (incoming, outgoing) = {
            let mut process = server.process.lock().unwrap();
            let process = process.as_mut().unwrap();
            (
                process.stdout.take().unwrap(),
                process.stdin.take().unwrap(),
            )
        };
        native_io::nonblocking(&incoming).unwrap();
        native_io::nonblocking(&outgoing).unwrap();
        let state = Arc::clone(&server.state);
        *server.worker.lock().unwrap() =
            Some(thread::spawn(move || serve(incoming, outgoing, state)));
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(
                server
                    .process
                    .lock()
                    .unwrap()
                    .as_mut()
                    .unwrap()
                    .try_wait()
                    .unwrap()
                    .is_none(),
                "Source TLS listener remains alive"
            );
            if TcpStream::connect_timeout(&address, Duration::from_millis(50)).is_ok() {
                break;
            }
            assert!(Instant::now() < deadline, "bounded Source TLS readiness");
            thread::sleep(Duration::from_millis(10));
        }
        server
    }
    pub fn hold(&self, kind: Kind, at: HoldAt) -> Hold {
        let gate = Arc::new(Gate {
            state: Mutex::new((false, false)),
            changed: Condvar::new(),
        });
        self.state.all_gates.lock().unwrap().push(Arc::clone(&gate));
        self.state
            .holds
            .lock()
            .unwrap()
            .insert(kind, (at, Arc::clone(&gate)));
        Hold(gate)
    }
    pub fn set_root(&self, root: PathBuf) {
        *self.state.root.lock().unwrap() = root;
    }
    pub fn take_requests(&self) -> Vec<ObservedRequest> {
        std::mem::take(&mut *self.state.requests.lock().unwrap())
    }
    pub fn take_statuses(&self) -> Vec<StatusCode> {
        std::mem::take(&mut *self.state.statuses.lock().unwrap())
    }
    pub fn received_bytes(&self) -> usize {
        self.state.control.received()
    }
    pub async fn wait_responded(&self, expected: usize) {
        // Client Content-Length completion may precede this worker's real Body EOF poll.
        // Never turn fixture shutdown into a false incomplete-response observation.
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let actual = self.state.responded.load(Ordering::SeqCst);
                assert!(actual <= expected, "unexpected extra Source TLS response");
                if actual == expected {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("actual Source TLS responses reach complete body EOF before shutdown");
    }
    pub fn finish(&self) -> TlsObservation {
        self.state.control.stop();
        for gate in self.state.all_gates.lock().unwrap().iter() {
            gate.release();
        }
        if let Some(mut process) = self.process.lock().unwrap().take() {
            let _ = process.kill();
            process
                .wait()
                .expect("reap the fixture's own Source TLS process");
        }
        if let Some(worker) = self.worker.lock().unwrap().take() {
            match worker.join() {
                Ok(Err(error)) if error.kind() != io::ErrorKind::Interrupted => {
                    *self.error.lock().unwrap() = Some(error.to_string());
                }
                Err(_) => {
                    *self.error.lock().unwrap() = Some("Source TLS responder panicked".to_owned());
                }
                _ => {}
            }
        }
        TlsObservation {
            received_bytes: self.received_bytes(),
            dispatched: self.state.dispatched.load(Ordering::SeqCst),
            responded: self.state.responded.load(Ordering::SeqCst),
            error: self.error.lock().unwrap().clone(),
        }
    }
}
impl Drop for SourceTlsServer {
    fn drop(&mut self) {
        self.finish();
    }
}
