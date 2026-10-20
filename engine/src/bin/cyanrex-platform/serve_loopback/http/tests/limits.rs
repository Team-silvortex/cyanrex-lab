use super::support::*;
use axum::{routing::get, Router};
use std::{
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{mpsc, Semaphore},
};

#[tokio::test]
async fn loopback_http_continuous_header_drip_cannot_reset_the_whole_header_deadline() {
    let (server, calls) = Server::counted().await;
    // Positive control completes partially sent headers inside the fixed two-second window.
    let mut socket = TcpStream::connect(server.address).await.unwrap();
    socket
        .write_all(
            format!(
                "GET /probe HTTP/1.1\r\nHost: {}\r\nX-Slow: ",
                server.authority
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    for _ in 0..3 {
        tokio::time::sleep(Duration::from_millis(200)).await;
        socket.write_all(b"x").await.unwrap();
    }
    socket.write_all(b"\r\n\r\n").await.unwrap();
    accepted(&read(socket).await);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let mut socket = TcpStream::connect(server.address).await.unwrap();
    socket
        .write_all(
            format!(
                "GET /probe HTTP/1.1\r\nHost: {}\r\nX-Slow: ",
                server.authority
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let (reader, mut writer) = socket.into_split();
    let writes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let record = writes.clone();
    let dripping = tokio::spawn(async move {
        for _ in 0..24 {
            tokio::time::sleep(Duration::from_millis(200)).await;
            if writer.write_all(b"x").await.is_err() {
                break;
            }
            record.fetch_add(1, Ordering::SeqCst);
        }
    });
    let started = Instant::now();
    let mut bytes = Vec::new();
    let result = tokio::time::timeout(
        Duration::from_secs(4),
        reader.take(64 * 1024).read_to_end(&mut bytes),
    )
    .await;
    let elapsed = started.elapsed();
    dripping.abort();
    let _ = dripping.await;
    let closed = matches!(result, Ok(Ok(_)))
        || matches!(result,Ok(Err(ref error)) if matches!(error.kind(),
        std::io::ErrorKind::ConnectionReset|std::io::ErrorKind::ConnectionAborted));
    // An idle-only implementation keeps the active drip open until the safety deadline. Reap
    // its writer and owned server before asserting, rather than leaking the negative control.
    let dispatched = calls.load(Ordering::SeqCst);
    server.shutdown().await;
    assert!(closed && elapsed >= Duration::from_millis(1700) && elapsed < Duration::from_secs(4));
    assert!(
        writes.load(Ordering::SeqCst) >= 4,
        "negative control did not actively drip bytes"
    );
    assert_eq!(dispatched, 1);
    denied(&bytes);
}

#[tokio::test]
async fn loopback_http_bounds_actual_header_count_and_buffer_before_dispatch() {
    let (server, calls) = Server::counted().await;
    let mut allowed = format!("GET /probe HTTP/1.1\r\nHost: {}\r\n", server.authority);
    for index in 0..31 {
        allowed.push_str(&format!("X-Synthetic-{index}: value\r\n"));
    }
    allowed.push_str("\r\n");
    accepted(&exchange(server.address, allowed.as_bytes()).await);
    let rejected = allowed.replacen("\r\n\r\n", "\r\nX-Excess: rejected\r\n\r\n", 1);
    denied(&exchange(server.address, rejected.as_bytes()).await);
    let huge = format!(
        "GET /probe HTTP/1.1\r\nHost: {}\r\nX-Synthetic: {}{CANARY}\r\n\r\n",
        server.authority,
        "x".repeat(17 * 1024)
    );
    denied(&exchange(server.address, huge.as_bytes()).await);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    accepted(&exchange(server.address, &server.request()).await);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    server.shutdown().await;
}

#[tokio::test]
async fn loopback_http_idle_and_incomplete_headers_hit_a_real_whole_header_deadline() {
    let (server, calls) = Server::counted().await;
    for initial in [
        Vec::new(),
        format!(
            "GET /probe HTTP/1.1\r\nHost: {}\r\nX-Slow: ",
            server.authority
        )
        .into_bytes(),
    ] {
        let mut socket = TcpStream::connect(server.address).await.unwrap();
        socket.write_all(&initial).await.unwrap();
        let started = Instant::now();
        denied(&read(socket).await);
        assert!(
            started.elapsed() >= Duration::from_millis(1700),
            "header deadline fired too early"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "header deadline did not close idle input"
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    accepted(&exchange(server.address, &server.request()).await);
    server.shutdown().await;
}

#[tokio::test]
async fn loopback_http_sixteen_admitted_connections_reject_excess_then_release_on_completion() {
    let release = Arc::new(Semaphore::new(0));
    let (entered, mut entries) = mpsc::unbounded_channel::<()>();
    let record = release.clone();
    let router = Router::new()
        .route(
            "/held",
            get(move || {
                let record = record.clone();
                let entered = entered.clone();
                async move {
                    entered.send(()).unwrap();
                    record.acquire().await.unwrap().forget();
                    "synthetic-ok"
                }
            }),
        )
        .route("/probe", get(|| async { "synthetic-ok" }));
    let server = Server::start(router).await;
    let mut sockets = Vec::new();
    for _ in 0..16 {
        let mut socket = TcpStream::connect(server.address).await.unwrap();
        socket
            .write_all(
                format!("GET /held HTTP/1.1\r\nHost: {}\r\n\r\n", server.authority).as_bytes(),
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(2), entries.recv())
            .await
            .unwrap()
            .unwrap();
        sockets.push(socket);
    }
    // Each request reached the synthetic handler: this proves all sixteen permits were admitted,
    // rather than guessing from connect() success (which also succeeds in a TCP listen backlog).
    denied(&exchange(server.address, &server.request()).await);
    release.add_permits(1);
    accepted(&read(sockets.remove(0)).await);
    accepted(&exchange(server.address, &server.request()).await);
    release.add_permits(15);
    for socket in sockets {
        accepted(&read(socket).await);
    }
    server.shutdown().await;
}
