use super::material::Material;
use crate::serve_loopback::socket_ownership::SocketOwnership;
use axum::{routing::get, Router};
use rustls::{pki_types::ServerName, ClientConfig, ServerConfig};
use std::{
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinHandle,
};
use tokio_rustls::{client::TlsStream, TlsConnector};

pub(super) const CANARY: &str = "synthetic-tls-request-canary-never-echo-3c9f";
pub(super) struct Server {
    pub(super) address: SocketAddr,
    pub(super) authority: String,
    ownership: SocketOwnership,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), ()>>>,
}
impl Server {
    pub(super) async fn new(material: &Material, router: Router) -> Self {
        Self::at_ip(material, router, Ipv4Addr::LOCALHOST.into()).await
    }
    pub(super) async fn at_ip(material: &Material, router: Router, ip: IpAddr) -> Self {
        let config =
            crate::serve_loopback::tls::load_tls_identity(&material.cert, &material.key, ip)
                .unwrap_or_else(|_| panic!("owned test TLS identity rejected"));
        Self::configured(router, ip, config).await
    }
    async fn configured(router: Router, ip: IpAddr, config: Arc<ServerConfig>) -> Self {
        let listener = TcpListener::bind((ip, 0)).await.unwrap();
        let ownership = SocketOwnership::capture(&listener).unwrap();
        assert!(ownership.is_open_in_this_process().unwrap());
        let address = listener.local_addr().unwrap();
        let authority = address.to_string();
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(crate::serve_loopback::http::serve_tls(
            listener,
            router,
            authority.clone(),
            config,
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
    pub(super) async fn counted(material: &Material) -> (Self, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let record = calls.clone();
        let router = Router::new().route(
            "/probe",
            get(move || {
                let record = record.clone();
                async move {
                    record.fetch_add(1, Ordering::SeqCst);
                    "synthetic-tls-ok"
                }
            }),
        );
        (Self::new(material, router).await, calls)
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
                "TLS listener disappeared before its normal stop"
            );
        }
        self.stop();
        let result =
            tokio::time::timeout(Duration::from_secs(5), self.task.as_mut().unwrap()).await;
        let good = matches!(result, Ok(Ok(Ok(()))));
        if !good {
            self.task.as_ref().unwrap().abort();
            let _ = self.task.as_mut().unwrap().await;
        }
        self.task.take();
        assert!(good, "owned TLS server did not shut down cleanly");
        assert!(
            !self.ownership.is_open_in_this_process().unwrap(),
            "test process retained its original TLS listener socket after shutdown"
        );
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
pub(super) async fn connect(
    address: SocketAddr,
    config: Arc<ClientConfig>,
    name: ServerName<'static>,
) -> io::Result<TlsStream<TcpStream>> {
    tokio::time::timeout(Duration::from_secs(5), async {
        let socket = TcpStream::connect(address).await?;
        TlsConnector::from(config).connect(name, socket).await
    })
    .await
    .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?
}
pub(super) async fn trusted(server: &Server, material: &Material) -> TlsStream<TcpStream> {
    connect(
        server.address,
        material.client_config(true, vec![b"http/1.1".to_vec()]),
        server.address.ip().into(),
    )
    .await
    .unwrap_or_else(|_| panic!("trusted actual TLS connection failed"))
}
pub(super) async fn read(mut stream: impl AsyncRead + Unpin) -> Vec<u8> {
    let mut bytes = Vec::new();
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        (&mut stream).take(64 * 1024).read_to_end(&mut bytes),
    )
    .await;
    match result {
        Ok(Ok(_)) => {}
        Ok(Err(error))
            if matches!(
                error.kind(),
                io::ErrorKind::UnexpectedEof
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::ConnectionAborted
            ) => {}
        _ => panic!("owned TLS read failed or exceeded its deadline"),
    }
    assert!(bytes.len() < 64 * 1024);
    bytes
}
pub(super) async fn exchange(server: &Server, material: &Material, request: &[u8]) -> Vec<u8> {
    let mut stream = trusted(server, material).await;
    stream.write_all(request).await.unwrap();
    read(stream).await
}
pub(super) fn accepted(bytes: &[u8]) {
    assert!(
        bytes.starts_with(b"HTTP/1.1 200 ") && bytes.ends_with(b"synthetic-tls-ok"),
        "trusted TLS request did not reach HTTP1 handler"
    );
}
pub(super) fn private(bytes: &[u8]) {
    let text = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    for value in ["set-cookie:", "access-control-allow-", "location:", CANARY] {
        assert!(
            !text.contains(value),
            "rejected TLS transport leaked private or active response values"
        );
    }
}
