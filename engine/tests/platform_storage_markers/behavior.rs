use super::fixture::{
    forbidden_reads_are_denied, only_marker_columns_are_readable, prepare, snapshot, within,
    Fixture,
};
use cyanrex_engine::platform_host::{
    inspect_platform_database, inspect_platform_storage_markers, verify_platform_storage_markers,
};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_storage_markers_observe_real_templates_with_only_marker_column_permissions(
) {
    let (first, second, matches, verified, readonly, columns, denied, unchanged) =
        Fixture::checked(|fixture| {
            Box::pin(async move {
                prepare(fixture).await;
                let before = snapshot(fixture).await;
                let readonly = fixture.runtime_is_read_only_and_unprivileged().await;
                let columns = only_marker_columns_are_readable(fixture).await;
                let denied = forbidden_reads_are_denied(fixture).await;
                let mut connection =
                    within(fixture.runtime().acquire())
                        .await
                        .unwrap_or_else(|_| {
                            panic!("caller-owned marker observation connection unavailable")
                        });
                let catalog = within(inspect_platform_database(fixture.config(), &mut connection))
                    .await
                    .unwrap_or_else(|_| panic!("original safe catalog observation unavailable"));
                let first = within(inspect_platform_storage_markers(
                    fixture.config(),
                    &mut connection,
                    &catalog,
                ))
                .await;
                let second = within(inspect_platform_storage_markers(
                    fixture.config(),
                    &mut connection,
                    &catalog,
                ))
                .await;
                let matches = match (&first, &second) {
                    (Ok(first), Ok(second)) => first.matches(second) && second.matches(first),
                    _ => false,
                };
                let verified = match &first {
                    Ok(observation) => {
                        within(verify_platform_storage_markers(
                            fixture.config(),
                            &mut connection,
                            observation,
                        ))
                        .await
                    }
                    Err(error) => Err(*error),
                };
                let first = first.map(|_| ());
                let second = second.map(|_| ());
                drop(catalog);
                drop(connection);
                let unchanged = before == snapshot(fixture).await;
                (
                    first, second, matches, verified, readonly, columns, denied, unchanged,
                )
            })
        })
        .await;
    // The owning legacy fixture closes both pools and removes only its own database/role
    // before any behavior assertions, including the initial inert implementation's failure.
    assert!(readonly && columns);
    assert_eq!(denied.len(), 7);
    assert!(denied.into_iter().all(|denied| denied));
    assert!(
        unchanged,
        "marker inspection must not alter stored markers, business rows or catalog identities"
    );
    assert_eq!(first, Ok(()));
    assert_eq!(second, Ok(()));
    assert!(matches);
    assert_eq!(verified, Ok(()));
}
