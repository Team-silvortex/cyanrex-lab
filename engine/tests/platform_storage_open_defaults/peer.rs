//! A real PostgreSQL startup frame reaches an owned TCP peer, but no password/authentication,
//! catalog data, real database or successful retained connector is supplied by these tests.
use super::{
    environment::run_case,
    fixture::{config, endpoint, expected, no_dial, Root, PASSWORD},
};
use cyanrex_engine::platform_host::{
    open_platform_storage_expected, PlatformStorageConnectionError,
};
use std::{
    ffi::CString,
    fs::File,
    io::{self, Read},
    os::{fd::FromRawFd, unix::ffi::OsStrExt},
    path::Path,
    time::{Duration, Instant},
};
use tokio::{
    io::AsyncReadExt,
    net::{TcpListener, TcpStream},
};

struct RootOpens(File);
impl RootOpens {
    fn new(path: &Path) -> Self {
        // SAFETY: the owned fd is transferred once; only this private fixture path is watched.
        let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        assert!(fd >= 0);
        let file = unsafe { File::from_raw_fd(fd) };
        let path = CString::new(path.as_os_str().as_bytes()).unwrap();
        assert!(unsafe { libc::inotify_add_watch(fd, path.as_ptr(), libc::IN_OPEN) } >= 0);
        Self(file)
    }
    fn observed(&mut self) -> bool {
        let mut bytes = [0; 8192];
        match self.0.read(&mut bytes) {
            Ok(length) => length != 0,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => false,
            Err(_) => panic!("owned root-open observer unavailable"),
        }
    }
}

fn attempt(
    root: &Root,
    listener: &TcpListener,
) -> impl std::future::Future<Output = Result<(), PlatformStorageConnectionError>> + Send + 'static
{
    let config = config(0);
    let expected = expected(config.clone());
    let endpoint = endpoint(listener);
    // Capture before installing the watch, so this observation cannot fake the factory precheck.
    let original = root.original(&config);
    let path = root.path().to_owned();
    async move {
        open_platform_storage_expected(config, endpoint, PASSWORD, expected, &path, original)
            .await
            .map(|_| ())
    }
}
async fn startup(listener: &TcpListener) -> Option<(TcpStream, bool)> {
    tokio::time::timeout(Duration::from_secs(3), async {
        let (mut peer, _) = listener.accept().await.ok()?;
        let mut header = [0; 4];
        peer.read_exact(&mut header).await.ok()?;
        let length = u32::from_be_bytes(header) as usize;
        if !(8..=4096).contains(&length) {
            return None;
        }
        let mut payload = vec![0; length - 4];
        peer.read_exact(&mut payload).await.ok()?;
        let contains = |value: &[u8]| payload.windows(value.len()).any(|window| window == value);
        let explicit = payload.starts_with(&196608u32.to_be_bytes())
            && contains(b"user\0platform_runtime\0")
            && contains(b"database\0platform_db\0")
            && contains(b"search_path=source_private")
            && !contains(PASSWORD.as_bytes())
            && !contains(b"untrusted_");
        Some((peer, explicit))
    })
    .await
    .ok()
    .flatten()
}
async fn closed(peer: &mut TcpStream) -> bool {
    match tokio::time::timeout(Duration::from_secs(2), peer.read(&mut [0; 1])).await {
        Ok(Ok(0)) => true,
        Ok(Err(error)) => matches!(
            error.kind(),
            io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted
        ),
        _ => false,
    }
}

pub(super) async fn exercise(case: &str, base: &Path) {
    let root = Root::new_in(base);
    let listener = super::fixture::listener().await;
    let future = attempt(&root, &listener);
    let mut opens = RootOpens::new(root.path());
    let started = Instant::now();
    let mut pending = tokio::spawn(future);
    let peer = startup(&listener).await;
    let reached = peer.is_some();
    let explicit = peer.as_ref().is_some_and(|(_, explicit)| *explicit);
    let precheck = opens.observed();
    let mut peer = peer.map(|(socket, _)| socket);
    let mut cancelled = false;
    let mut failed = false;
    let mut postcheck = false;
    let mut socket_closed = true;
    match case {
        "peer-close" => {
            drop(peer.take());
            failed = matches!(
                tokio::time::timeout(Duration::from_secs(2), &mut pending).await,
                Ok(Ok(Err(PlatformStorageConnectionError::Unavailable)))
            );
            postcheck = opens.observed();
        }
        "peer-swap" => {
            root.swap();
            let mut replacement = RootOpens::new(root.path());
            drop(peer.take());
            failed = matches!(
                tokio::time::timeout(Duration::from_secs(2), &mut pending).await,
                Ok(Ok(Err(PlatformStorageConnectionError::Unavailable)))
            );
            postcheck = replacement.observed();
            drop(replacement);
            root.restore();
        }
        "peer-cancel" => {
            pending.abort();
            cancelled = matches!((&mut pending).await,Err(error)if error.is_cancelled());
            if let Some(socket) = peer.as_mut() {
                socket_closed = closed(socket).await;
            } else {
                socket_closed = false;
            }
        }
        "peer-timeout" => {
            failed = matches!(
                tokio::time::timeout(Duration::from_secs(13), &mut pending).await,
                Ok(Ok(Err(PlatformStorageConnectionError::Unavailable)))
            );
            postcheck = opens.observed();
            if let Some(socket) = peer.as_mut() {
                socket_closed = closed(socket).await;
            } else {
                socket_closed = false;
            }
        }
        _ => panic!("unknown synthetic peer case"),
    }
    let elapsed = started.elapsed();
    if !pending.is_finished() {
        pending.abort();
        let _ = pending.await;
    }
    let no_retry = no_dial(&listener).await;
    drop(peer);
    drop(listener);
    drop(opens);
    let cleaned = root.finish();
    assert!(reached && explicit && precheck && no_retry && cleaned);
    if case == "peer-cancel" {
        // A dropped future proves local socket closure, not mandatory root-postcheck or remote rollback.
        assert!(cancelled && socket_closed);
    } else {
        assert!(failed && postcheck && socket_closed);
    }
    if case == "peer-timeout" {
        assert!(elapsed >= Duration::from_secs(9) && elapsed < Duration::from_secs(14));
    }
}

#[test]
fn platform_storage_open_defaults_peer_failure_rechecks_root_after_actual_startup() {
    assert!(run_case("peer-close"));
}
#[test]
fn platform_storage_open_defaults_peer_failure_still_observes_a_replacement_root() {
    assert!(run_case("peer-swap"));
}
#[test]
fn platform_storage_open_defaults_cancelled_handshake_closes_socket_without_retry() {
    assert!(run_case("peer-cancel"));
}
#[test]
fn platform_storage_open_defaults_stalled_handshake_uses_the_total_deadline_then_closes() {
    assert!(run_case("peer-timeout"));
}
