use super::{
    fixture::{prepare, snapshot, within, Fixture},
    support::*,
};
use cyanrex_engine::platform_host::{
    inspect_platform_storage_markers, verify_platform_storage_markers, PlatformHostPreflightConfig,
    PlatformHostPreflightError as Error,
};
use sqlx_core::{query::query, row::Row};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_storage_markers_reject_missing_versions_scopes_and_catalog_dependencies()
{
    let (results, restorations, dependency_results, changed_verify, original_unchanged) = Fixture::checked(|fixture| Box::pin(async move {
        prepare(fixture).await;
        let original = catalog(fixture).await;
        let mut results = Vec::new();
        let mut restorations = Vec::new();
        for marker in markers(fixture) {
            fixture.sql(&format!("DELETE FROM {}", marker.name())).await;
            results.push((checked_inspect(fixture, &original).await, Error::UnsupportedStorageMarker));
            restore_row(fixture, &marker).await;
            let singleton_check: String = within(query("SELECT conname::text AS name FROM pg_catalog.pg_constraint
                WHERE conrelid=pg_catalog.to_regclass($1) AND contype='c' AND conkey=ARRAY[
                    (SELECT attnum FROM pg_catalog.pg_attribute WHERE attrelid=pg_catalog.to_regclass($1) AND attname='singleton')
                ]::smallint[]").bind(marker.name()).fetch_one(fixture.owner())).await
                .unwrap_or_else(|_| panic!("original singleton check identity unavailable")).get("name");
            fixture.sql(&format!("ALTER TABLE {} DROP CONSTRAINT {singleton_check}", marker.name())).await;
            fixture.sql(&format!("UPDATE {} SET singleton=FALSE", marker.name())).await;
            results.push((checked_inspect(fixture, &original).await, Error::UnsupportedStorageMarker));
            restore_row(fixture, &marker).await;
            let remaining = marker.columns().strip_prefix("singleton,").unwrap();
            fixture.sql(&format!("INSERT INTO {} ({}) SELECT FALSE,{remaining} FROM {} WHERE singleton", marker.name(), marker.columns(), marker.name())).await;
            results.push((checked_inspect(fixture, &original).await, Error::UnsupportedStorageMarker));
            restore_row(fixture, &marker).await;
            fixture.sql(&format!("ALTER TABLE {} ADD CONSTRAINT {singleton_check} CHECK (singleton)", marker.name())).await;
            fixture.sql(&format!("UPDATE {} SET version={}", marker.name(), marker.version + 1)).await;
            results.push((checked_inspect(fixture, &original).await, Error::UnsupportedStorageMarker));
            restore_row(fixture, &marker).await;
            for column in ["authority_id", "workspace_id"].into_iter().take(marker.scope_columns) {
                fixture.sql(&format!("UPDATE {} SET {column}='00000000-0000-0000-0000-000000008299'", marker.name())).await;
                results.push((checked_inspect(fixture, &original).await, Error::StorageScopeMismatch));
                restore_row(fixture, &marker).await;
            }
            let retained = format!("retained_{}", marker.table);
            fixture.sql(&format!("ALTER TABLE {} RENAME TO {retained}", marker.name())).await;
            results.push((checked_inspect(fixture, &original).await, Error::UnsupportedStorageMarker));
            fixture.sql(&format!("ALTER TABLE {}.{retained} RENAME TO {}", marker.schema, marker.table)).await;
            fixture.sql(&format!("REVOKE SELECT ON {} FROM {}", marker.name(), fixture.role)).await;
            fixture.sql(&format!("REVOKE SELECT ({}) ON {} FROM {}", marker.columns(), marker.name(), fixture.role)).await;
            results.push((checked_inspect(fixture, &original).await, Error::StorageUnavailable));
            grant_marker(fixture, &marker).await;
            restorations.push(checked_inspect(fixture, &original).await);
        }

        let config = fixture.config();
        let changed = PlatformHostPreflightConfig::new("https://different.example", config.scope(),
            config.namespaces().clone(), config.target().clone(), config.limits()).unwrap();
        let before = snapshot(fixture).await;
        let mut connection = within(fixture.runtime().acquire()).await
            .unwrap_or_else(|_| panic!("catalog dependency comparison connection unavailable"));
        let storage = within(inspect_platform_storage_markers(config, &mut connection, &original)).await
            .unwrap_or_else(|_| panic!("original storage binding observation unavailable"));
        let changed_config = within(inspect_platform_storage_markers(&changed, &mut connection, &original)).await.map(|_| ());
        let changed_verify = within(verify_platform_storage_markers(&changed, &mut connection, &storage)).await;
        drop(storage);
        drop(connection);
        let mut dependency_results = vec![changed_config];
        let schemas = [fixture.source.clone(), fixture.task.clone(), fixture.artifacts.clone()];
        for schema in &schemas {
            fixture.sql(&format!("ALTER SCHEMA {schema} RENAME TO retained_original_namespace")).await;
            fixture.sql(&format!("CREATE SCHEMA {schema}")).await;
            fixture.sql(&format!("REVOKE ALL ON SCHEMA {schema} FROM PUBLIC")).await;
            fixture.sql(&format!("GRANT USAGE ON SCHEMA {schema} TO {}", fixture.role)).await;
            // Locks must succeed so this case isolates the catalog dependency, not a missing
            // marker's independent failure. Only legitimate marker copies are installed here.
            for marker in markers(fixture).into_iter().filter(|marker| &marker.schema == schema) {
                fixture.sql(&format!("CREATE TABLE {} (LIKE retained_original_namespace.{} INCLUDING ALL)", marker.name(), marker.table)).await;
                fixture.sql(&format!("INSERT INTO {} SELECT * FROM retained_original_namespace.{}", marker.name(), marker.table)).await;
                grant_marker(fixture, &marker).await;
            }
            dependency_results.push(checked_inspect(fixture, &original).await.0);
            fixture.sql(&format!("DROP SCHEMA {schema} CASCADE")).await;
            fixture.sql(&format!("ALTER SCHEMA retained_original_namespace RENAME TO {schema}")).await;
            restorations.push(checked_inspect(fixture, &original).await);
        }
        (results, restorations, dependency_results, changed_verify, before == snapshot(fixture).await)
    })).await;
    assert_eq!(results.len(), 35);
    for ((result, unchanged), expected) in results {
        assert_eq!(result, Err(expected));
        assert!(
            unchanged,
            "rejection must never install or repair marker storage"
        );
    }
    assert_eq!(restorations.len(), 8);
    for (result, unchanged) in restorations {
        assert_eq!(result, Ok(()));
        assert!(unchanged);
    }
    assert_eq!(dependency_results.len(), 4);
    for result in dependency_results {
        assert_eq!(result, Err(Error::CatalogChanged));
    }
    assert_eq!(changed_verify, Err(Error::StorageMarkersChanged));
    assert!(original_unchanged);
}
