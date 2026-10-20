use crate::serve_loopback::socket_ownership::SocketOwnership;
use axum::{routing::get, Router};
use std::{
    io,
    net::SocketAddr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinHandle,
};

pub(super) const CANARY: &str = "synthetic-wire-secret-never-echo-84ac";
pub(super) struct Server {
    pub(super) address: SocketAddr,
    pub(super) authority: String,
    ownership: SocketOwnership,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), ()>>>,
}
impl Server {
    pub(super) async fn start(router: Router) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let ownership = SocketOwnership::capture(&listener).unwrap();
        assert!(ownership.is_open_in_this_process().unwrap());
        let address = listener.local_addr().unwrap();
        let authority = address.to_string();
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(super::super::serve(
            listener,
            router,
            authority.clone(),
            async {
                let _ = stopped.await;
            },
        ));
        Self {
            address,
            authority,
            ownership,
            stop: Some(stop),
            task: Some(task),
        }
    }
    pub(super) async fn counted() -> (Self, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let record = calls.clone();
        let router = Router::new().route(
            "/probe",
            get(move || {
                let record = record.clone();
                async move {
                    record.fetch_add(1, Ordering::SeqCst);
                    "synthetic-ok"
                }
            }),
        );
        (Self::start(router).await, calls)
    }
    pub(super) fn request(&self) -> Vec<u8> {
        format!("GET /probe HTTP/1.1\r\nHost: {}\r\n\r\n", self.authority).into_bytes()
    }
    pub(super) fn stop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
    pub(super) fn finished(&self) -> bool {
        self.task.as_ref().unwrap().is_finished()
    }
    pub(super) async fn shutdown(mut self) {
        if self.stop.is_some() {
            assert!(
                self.ownership.is_open_in_this_process().unwrap(),
                "listener disappeared before its normal stop"
            );
        }
        self.stop();
        let joined =
            tokio::time::timeout(Duration::from_secs(5), self.task.as_mut().unwrap()).await;
        let okay = matches!(joined, Ok(Ok(Ok(()))));
        if !okay {
            self.task.as_ref().unwrap().abort();
            let _ = self.task.as_mut().unwrap().await;
        }
        self.task.take();
        assert!(okay, "owned HTTP loop did not shut down successfully");
        assert!(
            !self.ownership.is_open_in_this_process().unwrap(),
            "test process retained its original listener socket after shutdown"
        );
    }
    pub(super) async fn drain_expired(mut self) {
        self.stop();
        let joined =
            tokio::time::timeout(Duration::from_secs(35), self.task.as_mut().unwrap()).await;
        let expected = matches!(joined, Ok(Ok(Err(()))));
        if !expected {
            self.task.as_ref().unwrap().abort();
            let _ = self.task.as_mut().unwrap().await;
        }
        self.task.take();
        assert!(
            expected,
            "expired soft drain did not fail after aborting and joining owned tasks"
        );
        assert!(!self.ownership.is_open_in_this_process().unwrap());
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
pub(super) async fn read(mut socket: TcpStream) -> Vec<u8> {
    let mut bytes = Vec::new();
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        (&mut socket).take(64 * 1024).read_to_end(&mut bytes),
    )
    .await;
    match result {
        Ok(Ok(_)) => {}
        Ok(Err(error))
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted
            ) => {}
        _ => panic!("owned raw TCP response failed or exceeded its deadline"),
    }
    assert!(
        bytes.len() < 64 * 1024,
        "raw response exceeded fixture budget"
    );
    bytes
}
pub(super) async fn exchange(address: SocketAddr, request: &[u8]) -> Vec<u8> {
    let mut socket = tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(address))
        .await
        .unwrap()
        .unwrap();
    match socket.write_all(request).await {
        Ok(()) => {}
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionReset
                    | io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::BrokenPipe
            ) => {}
        Err(_) => panic!("owned raw TCP request failed"),
    }
    read(socket).await
}
pub(super) fn accepted(bytes: &[u8]) {
    assert!(
        bytes.starts_with(b"HTTP/1.1 200 ") && bytes.ends_with(b"synthetic-ok"),
        "positive TCP control was not admitted"
    );
}
pub(super) fn denied(bytes: &[u8]) {
    assert!(
        bytes.is_empty()
            || [
                b"HTTP/1.1 400 ".as_slice(),
                b"HTTP/1.1 408 ",
                b"HTTP/1.1 431 ",
                // The request itself remains rejected; Hyper can answer HTTP/1.0 in kind.
                b"HTTP/1.0 400 ",
                b"HTTP/1.0 408 ",
                b"HTTP/1.0 431 "
            ]
            .iter()
            .any(|prefix| bytes.starts_with(prefix)),
        "malformed wire input was not rejected"
    );
    let lower = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    for forbidden in [
        "set-cookie:",
        "access-control-allow-origin:",
        "access-control-allow-credentials:",
        "location:",
        CANARY,
    ] {
        assert!(
            !lower.contains(forbidden),
            "rejected transport leaked a private wire value or active response header"
        );
    }
}
