use super::support::*;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn loopback_http_host_version_target_and_upgrade_gate_precedes_router() {
    let (server, calls) = Server::counted().await;
    for request in [
        "GET /probe HTTP/1.1\r\n\r\n".to_owned(),
        format!(
            "GET /probe HTTP/1.1\r\nHost: {}\r\nHost: {}\r\n\r\n",
            server.authority, server.authority
        ),
        format!("GET /probe HTTP/1.1\r\nHost: {CANARY}\r\n\r\n"),
        format!(
            "GET /probe HTTP/1.1\r\nHost: localhost:{}\r\n\r\n",
            server.address.port()
        ),
        format!("GET /probe HTTP/1.0\r\nHost: {}\r\n\r\n", server.authority),
        format!(
            "GET http://{}/probe HTTP/1.1\r\nHost: {}\r\n\r\n",
            server.authority, server.authority
        ),
        format!("OPTIONS * HTTP/1.1\r\nHost: {}\r\n\r\n", server.authority),
        format!(
            "CONNECT {} HTTP/1.1\r\nHost: {}\r\n\r\n",
            server.authority, server.authority
        ),
        format!(
            "GET /probe HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\n\r\n",
            server.authority
        ),
        format!(
            "GET /probe HTTP/1.1\r\nHost: {}\r\nConnection: keep-alive, UpGrAdE\r\n\r\n",
            server.authority
        ),
    ] {
        denied(&exchange(server.address, request.as_bytes()).await);
    }
    // A syntactically valid wrong Host reaches the explicit transport gate, not Hyper's parser.
    // Its privacy headers and empty body are a stronger contract than native parse-error closes.
    let wrong_host = format!("GET /probe HTTP/1.1\r\nHost: {CANARY}\r\n\r\n");
    let gated = exchange(server.address, wrong_host.as_bytes()).await;
    assert!(gated.starts_with(b"HTTP/1.1 400 "));
    let end = gated
        .windows(4)
        .position(|part| part == b"\r\n\r\n")
        .unwrap();
    assert_eq!(gated.len(), end + 4, "Host gate returned a nonempty body");
    let fields = String::from_utf8_lossy(&gated[..end]).to_ascii_lowercase();
    for (name, value) in [
        ("cache-control", "no-store"),
        ("pragma", "no-cache"),
        ("x-content-type-options", "nosniff"),
        ("content-length", "0"),
    ] {
        let values = fields
            .split("\r\n")
            .filter_map(|line| line.split_once(':'))
            .filter(|(key, _)| *key == name)
            .map(|(_, value)| value.trim())
            .collect::<Vec<_>>();
        assert!(
            values == [value],
            "Host gate private response header diverged"
        );
    }
    denied(&gated);
    // HTTP/2 is never negotiated. Hyper's native protocol rejection may be empty or 400/505,
    // rather than the private Router's JSON/status policy.
    let h2 = exchange(server.address, b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").await;
    assert!(h2.is_empty() || h2.starts_with(b"HTTP/1.1 400 ") || h2.starts_with(b"HTTP/1.1 505 "));
    let lower = String::from_utf8_lossy(&h2).to_ascii_lowercase();
    for forbidden in [
        "101 switching",
        "set-cookie:",
        "access-control-allow-",
        "location:",
        CANARY,
    ] {
        assert!(!lower.contains(forbidden));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    accepted(&exchange(server.address, &server.request()).await);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    server.shutdown().await;
}

#[tokio::test]
async fn loopback_http_forwarded_values_neither_override_host_nor_grant_trust() {
    let (server, calls) = Server::counted().await;
    let wrong=format!("GET /probe HTTP/1.1\r\nHost: {CANARY}\r\nForwarded: host={}\r\nX-Forwarded-Host: {}\r\n\r\n",server.authority,server.authority);
    denied(&exchange(server.address, wrong.as_bytes()).await);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let valid=format!("GET /probe HTTP/1.1\r\nHost: {}\r\nForwarded: proto=https;host={CANARY}\r\nX-Forwarded-Host: {CANARY}\r\nX-Forwarded-Proto: https\r\n\r\n",server.authority);
    accepted(&exchange(server.address, valid.as_bytes()).await);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    server.shutdown().await;
}

#[tokio::test]
async fn loopback_http_one_request_per_connection_ignores_pipelined_second_request() {
    let (server, calls) = Server::counted().await;
    let mut requests = server.request();
    requests.extend_from_slice(&server.request());
    let reply = exchange(server.address, &requests).await;
    accepted(&reply);
    assert_eq!(
        reply
            .windows(b"HTTP/1.1 200 ".len())
            .filter(|part| *part == b"HTTP/1.1 200 ")
            .count(),
        1
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    accepted(&exchange(server.address, &server.request()).await);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    server.shutdown().await;
}
