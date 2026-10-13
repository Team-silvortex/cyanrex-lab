//! Raw HTTP is exchanged only through native OpenSSL's decrypted stdin/stdout pipes.
use crate::{
    bytes,
    certificates::Certificates,
    fixture::{ManifestQuery, ObservedRequest, Selected},
    legacy_fixture::RequestQuery,
    reply::{Kind, Reply},
    FileTransferManifest,
};
use axum::{
    extract::Query,
    http::{HeaderMap, Method, Uri},
};
use std::{
    collections::BTreeMap,
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    os::fd::AsRawFd,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
pub enum HoldAt {
    Headers,
    Body(usize),
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
    fn wait(&self, stop: &AtomicBool) -> io::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut state = self.state.lock().unwrap();
        state.0 = true;
        self.changed.notify_all();
        while !state.1 && !stop.load(Ordering::SeqCst) {
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "bounded native TLS hold",
                ));
            }
            state = self
                .changed
                .wait_timeout(state, Duration::from_millis(50))
                .unwrap()
                .0;
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
        tokio::time::timeout(Duration::from_secs(8), async {
            while !self.0.state.lock().unwrap().0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("actual decrypted response must reach its held boundary");
    }
}
impl Drop for Hold {
    fn drop(&mut self) {
        self.release();
    }
}

