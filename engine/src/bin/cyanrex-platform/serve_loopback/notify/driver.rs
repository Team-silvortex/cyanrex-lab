//! A failed local notification is not permission to cancel an admitted command.
use crate::serve_loopback::{http, notify::Notifier, socket_ownership::SocketOwnership};
use axum::{routing::get, Router};
use std::{
    os::unix::{fs::DirBuilderExt, net::UnixDatagram},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot, Semaphore},
};

#[test]
fn stopping_send_failure_still_closes_acceptance_and_drains_the_admitted_handler() {
    assert!(super::support::child("stopping_error", None).1);
}

pub(super) async fn stopping_failure_fixture() {
    let directory =
        std::env::temp_dir().join(format!("cyanrex-notify-drain-{}", uuid::Uuid::new_v4()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let path = directory.join("receiver");
    let receiver = UnixDatagram::bind(&path).unwrap();
    let notifier = Notifier::connect(path.as_os_str()).unwrap();
    drop(receiver);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let ownership = SocketOwnership::capture(&listener).unwrap();
    let (entered, mut entries) = mpsc::unbounded_channel();
    let release = Arc::new(Semaphore::new(0));
    let wait = Arc::clone(&release);
    let router = Router::new().route(
        "/held",
        get(move || {
            let entered = entered.clone();
            let wait = Arc::clone(&wait);
            async move {
                let _ = entered.send(());
                wait.acquire().await.unwrap().forget();
                "synthetic-admitted-work"
            }
        }),
    );
    let (stop, stopped) = oneshot::channel();
    let mut task = tokio::spawn(async move {
        http::serve_notified(
            listener,
            router,
            address.to_string(),
            None,
            Some(&notifier),
            async {
                let _ = stopped.await;
            },
        )
        .await
    });
    let mut socket = TcpStream::connect(address).await.unwrap();
    socket
        .write_all(format!("GET /held HTTP/1.1\r\nHost: {address}\r\n\r\n").as_bytes())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), entries.recv())
        .await
        .unwrap()
        .unwrap();
    let _ = stop.send(());
    let closed = tokio::time::timeout(Duration::from_secs(2), async {
        while ownership.is_open_in_this_process().unwrap() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .is_ok();
    tokio::time::sleep(Duration::from_millis(50)).await;
    let retained = !task.is_finished();
    release.add_permits(1);
    let mut response = Vec::new();
    let received =
        tokio::time::timeout(Duration::from_secs(2), socket.read_to_end(&mut response)).await;
    let joined = tokio::time::timeout(Duration::from_secs(2), &mut task).await;
    let failed = matches!(joined, Ok(Ok(Err(()))));
    if !task.is_finished() {
        task.abort();
        let _ = task.await;
    }
    drop(socket);
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir(&directory).unwrap();
    assert!(
        closed && retained,
        "notification failure bypassed the normal drain"
    );
    assert!(
        received.is_ok_and(|result| result.is_ok())
            && response.ends_with(b"synthetic-admitted-work")
    );
    assert!(
        failed,
        "unconfirmed STOPPING delivery must fail only after draining work"
    );
}
