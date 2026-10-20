use super::{material::Material, network::*};
use axum::{routing::get, Router};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    sync::{mpsc, Semaphore},
};

#[tokio::test]
async fn tls_handshake_completion_starts_a_new_http_header_budget_not_a_whole_connection_timeout() {
    let material = Material::new();
    let (server, calls) = Server::counted(&material).await;
    let socket = TcpStream::connect(server.address).await.unwrap();
    let started = Instant::now();
    // Spend part of the TLS-only budget before sending ClientHello, then complete normally.
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let mut stream =
        tokio_rustls::TlsConnector::from(material.client_config(true, vec![b"http/1.1".to_vec()]))
            .connect(server.address.ip().into(), socket)
            .await
            .unwrap();
    stream
        .write_all(
            format!(
                "GET /probe HTTP/1.1\r\nHost: {}\r\nX-Partial: ",
                server.authority
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(1200)).await;
    stream.write_all(b"finished\r\n\r\n").await.unwrap();
    accepted(&read(stream).await);
    assert!(started.elapsed() >= Duration::from_millis(2300));
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    server.shutdown().await;
    drop(material);
}

#[tokio::test]
async fn tls_shutdown_closes_acceptance_but_drains_dispatched_business_past_both_two_second_budgets(
) {
    let material = Material::new();
    let release = Arc::new(Semaphore::new(0));
    let wait = release.clone();
    let (entered, mut entries) = mpsc::unbounded_channel::<()>();
    let router = Router::new().route(
        "/probe",
        get(move || {
            let wait = wait.clone();
            let entered = entered.clone();
            async move {
                entered.send(()).unwrap();
                wait.acquire().await.unwrap().forget();
                "synthetic-tls-ok"
            }
        }),
    );
    let mut server = Server::new(&material, router).await;
    let mut stream = trusted(&server, &material).await;
    stream.write_all(&server.request()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), entries.recv())
        .await
        .unwrap()
        .unwrap();
    server.stop();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if TcpStream::connect(server.address).await.is_err() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("stopped TLS host still accepted connections");
    let started = Instant::now();
    tokio::time::sleep(Duration::from_millis(2300)).await;
    assert!(
        !server.finished(),
        "TLS/HTTP budget cancelled an already dispatched fake command"
    );
    release.add_permits(1);
    accepted(&read(stream).await);
    server.shutdown().await;
    drop(material);
    assert!(started.elapsed() >= Duration::from_millis(2300));
}
