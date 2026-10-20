use super::{material::Material, network::*};
use axum::{routing::get, Router};
use rustls::pki_types::ServerName;
use std::{
    net::Ipv6Addr,
    sync::{atomic::Ordering, Arc},
};
use tokio::{io::AsyncWriteExt, net::TcpStream};

#[tokio::test]
async fn tls_loaded_identity_supports_normal_verified_tls12_and_tls13_clients() {
    let material = Material::new();
    let (server, calls) = Server::counted(&material).await;
    for (version, expected) in [
        (&rustls::version::TLS12, rustls::ProtocolVersion::TLSv1_2),
        (&rustls::version::TLS13, rustls::ProtocolVersion::TLSv1_3),
    ] {
        let config = material.client_versions(true, vec![b"http/1.1".to_vec()], &[version]);
        let mut stream = connect(server.address, config, server.address.ip().into())
            .await
            .unwrap();
        assert_eq!(stream.get_ref().1.protocol_version(), Some(expected));
        stream.write_all(&server.request()).await.unwrap();
        accepted(&read(stream).await);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    server.shutdown().await;
    drop(material);
}

#[tokio::test]
async fn tls_alpn_missing_mixed_and_h2_only_preserve_http1_only_compatibility() {
    let material = Material::new();
    let (server, calls) = Server::counted(&material).await;
    for (offered, expected) in [
        (Vec::new(), None),
        (
            vec![b"h2".to_vec(), b"http/1.1".to_vec()],
            Some(b"http/1.1".as_slice()),
        ),
    ] {
        let mut stream = connect(
            server.address,
            material.client_config(true, offered),
            server.address.ip().into(),
        )
        .await
        .unwrap();
        assert_eq!(stream.get_ref().1.alpn_protocol(), expected);
        stream.write_all(&server.request()).await.unwrap();
        accepted(&read(stream).await);
    }
    assert!(connect(
        server.address,
        material.client_config(true, vec![b"h2".to_vec()]),
        server.address.ip().into()
    )
    .await
    .is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    server.shutdown().await;
    drop(material);
}

#[tokio::test]
async fn tls_unknown_ca_and_wrong_ip_san_refuse_before_any_http_handler() {
    let material = Material::new();
    let (server, calls) = Server::counted(&material).await;
    assert!(connect(
        server.address,
        material.client_config(false, vec![b"http/1.1".to_vec()]),
        server.address.ip().into()
    )
    .await
    .is_err());
    assert!(connect(
        server.address,
        material.client_config(true, vec![b"http/1.1".to_vec()]),
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 2)).into()
    )
    .await
    .is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    accepted(&exchange(&server, &material, &server.request()).await);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    server.shutdown().await;
    drop(material);
}

#[tokio::test]
async fn tls_wrong_dns_sni_is_independently_refused_even_when_the_certificate_covers_that_dns() {
    let material = Material::new();
    let (server, calls) = Server::counted(&material).await;
    let name = ServerName::try_from("wrong.fixture.test".to_owned()).unwrap();
    // Only SNI emission changes: CA/signature/DNS SAN validation remains fully enabled. This
    // successful real handshake proves the later refusal is not a client hostname mismatch.
    let mut without_sni = (*material.client_config(true, vec![b"http/1.1".to_vec()])).clone();
    without_sni.enable_sni = false;
    let mut stream = connect(server.address, Arc::new(without_sni), name.clone())
        .await
        .unwrap();
    stream.write_all(&server.request()).await.unwrap();
    accepted(&read(stream).await);
    assert!(connect(
        server.address,
        material.client_config(true, vec![b"http/1.1".to_vec()]),
        name
    )
    .await
    .is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    server.shutdown().await;
    drop(material);
}

#[tokio::test]
async fn tls_http_host_gate_and_plaintext_or_h2_inputs_cannot_downgrade_the_listener() {
    let material = Material::new();
    let (server, calls) = Server::counted(&material).await;
    for request in [
        format!("GET /probe HTTP/1.1\r\nHost: {CANARY}\r\n\r\n"),
        format!(
            "GET /probe HTTP/1.1\r\nHost: {}\r\nHost: {}\r\n\r\n",
            server.authority, server.authority
        ),
        format!(
            "GET http://{}/probe HTTP/1.1\r\nHost: {}\r\n\r\n",
            server.authority, server.authority
        ),
        format!(
            "GET /probe HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\n\r\n",
            server.authority
        ),
    ] {
        let reply = exchange(&server, &material, request.as_bytes()).await;
        assert!(reply.starts_with(b"HTTP/1.1 400 "));
        private(&reply);
        let end = reply
            .windows(4)
            .position(|part| part == b"\r\n\r\n")
            .unwrap();
        assert_eq!(reply.len(), end + 4);
        let lower = String::from_utf8_lossy(&reply).to_ascii_lowercase();
        for header in [
            "cache-control: no-store\r\n",
            "pragma: no-cache\r\n",
            "x-content-type-options: nosniff\r\n",
            "content-length: 0\r\n",
        ] {
            assert!(lower.contains(header));
        }
    }
    let reply = exchange(&server, &material, b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").await;
    assert!(
        reply.is_empty()
            || reply.starts_with(b"HTTP/1.1 400 ")
            || reply.starts_with(b"HTTP/1.1 505 ")
    );
    private(&reply);
    let mut raw = TcpStream::connect(server.address).await.unwrap();
    raw.write_all(&server.request()).await.unwrap();
    let reply = read(raw).await;
    assert!(
        !reply.starts_with(b"HTTP/"),
        "TLS port replied as a downgraded plaintext HTTP listener"
    );
    private(&reply);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    accepted(&exchange(&server, &material, &server.request()).await);
    server.shutdown().await;
    drop(material);
}

#[tokio::test]
async fn tls_ipv6_literal_without_sni_verifies_the_actual_ipv6_san() {
    let material = Material::new();
    let server = Server::at_ip(
        &material,
        Router::new().route("/probe", get(|| async { "synthetic-tls-ok" })),
        Ipv6Addr::LOCALHOST.into(),
    )
    .await;
    accepted(&exchange(&server, &material, &server.request()).await);
    server.shutdown().await;
    drop(material);
}
