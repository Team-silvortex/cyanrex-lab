use super::{
    fixture::*,
    wait_helpers::{lock_marker, pending_marker_lock, release},
};
use cyanrex_engine::platform_host::open_platform_storage_expected;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_storage_open_rechecks_original_root_after_observed_marker_wait() {
    let (waited, swapped, released, denied, restored, recovered, unchanged, no_backend, cleaned) =
        Fixture::checked(|f| {
            Box::pin(async move {
                prepare(f).await;
                let root = Root::new();
                let catalog = Pins::read(f).await;
                let markers = MarkerPins::read(f).await;
                let original = root.original(f.config());
                let expected = markers.expectation(f, catalog);
                let endpoint = endpoint(f);
                let before = snapshot(f).await;
                let (holder, oid, pid) = lock_marker(f).await;
                let (result, (waited, swapped, released)) = tokio::join!(
                    open_platform_storage_expected(
                        f.config().clone(),
                        endpoint,
                        PASSWORD,
                        expected,
                        root.path(),
                        original
                    ),
                    async {
                        let waited = pending_marker_lock(f, oid, pid).await;
                        let swapped = waited && root.swap();
                        let released = release(holder).await;
                        (waited, swapped, released)
                    },
                );
                let denied = result.is_err();
                let restored = root.restore();
                let recovered = match open(f, &root, catalog, markers).await {
                    Ok(binding) => usable(f, &binding).await,
                    Err(_) => false,
                };
                let unchanged = before == snapshot(f).await;
                let no_backend = no_connections(f).await;
                let cleaned = root.finish();
                (
                    waited, swapped, released, denied, restored, recovered, unchanged, no_backend,
                    cleaned,
                )
            })
        })
        .await;
    assert!(
        waited
            && swapped
            && released
            && denied
            && restored
            && recovered
            && unchanged
            && no_backend
            && cleaned
    );
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_storage_open_cancelled_actual_marker_wait_leaves_no_binding_or_backend()
{
    let (waited, cancelled, released, unchanged, no_backend, cleaned) = Fixture::checked(|f| {
        Box::pin(async move {
            prepare(f).await;
            let root = Root::new();
            let catalog = Pins::read(f).await;
            let markers = MarkerPins::read(f).await;
            let expected = markers.expectation(f, catalog);
            let original = root.original(f.config());
            let path = root.path().to_owned();
            let config = f.config().clone();
            let endpoint = endpoint(f);
            let before = snapshot(f).await;
            let (holder, oid, pid) = lock_marker(f).await;
            let pending = Pending::start(async move {
                open_platform_storage_expected(
                    config, endpoint, PASSWORD, expected, &path, original,
                )
                .await
            });
            let waited = pending_marker_lock(f, oid, pid).await;
            let cancelled = pending.cancel().await;
            let released = release(holder).await;
            let no_backend = no_connections(f).await;
            let unchanged = before == snapshot(f).await;
            let cleaned = root.finish();
            (waited, cancelled, released, unchanged, no_backend, cleaned)
        })
    })
    .await;
    assert!(waited && cancelled && released && unchanged && no_backend && cleaned);
}
