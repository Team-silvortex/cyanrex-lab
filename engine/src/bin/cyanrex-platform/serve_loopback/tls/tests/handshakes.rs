use super::{material::Material, network::*};
use rustls::{ClientConfig, ClientConnection};
use std::{
    io,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    task::JoinSet,
};

pub(super) fn hello(address: std::net::SocketAddr, config: Arc<ClientConfig>) -> Vec<u8> {
    let mut state = ClientConnection::new(config, address.ip().into()).unwrap();
    let mut bytes = Vec::new();
    state.write_tls(&mut bytes).unwrap();
    bytes
}
pub(super) async fn held_hello(
    address: std::net::SocketAddr,
    config: Arc<ClientConfig>,
) -> TcpStream {
    let mut stream = TcpStream::connect(address).await.unwrap();
    stream.write_all(&hello(address, config)).await.unwrap();
    let mut header = [0; 5];
    tokio::time::timeout(Duration::from_secs(2), stream.read_exact(&mut header))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(header[0], 22, "no admitted TLS ServerHello flight");
    let length = u16::from_be_bytes([header[3], header[4]]) as usize;
    assert!((1..=16384).contains(&length));
    let mut body = vec![0; length];
    stream.read_exact(&mut body).await.unwrap();
    assert_eq!(body[0], 2, "positive flight was not ServerHello");
    // Do not process the flight or send ClientFinished. This is an actual admitted incomplete
    // TLS handshake, not a connect() which could merely be waiting in the kernel listen backlog.
    stream
}

#[tokio::test]
async fn tls_idle_and_actively_dripping_client_hello_share_one_fixed_handshake_deadline() {
    let material = Material::new();
    let (server, calls) = Server::counted(&material).await;
    let started = Instant::now();
    let socket = TcpStream::connect(server.address).await.unwrap();
    let idle = read(socket).await;
    let idle_elapsed = started.elapsed();
    assert!(idle.is_empty());
    assert!(idle_elapsed >= Duration::from_millis(1700) && idle_elapsed < Duration::from_secs(4));
    let mut socket = TcpStream::connect(server.address).await.unwrap();
    let bytes = hello(
        server.address,
        material.client_config(true, vec![b"http/1.1".to_vec()]),
    );
    socket.write_all(&bytes[..1]).await.unwrap();
    let (reader, mut writer) = socket.into_split();
    let sent = Arc::new(AtomicUsize::new(0));
    let record = sent.clone();
    let dripping = tokio::spawn(async move {
        for byte in &bytes[1..] {
            tokio::time::sleep(Duration::from_millis(150)).await;
            if writer.write_all(&[*byte]).await.is_err() {
                break;
            }
            record.fetch_add(1, Ordering::SeqCst);
        }
    });
    let mut received = Vec::new();
    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(4),
        reader.take(64 * 1024).read_to_end(&mut received),
    )
    .await;
    let elapsed = started.elapsed();
    dripping.abort();
    let _ = dripping.await;
    let closed = matches!(result, Ok(Ok(_)))
        || matches!(result,Ok(Err(ref error)) if matches!(error.kind(),io::ErrorKind::ConnectionReset|io::ErrorKind::ConnectionAborted));
    let dispatched = calls.load(Ordering::SeqCst);
    accepted(&exchange(&server, &material, &server.request()).await);
    server.shutdown().await;
    drop(material);
    assert!(closed && elapsed >= Duration::from_millis(1700) && elapsed < Duration::from_secs(4));
    assert!(
        sent.load(Ordering::SeqCst) >= 6,
        "negative control did not continuously send handshake bytes"
    );
    assert_eq!(dispatched, 0);
    private(&received);
}

#[tokio::test]
async fn tls_sixteen_incomplete_admitted_handshakes_hold_capacity_until_a_socket_is_released() {
    let material = Material::new();
    let (server, calls) = Server::counted(&material).await;
    accepted(&exchange(&server, &material, &server.request()).await);
    let config = material.client_config(true, vec![b"http/1.1".to_vec()]);
    let mut pending = JoinSet::new();
    for _ in 0..16 {
        let config = config.clone();
        let address = server.address;
        pending.spawn(async move { held_hello(address, config).await });
    }
    let mut sockets = Vec::new();
    while let Some(result) = pending.join_next().await {
        sockets.push(result.unwrap());
    }
    assert_eq!(sockets.len(), 16);
    assert!(
        connect(server.address, config.clone(), server.address.ip().into())
            .await
            .is_err()
    );
    let mut released = sockets.remove(0);
    released.shutdown().await.unwrap();
    let _ = read(released).await;
    accepted(&exchange(&server, &material, &server.request()).await);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    drop(sockets);
    server.shutdown().await;
    drop(material);
}

#[tokio::test]
async fn tls_stop_cancels_idle_and_admitted_partial_handshakes_without_waiting_their_deadline() {
    let material = Material::new();
    let (server, calls) = Server::counted(&material).await;
    let idle = TcpStream::connect(server.address).await.unwrap();
    let partial = held_hello(
        server.address,
        material.client_config(true, vec![b"http/1.1".to_vec()]),
    )
    .await;
    let started = Instant::now();
    server.shutdown().await;
    let elapsed = started.elapsed();
    let _ = read(idle).await;
    let _ = read(partial).await;
    drop(material);
    assert!(
        elapsed < Duration::from_secs(1),
        "stop waited for an unfinished handshake deadline"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
