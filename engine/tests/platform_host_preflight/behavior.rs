use super::fixture::{within, Fixture};
use cyanrex_engine::platform_host::{inspect_platform_database, verify_platform_database};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_host_preflight_catalog_observation_is_read_only_on_empty_unprivileged_namespaces()
{
    let (first, second, same, verified, policy, unchanged, tables) = Fixture::checked(|fixture| {
        Box::pin(async move {
            let before = fixture.snapshot().await;
            let policy = fixture.runtime_is_read_only_and_unprivileged().await;
            let mut connection = within(fixture.runtime().acquire())
                .await
                .unwrap_or_else(|_| panic!("caller-owned catalog connection unavailable"));
            let first = within(inspect_platform_database(fixture.config(), &mut connection)).await;
            let second = within(inspect_platform_database(fixture.config(), &mut connection)).await;
            let same = match (&first, &second) {
                (Ok(first), Ok(second)) => {
                    first == second && first.matches(second) && second.matches(first)
                }
                _ => false,
            };
            let verified = match &first {
                Ok(observation) => {
                    within(verify_platform_database(
                        fixture.config(),
                        &mut connection,
                        observation,
                    ))
                    .await
                }
                Err(error) => Err(*error),
            };
            drop(connection);
            let after = fixture.snapshot().await;
            (
                first.map(|_| ()),
                second.map(|_| ()),
                same,
                verified,
                policy,
                before == after,
                after.tables,
            )
        })
    })
    .await;
    // Even the initial inert implementation reaches this assertion only after the owned
    // database, role and every connection have been checked and cleaned up.
    assert!(
        policy,
        "fixture must enforce read-only and zero business-table privileges"
    );
    assert!(
        unchanged,
        "catalog inspection must not install or alter storage"
    );
    assert_eq!(tables, 0);
    assert_eq!(first, Ok(()));
    assert_eq!(second, Ok(()));
    assert!(same);
    assert_eq!(verified, Ok(()));
}
