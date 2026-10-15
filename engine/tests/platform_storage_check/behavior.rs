use super::{
    catalog_fixture::Fixture,
    marker_fixture::{
        forbidden_reads_are_denied, only_marker_columns_are_readable, prepare, snapshot,
    },
    process::{passed, Input},
    support::no_connections,
};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_storage_check_observes_real_markers_and_root_without_mutation_or_leftover_backend(
) {
    let (run, policy, denied, unchanged, no_backend, input_unchanged) =
        Fixture::checked(|fixture| {
            Box::pin(async move {
                prepare(fixture).await;
                let input = Input::new(fixture);
                let before = snapshot(fixture).await;
                let policy = only_marker_columns_are_readable(fixture).await;
                let denied = forbidden_reads_are_denied(fixture).await;
                let run = input.spawn(None).finish(false).await;
                let no_backend = no_connections(fixture).await;
                let unchanged = before == snapshot(fixture).await;
                let input_unchanged = input.finish();
                (run, policy, denied, unchanged, no_backend, input_unchanged)
            })
        })
        .await;
    // All direct children, owned files, database/role and fixture pools are cleaned first.
    passed(&run);
    assert!(policy && denied.len() == 7 && denied.into_iter().all(|value| value));
    assert!(unchanged && no_backend && input_unchanged);
}
