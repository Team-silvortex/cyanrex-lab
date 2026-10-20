//! Native TLS transport with synthetic routers only: no CLI, database or browser authority proof.
#[path = "tests/handshakes.rs"]
mod handshakes;
#[path = "tests/identity.rs"]
mod identity;
#[path = "tests/lifecycle.rs"]
mod lifecycle;
#[path = "tests/material.rs"]
mod material;
#[path = "tests/network.rs"]
mod network;
#[path = "tests/protocol.rs"]
mod protocol;
use axum::{routing::get, Router};
use material::Material;
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};
use tokio_rustls::TlsConnector;

#[tokio::test]
async fn tls_real_driver_accepts_trusted_ip_certificate_and_http11_over_actual_socket() {
    let material = Material::new();
    // The initial driver Red is independent of the simultaneously rejecting loader scaffold.
    // This cfg(test)-only builder consumes generated test material, never a public host bypass.
    let config = material.raw_server_config();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let authority = address.to_string();
    let (stop, stopped) = oneshot::channel::<()>();
    let task = tokio::spawn(crate::serve_loopback::http::serve_tls(
        listener,
        Router::new().route("/probe", get(|| async { "synthetic-tls-ok" })),
        authority.clone(),
        config,
        async {
            let _ = stopped.await;
        },
    ));
    let reply = tokio::time::timeout(Duration::from_secs(5), async {
        let socket = TcpStream::connect(address).await?;
        let connector =
            TlsConnector::from(material.client_config(true, vec![b"http/1.1".to_vec()]));
        let mut tls = connector.connect(address.ip().into(), socket).await?;
        tls.write_all(format!("GET /probe HTTP/1.1\r\nHost: {authority}\r\n\r\n").as_bytes())
            .await?;
        let mut bytes = Vec::new();
        tls.read_to_end(&mut bytes).await?;
        Ok::<_, std::io::Error>(bytes)
    })
    .await;
    let _ = stop.send(());
    let joined = tokio::time::timeout(Duration::from_secs(5), task).await;
    drop(material);
    assert!(
        reply.is_ok_and(|result| result.is_ok_and(
            |bytes| bytes.starts_with(b"HTTP/1.1 200 ") && bytes.ends_with(b"synthetic-tls-ok")
        )),
        "actual trusted TLS socket did not reach HTTP/1.1"
    );
    assert!(joined.is_ok_and(|result| result.is_ok_and(|result| result.is_ok())));
}
