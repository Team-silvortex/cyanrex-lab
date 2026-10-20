use super::{fixture::*, identity_helpers, marker_helpers};
use std::{net::TcpListener, time::Duration};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL and non-root capless child support"]
async fn postgres_platform_loopback_host_first_start_rejects_original_catalog_and_marker_substitution(
) {
    let (current_valid, refused, unchanged, restored, no_backend, cleaned) =
        Fixture::checked(|f| {
            Box::pin(async move {
                // The private declaration precedes all substitutions. A current-only diagnostic
                // may accept their same-shaped replacements, but this CLI must not adopt them.
                let prepared = Prepared::new(f).await;
                let original = snapshot(f).await;
                let mut current_valid = true;
                let mut refused = true;
                let mut unchanged = true;
                let mut restored = true;
                for index in 0..8 {
                    if index < 3 {
                        identity_helpers::replace_namespace(f, index).await;
                    } else {
                        identity_helpers::replace_marker(f, index - 3).await;
                    }
                    current_valid &= marker_helpers::current(f).await.is_ok();
                    let before = snapshot(f).await;
                    let run = prepared
                        .spawn(None)
                        .finish(None, Duration::from_secs(15))
                        .await;
                    refused &= failed(&run) && super::fixture::no_listener(prepared.bind).await;
                    unchanged &= before == snapshot(f).await;
                    if index < 3 {
                        identity_helpers::restore_namespace(f, index).await;
                    } else {
                        identity_helpers::restore_marker(f, index - 3).await;
                    }
                    restored &= original == snapshot(f).await;
                }
                let no_backend = no_connections(f).await;
                let cleaned = prepared.finish();
                (
                    current_valid,
                    refused,
                    unchanged,
                    restored,
                    no_backend,
                    cleaned,
                )
            })
        })
        .await;
    assert!(current_valid && refused && unchanged && restored && no_backend && cleaned);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL and non-root capless child support"]
async fn postgres_platform_loopback_host_occupied_bind_and_failed_storage_never_report_ready() {
    let (wrong_password, occupied, unchanged, no_backend, cleaned) = Fixture::checked(|f| {
        Box::pin(async move {
            let prepared = Prepared::new(f).await;
            let before = snapshot(f).await;
            let mut document = prepared.document.clone();
            document["database"]["password"] = "synthetic-deliberately-wrong-password".into();
            let path = prepared.variant("wrong-password.json", &document);
            // Positive authentication control: the runtime role really needs its SCRAM
            // password. The admin's local trust exception does not authorize this role.
            let wrong = prepared
                .spawn(Some(&path))
                .finish(None, Duration::from_secs(15))
                .await;
            let wrong_password = failed(&wrong) && super::fixture::no_listener(prepared.bind).await;
            let listener = TcpListener::bind(prepared.bind).unwrap();
            let run = prepared
                .spawn(None)
                .finish(None, Duration::from_secs(15))
                .await;
            let occupied = failed(&run);
            drop(listener);
            let occupied = occupied && super::fixture::no_listener(prepared.bind).await;
            let unchanged = before == snapshot(f).await;
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish();
            (wrong_password, occupied, unchanged, no_backend, cleaned)
        })
    })
    .await;
    assert!(wrong_password && occupied && unchanged && no_backend && cleaned);
}