struct State {
    manifest: FileTransferManifest,
    root: PathBuf,
    requests: Mutex<Vec<ObservedRequest>>,
    replies: Mutex<BTreeMap<Kind, Reply>>,
    holds: Mutex<BTreeMap<Kind, (HoldAt, Arc<Gate>)>>,
    all_gates: Mutex<Vec<Arc<Gate>>>,
    received: AtomicUsize,
    responded: AtomicUsize,
    stop: AtomicBool,
}
#[derive(Debug)]
pub struct TlsObservation {
    pub received_bytes: usize,
    pub responded: usize,
    pub error: Option<String>,
}
pub struct TlsServer {
    pub origin: String,
    state: Arc<State>,
    process: Mutex<Option<Child>>,
    worker: Mutex<Option<JoinHandle<io::Result<()>>>>,
    error: Mutex<Option<String>>,
}
fn nonblocking<F: AsRawFd>(fd: &F) -> io::Result<()> {
    let fd = fd.as_raw_fd();
    // Only this owned test pipe is changed; every IO loop also checks the stop bit and a deadline.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
fn ready(fd: i32, events: i16, stop: &AtomicBool, deadline: Instant) -> io::Result<()> {
    if stop.load(Ordering::SeqCst) {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "native TLS server stopping",
        ));
    }
    if Instant::now() >= deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "bounded native TLS pipe IO",
        ));
    }
    let mut descriptor = libc::pollfd {
        fd,
        events,
        revents: 0,
    };
    let result = unsafe { libc::poll(&mut descriptor, 1, 50) };
    if stop.load(Ordering::SeqCst) {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "native TLS server stopping",
        ));
    }
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
fn write_bytes<W: Write + AsRawFd>(
    writer: &mut W,
    mut data: &[u8],
    state: &State,
) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !data.is_empty() {
        ready(writer.as_raw_fd(), libc::POLLOUT, &state.stop, deadline)?;
        match writer.write(data) {
            _ if state.stop.load(Ordering::SeqCst) => {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "native TLS server stopping",
                ));
            }
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "native TLS pipe closed",
                ))
            }
            Ok(length) => data = &data[length..],
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error),
        }
    }
    writer.flush()
}
fn read_request<R: Read + AsRawFd>(
    reader: &mut R,
    state: &State,
    pending: &mut Vec<u8>,
) -> io::Result<ObservedRequest> {
    let deadline = Instant::now() + Duration::from_secs(40);
    let end = loop {
        if let Some(offset) = pending.windows(4).position(|part| part == b"\r\n\r\n") {
            break offset + 4;
        }
        if pending.len() > 8192 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "oversized native test request",
            ));
        }
        read_more(reader, state, pending, deadline)?;
    };
    let text = std::str::from_utf8(&pending[..end])
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "non-UTF8 HTTP test headers"))?;
    let mut lines = text.split("\r\n");
    let mut start = lines.next().unwrap().split_whitespace();
    let method = start
        .next()
        .unwrap_or("")
        .parse::<Method>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid test method"))?;
    let uri = start
        .next()
        .unwrap_or("")
        .parse::<Uri>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid test URI"))?;
    if start.next() != Some("HTTP/1.1") || start.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "test requires real HTTP/1.1",
        ));
    }
    let mut headers = HeaderMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid test header"))?;
        headers.append(
            name.parse::<axum::http::HeaderName>()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid header name"))?,
            value
                .trim()
                .parse()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid header value"))?,
        );
    }
    let length = headers
        .get("content-length")
        .map(|value| value.to_str().unwrap().parse::<usize>())
        .transpose()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid test body length"))?
        .unwrap_or(0);
    if end + length > 8192 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "oversized test request body",
        ));
    }
    while pending.len() < end + length {
        read_more(reader, state, pending, deadline)?;
    }
    let body = pending[end..end + length].to_vec();
    pending.drain(..end + length);
    let selected = match uri.path() {
        "/platform/v1/file-manifests" => Query::<ManifestQuery>::try_from_uri(&uri)
            .map(|query| Selected::Manifest(query.0))
            .map_err(|error| error.to_string()),
        "/platform/v1/file-chunks" => Query::<RequestQuery>::try_from_uri(&uri)
            .map(|query| Selected::Chunk(query.0))
            .map_err(|error| error.to_string()),
        _ => Err("unexpected controlled TLS endpoint".to_owned()),
    };
    Ok(ObservedRequest {
        method,
        uri,
        headers,
        body,
        selected,
        staged_before_request: bytes::tree(&state.root),
    })
}
fn read_more<R: Read + AsRawFd>(
    reader: &mut R,
    state: &State,
    pending: &mut Vec<u8>,
    deadline: Instant,
) -> io::Result<()> {
    ready(reader.as_raw_fd(), libc::POLLIN, &state.stop, deadline)?;
    let mut buffer = [0; 1024];
    let read = reader.read(&mut buffer);
    if let Ok(length) = read.as_ref() {
        // Count every actual decrypted byte independently of typed parsing and stop selection.
        state.received.fetch_add(*length, Ordering::SeqCst);
    }
    match read {
        _ if state.stop.load(Ordering::SeqCst) => Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "native TLS server stopping",
        )),
        Ok(0) => Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "native TLS pipe EOF",
        )),
        Ok(length) => {
            pending.extend_from_slice(&buffer[..length]);
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(()),
        Err(error) => Err(error),
    }
}
fn serve<R: Read + AsRawFd, W: Write + AsRawFd>(
    mut incoming: R,
    mut outgoing: W,
    state: Arc<State>,
) -> io::Result<()> {
    let mut pending = Vec::new();
    loop {
        let request = read_request(&mut incoming, &state, &mut pending)?;
        let kind = match request.selected.as_ref() {
            Ok(Selected::Manifest(_)) => Kind::Manifest,
            Ok(Selected::Chunk(query)) => Kind::Chunk(query.chunk_index),
            Err(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unexpected native request",
                ))
            }
        };
        let reply = state
            .replies
            .lock()
            .unwrap()
            .get(&kind)
            .cloned()
            .unwrap_or_else(|| Reply::good(&state.manifest, kind));
        let hold = state.holds.lock().unwrap().get(&kind).cloned();
        state.requests.lock().unwrap().push(request);
        if let Some((HoldAt::Headers, gate)) = &hold {
            gate.wait(&state.stop)?;
        }
        let mut head = format!(
            "HTTP/1.1 {} {}\r\n",
            reply.status.as_u16(),
            reply.status.canonical_reason().unwrap_or("Controlled")
        );
        for (name, value) in &reply.headers {
            head.push_str(&format!("{name}: {}\r\n", value.to_str().unwrap()));
        }
        head.push_str("connection: close\r\n\r\n");
        write_bytes(&mut outgoing, head.as_bytes(), &state)?;
        let body: Vec<u8> = reply
            .pieces
            .iter()
            .flat_map(|piece| piece.data.iter().copied())
            .collect();
        if let Some((HoldAt::Body(prefix), gate)) = hold {
            let prefix = prefix.min(body.len());
            write_bytes(&mut outgoing, &body[..prefix], &state)?;
            gate.wait(&state.stop)?;
            write_bytes(&mut outgoing, &body[prefix..], &state)?;
        } else {
            write_bytes(&mut outgoing, &body, &state)?;
        }
        state.responded.fetch_add(1, Ordering::SeqCst);
    }
}
impl TlsServer {
    pub fn start(
        certificates: &Certificates,
        manifest: FileTransferManifest,
        root: PathBuf,
        ip: Ipv4Addr,
    ) -> Self {
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
            .expect("start explicit native multi-request TLS server");
        let state = Arc::new(State {
            manifest,
            root,
            requests: Mutex::new(Vec::new()),
            replies: Mutex::new(BTreeMap::new()),
            holds: Mutex::new(BTreeMap::new()),
            all_gates: Mutex::new(Vec::new()),
            received: AtomicUsize::new(0),
            responded: AtomicUsize::new(0),
            stop: AtomicBool::new(false),
        });
        // Establish RAII ownership before any fallible pipe or readiness setup.
        let server = Self {
            origin: format!("https://{address}"),
            state,
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
        nonblocking(&incoming).unwrap();
        nonblocking(&outgoing).unwrap();
        let state = server.state.clone();
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
                "native TLS listener process must remain alive"
            );
            if TcpStream::connect_timeout(&address, Duration::from_millis(50)).is_ok() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "bounded native TLS listener readiness"
            );
            thread::sleep(Duration::from_millis(10));
        }
        server
    }
    pub fn hold(&self, kind: Kind, at: HoldAt) -> Hold {
        let gate = Arc::new(Gate {
            state: Mutex::new((false, false)),
            changed: Condvar::new(),
        });
        self.state.all_gates.lock().unwrap().push(gate.clone());
        self.state
            .holds
            .lock()
            .unwrap()
            .insert(kind, (at, gate.clone()));
        Hold(gate)
    }
    pub fn set_reply(&self, kind: Kind, reply: Reply) {
        assert!(reply.header_gate.is_none());
        assert!(
            reply
                .pieces
                .iter()
                .all(|piece| piece.gate.is_none() && !piece.fail && piece.delay.is_zero()),
            "native server uses its own real pipe Hold, not an async body mock"
        );
        self.state.replies.lock().unwrap().insert(kind, reply);
    }
    pub fn take_requests(&self) -> Vec<ObservedRequest> {
        std::mem::take(&mut *self.state.requests.lock().unwrap())
    }
    pub fn received_bytes(&self) -> usize {
        self.state.received.load(Ordering::SeqCst)
    }
    pub fn responded(&self) -> usize {
        self.state.responded.load(Ordering::SeqCst)
    }
    pub fn finish(&self) -> TlsObservation {
        self.state.stop.store(true, Ordering::SeqCst);
        for gate in self.state.all_gates.lock().unwrap().iter() {
            gate.release();
        }
        if let Some(mut process) = self.process.lock().unwrap().take() {
            let _ = process.kill();
            process
                .wait()
                .expect("reap this fixture's own stopped OpenSSL process");
        }
        if let Some(worker) = self.worker.lock().unwrap().take() {
            // No blocking pipe IO or indefinite Condvar wait exists in this owned thread.
            match worker.join() {
                Ok(Err(error)) if error.kind() != io::ErrorKind::Interrupted => {
                    *self.error.lock().unwrap() = Some(error.to_string());
                }
                Err(_) => {
                    *self.error.lock().unwrap() = Some("native TLS responder panicked".to_owned())
                }
                _ => {}
            }
        }
        TlsObservation {
            received_bytes: self.received_bytes(),
            responded: self.responded(),
            error: self.error.lock().unwrap().clone(),
        }
    }
}
impl Drop for TlsServer {
    fn drop(&mut self) {
        self.finish();
    }
}
