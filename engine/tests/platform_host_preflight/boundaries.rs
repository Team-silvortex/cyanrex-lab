use super::fixture::{within, Fixture};
use cyanrex_engine::platform_host::{
    inspect_platform_database, PlatformDatabaseTarget, PlatformHostPreflightConfig,
    PlatformHostPreflightError as Error,
};
use sqlx_core::{connection::Connection, query::query, row::Row};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_host_preflight_rejects_wrong_targets_outer_transactions_and_missing_catalog_permission(
) {
    let (results, callers_unchanged, healthy) = Fixture::checked(|fixture| Box::pin(async move {
        let mut results = Vec::new();
        let mut callers_unchanged = true;
        let original = fixture.config().clone();
        let target = original.target();
        let targets = [
            PlatformDatabaseTarget::new("another_database", target.role(), target.system_identifier(), target.database_oid()).unwrap(),
            PlatformDatabaseTarget::new(target.database(), "another_role", target.system_identifier(), target.database_oid()).unwrap(),
            PlatformDatabaseTarget::new(target.database(), target.role(), target.system_identifier().wrapping_add(1).max(1), target.database_oid()).unwrap(),
            PlatformDatabaseTarget::new(target.database(), target.role(), target.system_identifier(), target.database_oid().wrapping_add(1).max(1)).unwrap(),
        ];
        for target in targets {
            let config = PlatformHostPreflightConfig::new(original.origin(), original.scope(),
                original.namespaces().clone(), target, original.limits()).unwrap();
            let before = fixture.snapshot().await;
            let mut connection = within(fixture.runtime().acquire()).await
                .unwrap_or_else(|_| panic!("target boundary connection unavailable"));
            let result = within(inspect_platform_database(&config, &mut connection)).await.map(|_| ());
            drop(connection);
            callers_unchanged &= before == fixture.snapshot().await;
            results.push((result, Error::TargetMismatch));
        }

        let auxiliary = fixture.additional_role().await;
        fixture.sql(&format!("GRANT {auxiliary} TO {}", fixture.role)).await;
        let mut connection = within(fixture.runtime().acquire()).await
            .unwrap_or_else(|_| panic!("SET ROLE boundary connection unavailable"));
        within(query(&format!("SET ROLE {auxiliary}")).execute(&mut *connection)).await
            .unwrap_or_else(|_| panic!("owned SET ROLE setup failed"));
        let result = within(inspect_platform_database(&original, &mut connection)).await.map(|_| ());
        let role_preserved: bool = within(query("SELECT current_user <> session_user AS preserved")
            .fetch_one(&mut *connection)).await.unwrap_or_else(|_| panic!("SET ROLE oracle failed")).get("preserved");
        callers_unchanged &= role_preserved;
        results.push((result, Error::UnsafeRole));
        within(query("RESET ROLE").execute(&mut *connection)).await.unwrap_or_else(|_| panic!("owned role reset failed"));
        drop(connection);
        fixture.sql(&format!("REVOKE {auxiliary} FROM {}", fixture.role)).await;

        let mut connection = within(fixture.runtime().acquire()).await
            .unwrap_or_else(|_| panic!("outer transaction boundary connection unavailable"));
        {
            let mut outer = within(connection.begin()).await.unwrap_or_else(|_| panic!("caller managed transaction setup failed"));
            within(query("SET LOCAL application_name = 'catalog-caller-outer'").execute(&mut *outer)).await.unwrap_or_else(|_| panic!("caller managed marker setup failed"));
            let result = within(inspect_platform_database(&original, &mut outer)).await.map(|_| ());
            let kept: bool = within(query("SELECT current_setting('application_name') = 'catalog-caller-outer' AS kept")
                .fetch_one(&mut *outer)).await.unwrap_or_else(|_| panic!("caller managed transaction oracle failed")).get("kept");
            callers_unchanged &= kept;
            results.push((result, Error::InvalidConfig));
            within(outer.rollback()).await.unwrap_or_else(|_| panic!("caller managed rollback failed"));
        }
        // Raw BEGIN deliberately bypasses SQLx's transaction-depth counter. Establish the
        // caller's actual server snapshot before inspection; it must neither nest nor commit it.
        for mode in ["READ ONLY", "READ WRITE"] {
            within(query(&format!("BEGIN ISOLATION LEVEL REPEATABLE READ {mode}")).execute(&mut *connection)).await.unwrap_or_else(|_| panic!("caller raw transaction setup failed"));
            within(query("SET LOCAL application_name = 'catalog-caller-raw'").execute(&mut *connection)).await.unwrap_or_else(|_| panic!("caller raw marker setup failed"));
            let snapshot: String = within(query("SELECT txid_current_snapshot()::text AS snapshot")
                .fetch_one(&mut *connection)).await.unwrap_or_else(|_| panic!("caller raw snapshot setup failed")).get("snapshot");
            within(query("SAVEPOINT cyanrex_platform_preflight_probe").execute(&mut *connection)).await.unwrap_or_else(|_| panic!("caller raw savepoint setup failed"));
            let result = within(inspect_platform_database(&original, &mut connection)).await.map(|_| ());
            let row = within(query("SELECT current_setting('application_name') = 'catalog-caller-raw' AS marker,
                current_setting('transaction_isolation') = 'repeatable read' AS isolation,
                txid_current_snapshot()::text AS snapshot")
                .fetch_one(&mut *connection)).await.unwrap_or_else(|_| panic!("caller raw transaction oracle failed"));
            callers_unchanged &= row.get::<bool, _>("marker") && row.get::<bool, _>("isolation")
                && row.get::<String, _>("snapshot") == snapshot;
            results.push((result, Error::InvalidConfig));
            within(query("ROLLBACK TO SAVEPOINT cyanrex_platform_preflight_probe").execute(&mut *connection)).await.unwrap_or_else(|_| panic!("caller original same-named savepoint was lost"));
            within(query("ROLLBACK").execute(&mut *connection)).await.unwrap_or_else(|_| panic!("caller raw rollback failed"));
        }
        within(query("BEGIN READ ONLY").execute(&mut *connection)).await.unwrap_or_else(|_| panic!("caller aborted transaction setup failed"));
        let division = within(query("SELECT 1 / 0").execute(&mut *connection)).await;
        let actual_division_failure = division.as_ref().err().and_then(|error| error.as_database_error())
            .is_some_and(|error| error.code().as_deref() == Some("22012"));
        let result = within(inspect_platform_database(&original, &mut connection)).await.map(|_| ());
        let after = within(query("SELECT 1").execute(&mut *connection)).await;
        let still_aborted = after.as_ref().err().and_then(|error| error.as_database_error())
            .is_some_and(|error| error.code().as_deref() == Some("25P02"));
        callers_unchanged &= actual_division_failure && still_aborted;
        results.push((result, Error::InvalidConfig));
        within(query("ROLLBACK").execute(&mut *connection)).await.unwrap_or_else(|_| panic!("caller aborted rollback failed"));
        drop(connection);

        fixture.sql(&format!("REVOKE EXECUTE ON FUNCTION pg_catalog.pg_control_system() FROM {}", fixture.role)).await;
        let (result, unchanged) = fixture.inspect_unchanged().await;
        callers_unchanged &= unchanged;
        results.push((result, Error::StorageUnavailable));
        fixture.sql(&format!("GRANT EXECUTE ON FUNCTION pg_catalog.pg_control_system() TO {}", fixture.role)).await;
        let healthy = fixture.inspect_unchanged().await;
        (results, callers_unchanged, healthy)
    })).await;
    assert!(
        callers_unchanged,
        "preflight must not alter caller transactions, identity or catalogs"
    );
    assert_eq!(results.len(), 10);
    for (actual, expected) in results {
        assert_eq!(actual, Err(expected));
    }
    assert_eq!(healthy.0, Ok(()));
    assert!(healthy.1);
}
