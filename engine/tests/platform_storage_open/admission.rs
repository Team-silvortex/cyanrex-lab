use super::fixture::*;
use cyanrex_engine::platform_host::open_platform_storage_expected;
use std::time::Duration;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_storage_open_configuration_and_raw_root_mismatch_never_dial() {
    let (denied, no_dials, unchanged, no_backend, cleaned) = Fixture::checked(|f| {
        Box::pin(async move {
            prepare(f).await;
            let root = Root::new();
            let catalog = Pins::read(f).await;
            let markers = MarkerPins::read(f).await;
            let before = snapshot(f).await;
            let mut denied = true;
            let mut no_dials = true;
            for config in config_variants(f) {
                let (listener, endpoint) = probe().await;
                // The root matches this candidate configuration. Only the declaration differs;
                // an unrelated root mismatch must not mask a missing expected-config guard.
                let original = root.original(&config);
                let result = tokio::time::timeout(
                    Duration::from_secs(1),
                    open_platform_storage_expected(
                        config,
                        endpoint,
                        PASSWORD,
                        markers.expectation(f, catalog),
                        root.path(),
                        original,
                    ),
                )
                .await;
                denied &= result.is_ok_and(|result| result.is_err());
                no_dials &= never_dialed(listener).await;
            }
            // Conversely, a valid declaration cannot hide a root bound to another configuration.
            let other = config_variants(f).remove(0);
            let (listener, endpoint) = probe().await;
            let result = tokio::time::timeout(
                Duration::from_secs(1),
                open_platform_storage_expected(
                    f.config().clone(),
                    endpoint,
                    PASSWORD,
                    markers.expectation(f, catalog),
                    root.path(),
                    root.original(&other),
                ),
            )
            .await;
            denied &= result.is_ok_and(|result| result.is_err());
            no_dials &= never_dialed(listener).await;
            let raw = root.path().join(".");
            let (listener, endpoint) = probe().await;
            let result = tokio::time::timeout(
                Duration::from_secs(1),
                open_platform_storage_expected(
                    f.config().clone(),
                    endpoint,
                    PASSWORD,
                    markers.expectation(f, catalog),
                    &raw,
                    root.original(f.config()),
                ),
            )
            .await;
            denied &= result.is_ok_and(|result| result.is_err());
            no_dials &= never_dialed(listener).await;
            let unchanged = before == snapshot(f).await;
            let no_backend = no_connections(f).await;
            let cleaned = root.finish();
            (denied, no_dials, unchanged, no_backend, cleaned)
        })
    })
    .await;
    assert!(denied && no_dials && unchanged && no_backend && cleaned);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_storage_open_replaced_original_root_refuses_before_dial() {
    let (swapped, denied, no_dial, restored, unchanged, no_backend, cleaned) =
        Fixture::checked(|f| {
            Box::pin(async move {
                prepare(f).await;
                let root = Root::new();
                let catalog = Pins::read(f).await;
                let markers = MarkerPins::read(f).await;
                let original = root.original(f.config());
                let before = snapshot(f).await;
                let swapped = root.swap();
                let (listener, endpoint) = probe().await;
                let result = tokio::time::timeout(
                    Duration::from_secs(1),
                    open_platform_storage_expected(
                        f.config().clone(),
                        endpoint,
                        PASSWORD,
                        markers.expectation(f, catalog),
                        root.path(),
                        original,
                    ),
                )
                .await;
                let denied = result.is_ok_and(|result| result.is_err());
                let no_dial = never_dialed(listener).await;
                let restored = root.restore();
                let unchanged = before == snapshot(f).await;
                let no_backend = no_connections(f).await;
                let cleaned = root.finish();
                (
                    swapped, denied, no_dial, restored, unchanged, no_backend, cleaned,
                )
            })
        })
        .await;
    assert!(swapped && denied && no_dial && restored && unchanged && no_backend && cleaned);
}
