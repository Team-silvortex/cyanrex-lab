use super::{
    catalog_fixture::Fixture,
    helpers::{attempt, connector},
};
use cyanrex_engine::platform_host::PlatformDatabaseConnectionError;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_connection_rechecks_unsafe_privileges_and_control_permission_each_time()
{
    let (results, restored, unchanged, tables) = Fixture::checked(|fixture| {
        Box::pin(async move {
            let before = fixture.snapshot().await;
            let connector = connector(fixture)
                .await
                .unwrap_or_else(|_| panic!("owned original connector unavailable"));
            let changes = [
                (
                    format!(
                        "GRANT CREATE ON SCHEMA {} TO {}",
                        fixture.source, fixture.role
                    ),
                    format!(
                        "REVOKE CREATE ON SCHEMA {} FROM {}",
                        fixture.source, fixture.role
                    ),
                ),
                (
                    format!(
                        "GRANT TEMP ON DATABASE {} TO {}",
                        fixture.database, fixture.role
                    ),
                    format!(
                        "REVOKE TEMP ON DATABASE {} FROM {}",
                        fixture.database, fixture.role
                    ),
                ),
                (
                    format!(
                        "REVOKE EXECUTE ON FUNCTION pg_catalog.pg_control_system() FROM {}",
                        fixture.role
                    ),
                    format!(
                        "GRANT EXECUTE ON FUNCTION pg_catalog.pg_control_system() TO {}",
                        fixture.role
                    ),
                ),
            ];
            let mut results = Vec::new();
            let mut restored = Vec::new();
            for (change, restore) in changes {
                fixture.sql(&change).await;
                results.push(attempt(&connector).await);
                // Even an unexpectedly successful connect is closed by attempt before
                // restoration and before the owned database cleanup starts.
                fixture.sql(&restore).await;
                restored.push(attempt(&connector).await);
            }
            let after = fixture.snapshot().await;
            (results, restored, before == after, after.tables)
        })
    })
    .await;
    assert_eq!(results.len(), 3);
    assert!(results
        .into_iter()
        .all(|value| value == Err(PlatformDatabaseConnectionError::Unavailable)));
    assert_eq!(restored, vec![Ok(()), Ok(()), Ok(())]);
    assert!(
        unchanged,
        "permission rejection must not repair or adopt privileges"
    );
    assert_eq!(tables, 0);
}
