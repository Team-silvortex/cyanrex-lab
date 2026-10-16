use super::{
    catalog_fixture::Fixture,
    expectation_helpers::Pins,
    marker_fixture::{
        forbidden_reads_are_denied, only_marker_columns_are_readable, prepare, snapshot,
    },
    support::{current, no_connections, observe, MarkerPins},
};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_marker_expectation_observes_real_original_markers_without_adoption_or_mutation(
) {
    let (first, second, genuine, policy, denied, unchanged, no_backend) =
        Fixture::checked(|fixture| {
            Box::pin(async move {
                prepare(fixture).await;
                // Original identities come from independent administrator catalog queries,
                // before any role observer can become a first-read accepted baseline.
                let catalog = Pins::read(fixture).await;
                let markers = MarkerPins::read(fixture).await;
                let expected = markers.expectation(fixture, catalog);
                let before = snapshot(fixture).await;
                let policy = only_marker_columns_are_readable(fixture).await;
                let denied = forbidden_reads_are_denied(fixture).await;
                let first = observe(fixture, &expected).await;
                let second = observe(fixture, &expected).await;
                let old = current(fixture).await;
                let genuine = match (&first, &second, &old) {
                    (Ok(a), Ok(b), Ok(c)) => a.matches(b) && a.matches(c),
                    _ => false,
                };
                let unchanged = before == snapshot(fixture).await
                    && catalog == Pins::read(fixture).await
                    && markers == MarkerPins::read(fixture).await;
                let no_backend = no_connections(fixture).await;
                (
                    first.map(|_| ()),
                    second.map(|_| ()),
                    genuine,
                    policy,
                    denied,
                    unchanged,
                    no_backend,
                )
            })
        })
        .await;
    // The behavior Red must occur after every owned connection, database and role is cleaned.
    assert!(policy && denied.len() == 7 && denied.into_iter().all(|value| value));
    assert_eq!(first, Ok(()));
    assert_eq!(second, Ok(()));
    assert!(genuine && unchanged && no_backend);
}
