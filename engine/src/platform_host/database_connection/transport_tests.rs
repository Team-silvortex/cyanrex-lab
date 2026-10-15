//! Real loopback handshake faults only, not successful authentication/catalog or deployed TLS.
use super::*;
use tokio::{
    io::AsyncReadExt,
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

async fn fixture() -> (TcpListener, JoinHandle<Result<PgConnection>>) {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let options = PgConnectOptions::new_without_pgpass()
        .host("127.0.0.1")
        .port(listener.local_addr().unwrap().port())
        .username("synthetic_connector_role")
        .database("synthetic_connector_database")
        .password("synthetic_connector_password")
        .ssl_mode(PgSslMode::Disable)
        .disable_statement_logging();
    let pending = tokio::spawn(async move { dial(&options).await });
    (listener, pending)
}

async fn startup(listener: &TcpListener) -> Option<TcpStream> {
    tokio::time::timeout(Duration::from_secs(2), async {
        let (mut peer, _) = listener.accept().await.ok()?;
        let mut header = [0; 4];
        peer.read_exact(&mut header).await.ok()?;
        let length = u32::from_be_bytes(header) as usize;
        if !(8..=4096).contains(&length) {
            return None;
        }
        let mut frame = vec![0; length - 4];
        peer.read_exact(&mut frame).await.ok()?;
        // No AuthenticationRequest is sent; the synthetic password is never requested.
        Some(peer)
    })
    .await
    .ok()
    .flatten()
}

async fn no_second_attempt(listener: &TcpListener) -> bool {
    tokio::time::timeout(Duration::from_millis(150), listener.accept())
        .await
        .is_err()
}

#[tokio::test]
async fn platform_connection_peer_close_is_one_failed_attempt_without_raw_diagnostics() {
    let (listener, mut pending) = fixture().await;
    let peer = startup(&listener).await;
    let reached = peer.is_some();
    drop(peer);
    let result = tokio::time::timeout(Duration::from_secs(2), &mut pending).await;
    let failed = matches!(
        &result,
        Ok(Ok(Err(PlatformDatabaseConnectionError::Unavailable)))
    );
    drop(result);
    if !pending.is_finished() {
        pending.abort();
        let _ = pending.await;
    }
    let no_retry = no_second_attempt(&listener).await;
    drop(listener);
    assert!(reached && failed && no_retry);
    assert_eq!(
        PlatformDatabaseConnectionError::Unavailable.to_string(),
        "platform database connection is unavailable or unverified"
    );
}

#[tokio::test]
async fn platform_connection_cancelled_handshake_closes_the_socket_without_retry() {
    let (listener, pending) = fixture().await;
    let mut peer = startup(&listener).await;
    let reached = peer.is_some();
    pending.abort();
    let cancelled = pending.await.is_err_and(|error| error.is_cancelled());
    let closed = if let Some(peer) = peer.as_mut() {
        matches!(
            tokio::time::timeout(Duration::from_secs(2), peer.read(&mut [0; 1])).await,
            Ok(Ok(0))
        )
    } else {
        false
    };
    let no_retry = no_second_attempt(&listener).await;
    drop(peer);
    drop(listener);
    assert!(reached && cancelled && closed && no_retry);
}

#[tokio::test]
async fn platform_connection_stalled_handshake_hits_the_real_deadline_then_closes() {
    let (listener, mut pending) = fixture().await;
    let mut peer = startup(&listener).await;
    let reached = peer.is_some();
    let result = tokio::time::timeout(Duration::from_secs(12), &mut pending).await;
    let expired = matches!(
        &result,
        Ok(Ok(Err(PlatformDatabaseConnectionError::Unavailable)))
    );
    drop(result);
    if !pending.is_finished() {
        pending.abort();
        let _ = pending.await;
    }
    let closed = if let Some(peer) = peer.as_mut() {
        matches!(
            tokio::time::timeout(Duration::from_secs(2), peer.read(&mut [0; 1])).await,
            Ok(Ok(0))
        )
    } else {
        false
    };
    let no_retry = no_second_attempt(&listener).await;
    drop(peer);
    drop(listener);
    assert!(reached && expired && closed && no_retry);
}
