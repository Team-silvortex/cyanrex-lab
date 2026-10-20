use super::{fixture::*, http, identity_helpers};
use std::net::{IpAddr, Ipv4Addr};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL, OpenSSL and non-root capless child support"]
async fn postgres_platform_loopback_tls_host_running_drift_readonly_and_missing_delete_preserve_unconfirmed_cookies(
) {
    let (ready, issued, denied, unchanged, restored, healthy, closed, no_backend, cleaned) =
        Fixture::checked(|f| {
            Box::pin(async move {
                let prepared = Prepared::new(f, IpAddr::V4(Ipv4Addr::LOCALHOST)).await;
                let mut child = prepared.spawn(None, None, None);
                let ready = child.ready().await;
                let mut issued = false;
                let mut denied = false;
                let mut unchanged = false;
                let mut restored = false;
                let mut healthy = false;
                if ready {
                    let client = http::client(Some(&prepared.tls.ca));
                    let token = http::token(&http::login(&client, &prepared.origin).await);
                    issued = token.is_some();
                    let token = token.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                    let original = snapshot(f).await;
                    denied = true;
                    unchanged = true;
                    restored = true;
                    for root in [true, false] {
                        if root {
                            denied &= prepared.root().swap();
                        } else {
                            identity_helpers::replace_marker(f, 3).await;
                        }
                        let before = snapshot(f).await;
                        for response in [
                            http::login(&client, &prepared.origin).await,
                            http::get(&client, &prepared.origin, &token).await,
                            http::logout(&client, &prepared.origin, &token).await,
                        ] {
                            denied &=
                                http::denied(&response, 503, "service_unavailable", "unconfirmed");
                        }
                        unchanged &= before == snapshot(f).await && session_count(f).await == 1;
                        if root {
                            restored &= prepared.root().restore();
                        } else {
                            identity_helpers::restore_marker(f, 3).await;
                        }
                        restored &= original == snapshot(f).await;
                    }
                    f.sql(&format!(
                        "REVOKE DELETE ON {}.sessions FROM {}",
                        f.source, f.role
                    ))
                    .await;
                    let before = snapshot(f).await;
                    denied &= http::denied(
                        &http::logout(&client, &prepared.origin, &token).await,
                        503,
                        "service_unavailable",
                        "unconfirmed",
                    );
                    unchanged &= before == snapshot(f).await && session_count(f).await == 1;
                    f.sql(&format!(
                        "GRANT DELETE ON {}.sessions TO {}",
                        f.source, f.role
                    ))
                    .await;
                    read_only(f, true).await;
                    let before = snapshot(f).await;
                    for response in [
                        http::login(&client, &prepared.origin).await,
                        http::get(&client, &prepared.origin, &token).await,
                        http::logout(&client, &prepared.origin, &token).await,
                    ] {
                        denied &=
                            http::denied(&response, 503, "service_unavailable", "unconfirmed");
                    }
                    unchanged &= before == snapshot(f).await && session_count(f).await == 1;
                    read_only(f, false).await;
                    healthy = http::current(&http::get(&client, &prepared.origin, &token).await)
                        && http::cleared(&http::logout(&client, &prepared.origin, &token).await)
                        && session_count(f).await == 0;
                }
                let closed = stopped(&child.stop().await);
                let no_backend = no_connections(f).await;
                let cleaned = prepared.finish();
                (
                    ready, issued, denied, unchanged, restored, healthy, closed, no_backend,
                    cleaned,
                )
            })
        })
        .await;
    assert!(
        ready
            && issued
            && denied
            && unchanged
            && restored
            && healthy
            && closed
            && no_backend
            && cleaned
    );
}
