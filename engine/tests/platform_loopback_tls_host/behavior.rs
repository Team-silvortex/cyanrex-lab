use super::{fixture::*, http};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::Duration,
};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL, OpenSSL and non-root capless child support"]
async fn postgres_platform_loopback_tls_host_real_https_ipv4_ipv6_secure_cookie_login_query_logout_and_signals(
) {
    for ip in [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ] {
        let (ready, safe, issued, current, cleared, revoked, closed, no_backend, cleaned) =
            Fixture::checked(|f| {
                Box::pin(async move {
                    let prepared = Prepared::new(f, ip).await;
                    let mut child = prepared.spawn(None, None, None);
                    let ready = child.ready().await;
                    let mut safe = false;
                    let mut issued = false;
                    let mut current = false;
                    let mut cleared = false;
                    let mut revoked = false;
                    if ready {
                        let client = http::client(Some(&prepared.tls.ca));
                        let token = http::token(&http::login(&client, &prepared.origin).await);
                        issued = token.is_some() && watermark(f).await >= 0;
                        safe = safe_threads(child.pid(), prepared.identity());
                        if let Some(token) = token.as_deref() {
                            issued &= stored_digest(f, token).await;
                            current =
                                http::current(&http::get(&client, &prepared.origin, token).await);
                            cleared = http::cleared(
                                &http::logout(&client, &prepared.origin, token).await,
                            );
                            revoked = http::denied(
                                &http::get(&client, &prepared.origin, token).await,
                                401,
                                "invalid_session",
                                "unconfirmed",
                            ) && session_count(f).await == 0;
                        }
                    }
                    let signal = if ip.is_ipv6() {
                        libc::SIGINT
                    } else {
                        libc::SIGTERM
                    };
                    let closed =
                        stopped(&child.finish(Some(signal), Duration::from_secs(35)).await);
                    let no_backend = no_connections(f).await;
                    let cleaned = prepared.finish();
                    (
                        ready, safe, issued, current, cleared, revoked, closed, no_backend, cleaned,
                    )
                })
            })
            .await;
        assert!(ready, "real TLS child never reported readiness");
        assert!(safe && issued && current && cleared && revoked && closed && no_backend && cleaned);
    }
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL, OpenSSL and non-root capless child support"]
async fn postgres_platform_loopback_tls_host_unknown_ca_and_wrong_peer_refuse_before_login_then_trusted_client_succeeds(
) {
    let (ready, unknown, wrong, unchanged, healthy, closed, no_backend, cleaned) =
        Fixture::checked(|f| {
            Box::pin(async move {
                let prepared = Prepared::new(f, IpAddr::V4(Ipv4Addr::LOCALHOST)).await;
                let mut child = prepared.spawn(None, None, None);
                let ready = child.ready().await;
                let mut unknown = false;
                let mut wrong = false;
                let mut unchanged = false;
                let mut healthy = false;
                if ready {
                    let before = snapshot(f).await;
                    let counter = watermark(f).await;
                    unknown = http::refused_login(&http::client(None), &prepared.origin).await;
                    let wrong_origin =
                        format!("https://wrong-platform-peer.test:{}", prepared.bind.port());
                    wrong = http::refused_login(
                        &http::wrong_peer_client(&prepared.tls.ca, prepared.bind),
                        &wrong_origin,
                    )
                    .await;
                    unchanged = before == snapshot(f).await
                        && watermark(f).await == counter
                        && session_count(f).await == 0;
                    let trusted = http::client(Some(&prepared.tls.ca));
                    let token = http::token(&http::login(&trusted, &prepared.origin).await);
                    healthy = token.is_some();
                    if let Some(token) = token.as_deref() {
                        healthy &= stored_digest(f, token).await
                            && http::current(&http::get(&trusted, &prepared.origin, token).await)
                            && http::cleared(
                                &http::logout(&trusted, &prepared.origin, token).await,
                            );
                    }
                }
                let closed = stopped(&child.stop().await);
                let no_backend = no_connections(f).await;
                let cleaned = prepared.finish();
                (
                    ready, unknown, wrong, unchanged, healthy, closed, no_backend, cleaned,
                )
            })
        })
        .await;
    assert!(ready && unknown && wrong && unchanged && healthy && closed && no_backend && cleaned);
}
