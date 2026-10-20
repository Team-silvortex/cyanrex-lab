use super::{fixture::*, http};
use std::{
    net::{Ipv6Addr, TcpListener},
    time::Duration,
};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL and non-root capless child support"]
async fn postgres_platform_loopback_host_capless_http_login_cookie_query_logout_and_sigterm() {
    let (
        ready,
        safe,
        issued,
        authenticated,
        ipv6_available,
        ipv6_passed,
        logged_out,
        revoked,
        closed,
        no_backend,
        cleaned,
    ) = Fixture::checked(|f| {
        Box::pin(async move {
            let mut prepared = Prepared::new(f).await;
            let mut child = prepared.spawn(None);
            let ready = child.ready().await;
            let mut safe = false;
            let mut issued = false;
            let mut authenticated = false;
            let mut logged_out = false;
            let mut revoked = false;
            let mut ipv6_available = false;
            let mut ipv6_passed = false;
            if ready {
                let client = http::client();
                let response = http::login(&client, &prepared.origin).await;
                let token = http::token(&response);
                issued = token.is_some() && watermark(f).await >= 0;
                safe = safe_threads(child.pid(), &prepared.identity);
                if let Some(token) = token.as_deref() {
                    issued &= stored_digest(f, token).await;
                    authenticated =
                        http::current(&http::get(&client, &prepared.origin, token).await);
                    (ipv6_available, ipv6_passed) = ipv6(&mut prepared, token).await;
                    logged_out =
                        http::cleared(&http::logout(&client, &prepared.origin, token).await);
                    revoked = http::denied(
                        &http::get(&client, &prepared.origin, token).await,
                        401,
                        "invalid_session",
                        "unconfirmed",
                    ) && session_count(f).await == 0;
                }
            }
            let run = child.stop().await;
            let closed = stopped(&run);
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish();
            (
                ready,
                safe,
                issued,
                authenticated,
                ipv6_available,
                ipv6_passed,
                logged_out,
                revoked,
                closed,
                no_backend,
                cleaned,
            )
        })
    })
    .await;
    assert!(ready, "the real child never reported loopback readiness");
    assert!(
        ipv6_available,
        "explicit IPv6 loopback acceptance was unavailable, not passed"
    );
    assert!(
        safe && issued
            && authenticated
            && ipv6_passed
            && logged_out
            && revoked
            && closed
            && no_backend
            && cleaned
    );
}

async fn ipv6(prepared: &mut Prepared<'_>, token: &str) -> (bool, bool) {
    let Ok(reserve) = TcpListener::bind((Ipv6Addr::LOCALHOST, 0)) else {
        return (false, false);
    };
    let bind = reserve.local_addr().unwrap();
    drop(reserve);
    let origin = format!("http://{bind}");
    let mut document = prepared.document.clone();
    document["origin"] = origin.clone().into();
    let path = prepared.variant("ipv6.json", &document);
    let original_bind = prepared.bind;
    prepared.bind = bind;
    let mut child = prepared.spawn(Some(&path));
    prepared.bind = original_bind;
    let ready = child.ready().await;
    let mut query = false;
    let mut fallback = false;
    if ready {
        let client = http::client();
        query = http::current(&http::get(&client, &origin, token).await);
        let reply = http::call(
            &client,
            &origin,
            reqwest::Method::GET,
            "/platform/v1/not-a-route",
            None,
            None,
        )
        .await;
        fallback = http::denied(&reply, 404, "not_found", "not_attempted");
    }
    let run = child
        .finish(Some(libc::SIGINT), Duration::from_secs(35))
        .await;
    let no_port = super::fixture::no_listener(bind).await;
    (true, ready && query && fallback && stopped(&run) && no_port)
}
