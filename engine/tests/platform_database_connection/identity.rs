use super::{
    catalog_fixture::Fixture,
    helpers::{attempt, connector, endpoint, quote_identifier},
};
use cyanrex_engine::platform_host::{
    PlatformDatabaseConnectionError, PlatformDatabaseConnector, PlatformHostPreflightConfig,
};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_connection_rejects_same_name_namespace_replacement_without_adoption() {
    let (mismatch, rejected, fresh_valid, changed, restored, unchanged, tables) =
        Fixture::checked(|fixture| {
            Box::pin(async move {
                let before = fixture.snapshot().await;
                let original = fixture.observe().await.unwrap();
                let mismatched = PlatformHostPreflightConfig::new(
                    "https://other.example",
                    fixture.config().scope(),
                    fixture.config().namespaces().clone(),
                    fixture.config().target().clone(),
                    fixture.config().limits(),
                )
                .unwrap();
                let mismatch = PlatformDatabaseConnector::new(
                    mismatched,
                    original,
                    endpoint(fixture),
                    "synthetic-platform-catalog-fixture-only",
                )
                .map(|_| ());
                let original = fixture.observe().await.unwrap();
                let connector = connector(fixture)
                    .await
                    .unwrap_or_else(|_| panic!("owned original connector unavailable"));
                let saved = format!("{}_saved", fixture.task);
                fixture
                    .sql(&format!("ALTER SCHEMA {} RENAME TO {saved}", fixture.task))
                    .await;
                fixture
                    .sql(&format!("CREATE SCHEMA {}", fixture.task))
                    .await;
                fixture
                    .sql(&format!(
                        "REVOKE ALL ON SCHEMA {} FROM PUBLIC",
                        fixture.task
                    ))
                    .await;
                fixture
                    .sql(&format!(
                        "GRANT USAGE ON SCHEMA {} TO {}",
                        fixture.task, fixture.role
                    ))
                    .await;
                // The replacement is independently valid, but validity must never replace
                // the exact namespace OID retained by the original connector.
                let replacement = fixture.observe().await;
                let fresh_valid = replacement.is_ok();
                let changed = replacement
                    .as_ref()
                    .is_ok_and(|value| !original.matches(value));
                let rejected = attempt(&connector).await;
                fixture.sql(&format!("DROP SCHEMA {}", fixture.task)).await;
                fixture
                    .sql(&format!("ALTER SCHEMA {saved} RENAME TO {}", fixture.task))
                    .await;
                let restored = attempt(&connector).await;
                let after = fixture.snapshot().await;
                (
                    mismatch,
                    rejected,
                    fresh_valid,
                    changed,
                    restored,
                    before == after,
                    after.tables,
                )
            })
        })
        .await;
    assert_eq!(
        mismatch,
        Err(PlatformDatabaseConnectionError::InvalidConfig)
    );
    assert!(
        fresh_valid,
        "a valid replacement is not the original target"
    );
    assert!(
        changed,
        "the independent observation must expose the changed OID"
    );
    assert_eq!(rejected, Err(PlatformDatabaseConnectionError::Unavailable));
    assert_eq!(restored, Ok(()));
    assert!(unchanged);
    assert_eq!(tables, 0);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_connection_rejects_namespace_owner_drift_until_exact_restoration() {
    let (rejected, fresh_valid, changed, restored, unchanged, tables) =
        Fixture::checked(|fixture| {
            Box::pin(async move {
                let auxiliary = fixture.additional_role().await;
                let owner = fixture.owner_name().await;
                let before = fixture.snapshot().await;
                let original = fixture.observe().await.unwrap();
                let connector = connector(fixture)
                    .await
                    .unwrap_or_else(|_| panic!("owned original connector unavailable"));
                fixture
                    .sql(&format!(
                        "ALTER SCHEMA {} OWNER TO {auxiliary}",
                        fixture.task
                    ))
                    .await;
                let changed_observation = fixture.observe().await;
                let fresh_valid = changed_observation.is_ok();
                let changed = changed_observation
                    .as_ref()
                    .is_ok_and(|value| !original.matches(value));
                let rejected = attempt(&connector).await;
                fixture
                    .sql(&format!(
                        "ALTER SCHEMA {} OWNER TO {}",
                        fixture.task,
                        quote_identifier(&owner)
                    ))
                    .await;
                let restored = attempt(&connector).await;
                let after = fixture.snapshot().await;
                (
                    rejected,
                    fresh_valid,
                    changed,
                    restored,
                    before == after,
                    after.tables,
                )
            })
        })
        .await;
    assert!(
        fresh_valid,
        "owner drift remains valid only as a new observation"
    );
    assert!(
        changed,
        "namespace owner identity is part of the original target"
    );
    assert_eq!(rejected, Err(PlatformDatabaseConnectionError::Unavailable));
    assert_eq!(restored, Ok(()));
    assert!(unchanged);
    assert_eq!(tables, 0);
}
