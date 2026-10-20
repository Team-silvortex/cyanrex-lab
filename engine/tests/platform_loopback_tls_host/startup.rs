use super::{certificates::Certificates, fixture::*, identity_helpers, wait_helpers};
use std::{
    net::{IpAddr, Ipv4Addr, TcpListener},
    time::Duration,
};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL, OpenSSL and non-root capless child support"]
async fn postgres_platform_loopback_tls_host_tls_bind_and_original_storage_startup_failures_never_report_ready(
) {
    let (
        tls,
        wrong_password,
        occupied,
        identity,
        pending,
        root_changed,
        restored,
        healthy,
        unchanged,
        no_backend,
        cleaned,
    ) = Fixture::checked(|f| {
        Box::pin(async move {
            let prepared = Prepared::new(f, IpAddr::V4(Ipv4Addr::LOCALHOST)).await;
            let original = snapshot(f).await;
            let other = Certificates::generate(
                prepared.tls.key.parent().unwrap(),
                IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)),
            );
            let tls = failed(
                &prepared
                    .spawn(None, Some(&other.certificate), Some(&other.key))
                    .finish(None, Duration::from_secs(15))
                    .await,
            ) && no_listener(prepared.bind).await;
            let mut document = prepared.document.clone();
            document["database"]["password"] = "synthetic-deliberately-wrong-scram-password".into();
            let bad = prepared.variant("wrong-password.json", &document);
            let wrong_password = failed(
                &prepared
                    .spawn(Some(&bad), None, None)
                    .finish(None, Duration::from_secs(15))
                    .await,
            ) && no_listener(prepared.bind).await;
            let occupied_socket = TcpListener::bind(prepared.bind).unwrap();
            let occupied = failed(
                &prepared
                    .spawn(None, None, None)
                    .finish(None, Duration::from_secs(15))
                    .await,
            );
            drop(occupied_socket);
            let mut identity = true;
            identity_helpers::replace_namespace(f, 1).await;
            let replaced = snapshot(f).await;
            identity &= super::marker_helpers::current(f).await.is_ok()
                && failed(
                    &prepared
                        .spawn(None, None, None)
                        .finish(None, Duration::from_secs(15))
                        .await,
                )
                && replaced == snapshot(f).await;
            identity_helpers::restore_namespace(f, 1).await;
            for index in 0..5 {
                identity_helpers::replace_marker(f, index).await;
                let replaced = snapshot(f).await;
                identity &= super::marker_helpers::current(f).await.is_ok()
                    && failed(
                        &prepared
                            .spawn(None, None, None)
                            .finish(None, Duration::from_secs(15))
                            .await,
                    )
                    && replaced == snapshot(f).await;
                identity_helpers::restore_marker(f, index).await;
            }
            let (holder, oid, blocker) = wait_helpers::lock_marker(f).await;
            let child = prepared.spawn(None, None, None);
            let pending = wait_helpers::pending_marker_lock(f, oid, blocker).await;
            let swapped = prepared.root().swap();
            let released = wait_helpers::release(holder).await;
            let blocked_run = child.finish(None, Duration::from_secs(15)).await;
            let root_changed =
                swapped && released && failed(&blocked_run) && no_listener(prepared.bind).await;
            let restored = prepared.root().restore();
            // The same valid role/password and exact restored original declaration are usable.
            let mut next = prepared.spawn(None, None, None);
            let ready = next.ready().await;
            let run = next.stop().await;
            let healthy = ready && stopped(&run);
            let unchanged = original == snapshot(f).await;
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish();
            (
                tls,
                wrong_password,
                occupied,
                identity,
                pending,
                root_changed,
                restored,
                healthy,
                unchanged,
                no_backend,
                cleaned,
            )
        })
    })
    .await;
    assert!(
        tls && wrong_password
            && occupied
            && identity
            && pending
            && root_changed
            && restored
            && healthy
            && unchanged
            && no_backend
            && cleaned
    );
}
