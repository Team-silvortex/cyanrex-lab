//! Raw TCP over the production HTTP loop with a synthetic Router only. This is not a CLI,
//! PostgreSQL, authentication, commit, Cookie-delivery or browser acceptance proof.
use axum::{routing::get, Router};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

#[path = "tests/lifecycle.rs"]
mod lifecycle;
#[path = "tests/limits.rs"]
mod limits;
#[path = "tests/panic.rs"]
mod panic;
#[path = "tests/protocol.rs"]
mod protocol;
#[path = "tests/support.rs"]
mod support;

#[tokio::test]
async fn loopback_http_real_tcp_serves_one_exact_host_http11_request() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let authority = address.to_string();
    let (stop, stopped) = oneshot::channel::<()>();
    let task = tokio::spawn(super::serve(
        listener,
        Router::new().route("/probe", get(|| async { "synthetic-ok" })),
        authority.clone(),
        async {
            let _ = stopped.await;
        },
    ));
    let reply = tokio::time::timeout(Duration::from_secs(4), async {
        let mut socket = TcpStream::connect(address).await?;
        socket
            .write_all(format!("GET /probe HTTP/1.1\r\nHost: {authority}\r\n\r\n").as_bytes())
            .await?;
        let mut bytes = Vec::new();
        socket.read_to_end(&mut bytes).await?;
        Ok::<_, std::io::Error>(bytes)
    })
    .await;
    let _ = stop.send(());
    let joined = tokio::time::timeout(Duration::from_secs(4), task).await;
    assert!(
        reply.is_ok_and(|result| result.is_ok_and(
            |bytes| bytes.starts_with(b"HTTP/1.1 200 ") && bytes.ends_with(b"synthetic-ok")
        )),
        "real HTTP socket did not serve the admitted request"
    );
    assert!(
        joined.is_ok_and(|result| result.is_ok_and(|result| result.is_ok())),
        "owned HTTP loop failed"
    );
}
