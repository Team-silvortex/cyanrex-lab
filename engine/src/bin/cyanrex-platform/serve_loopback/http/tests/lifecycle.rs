use super::support::*;
use axum::{routing::get, Router};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    sync::{mpsc, Semaphore},
};

struct Dropped(Arc<AtomicBool>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn loopback_http_soft_drain_deadline_aborts_and_joins_owned_async_handlers_then_fails() {
    let cancelled = Arc::new(AtomicBool::new(false));
    let record = cancelled.clone();
    let (entered, mut entries) = mpsc::unbounded_channel::<()>();
    let router = Router::new().route(
        "/probe",
        get(move || {
            let record = record.clone();
            let entered = entered.clone();
            async move {
                let _dropped = Dropped(record);
                entered.send(()).unwrap();
                std::future::pending::<()>().await;
                "synthetic-ok"
            }
        }),
    );
    let mut server = Server::start(router).await;
    let mut socket = TcpStream::connect(server.address).await.unwrap();
    socket.write_all(&server.request()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), entries.recv())
        .await
        .unwrap()
        .unwrap();
    let started = Instant::now();
    server.stop();
    server.drain_expired().await;
    let elapsed = started.elapsed();
    assert!(elapsed >= Duration::from_secs(29) && elapsed < Duration::from_secs(35));
    assert!(
        cancelled.load(Ordering::SeqCst),
        "soft drain returned before its owned async handler dropped"
    );
    // This is cancellation of a fake async handler, not rollback or termination of KDF jobs.
    denied(&read(socket).await);
}

#[tokio::test]
async fn loopback_http_shutdown_closes_listener_but_drains_admitted_business_past_header_deadline()
{
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
                "synthetic-ok"
            }
        }),
    );
    let mut server = Server::start(router).await;
    let mut socket = TcpStream::connect(server.address).await.unwrap();
    socket.write_all(&server.request()).await.unwrap();
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
    .expect("stopped listener still accepted connections");
    // This elapsed-time assertion targets the fixed transport header budget, never KDF speed.
    // The dispatched handler is deliberately held: no total connection timeout may cancel it.
    tokio::time::sleep(Duration::from_millis(2300)).await;
    assert!(
        !server.finished(),
        "shutdown cancelled an admitted business handler"
    );
    release.add_permits(1);
    accepted(&read(socket).await);
    server.shutdown().await;
}
