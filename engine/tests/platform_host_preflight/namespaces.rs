use super::{
    config::scope,
    fixture::{within, Fixture},
};
use cyanrex_engine::{
    platform_host::{
        inspect_platform_database, verify_platform_database, PlatformDatabaseTarget,
        PlatformHostPreflightConfig, PlatformHostPreflightError as Error, PlatformNamespaces,
    },
    services::auth_service::durable_source::SessionStockLimits,
};
use sqlx_core::{query::query, row::Row};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_host_preflight_rejects_unsafe_namespaces_and_invalidates_original_catalog_observations(
) {
    let (unsafe_results, changed_results, owner_changes, replacement, healthy) = Fixture::checked(|fixture| Box::pin(async move {
        let mut unsafe_results = Vec::new();
        let owner = fixture.owner_name().await.replace('"', "\"\"");
        let schemas = [fixture.source.clone(), fixture.task.clone(), fixture.artifacts.clone()];
        for schema in &schemas {
            fixture.sql(&format!("GRANT CREATE ON SCHEMA {schema} TO {}", fixture.role)).await;
            unsafe_results.push(fixture.inspect_unchanged().await);
            fixture.sql(&format!("REVOKE CREATE ON SCHEMA {schema} FROM {}", fixture.role)).await;
            fixture.sql(&format!("ALTER SCHEMA {schema} OWNER TO {}", fixture.role)).await;
            unsafe_results.push(fixture.inspect_unchanged().await);
            fixture.sql(&format!("ALTER SCHEMA {schema} OWNER TO \"{owner}\"")).await;
            fixture.sql(&format!("GRANT USAGE ON SCHEMA {schema} TO {}", fixture.role)).await;
            fixture.sql(&format!("REVOKE USAGE ON SCHEMA {schema} FROM {}", fixture.role)).await;
            unsafe_results.push(fixture.inspect_unchanged().await);
            fixture.sql(&format!("GRANT USAGE ON SCHEMA {schema} TO {}", fixture.role)).await;
            fixture.sql(&format!("ALTER SCHEMA {schema} RENAME TO absent_original_namespace")).await;
            unsafe_results.push(fixture.inspect_unchanged().await);
            fixture.sql(&format!("ALTER SCHEMA absent_original_namespace RENAME TO {schema}")).await;
        }

        let mut connection = within(fixture.runtime().acquire()).await.unwrap_or_else(|_| panic!("path observation connection unavailable"));
        for path in [format!("{}, {}", fixture.source, fixture.task), fixture.task.clone(),
            "public".into(), format!("pg_catalog, {}", fixture.source), "".into()] {
            within(query("SELECT pg_catalog.set_config('search_path', $1, false)")
                .bind(&path).execute(&mut *connection)).await.unwrap_or_else(|_| panic!("owned caller path setup failed"));
            let result = within(inspect_platform_database(fixture.config(), &mut connection)).await.map(|_| ());
            let kept: String = within(query("SELECT current_setting('search_path') AS path")
                .fetch_one(&mut *connection)).await.unwrap_or_else(|_| panic!("caller path oracle failed")).get("path");
            unsafe_results.push((result, kept == path));
        }
        within(query("SELECT pg_catalog.set_config('search_path', $1, false)")
            .bind(&fixture.source).execute(&mut *connection)).await.unwrap_or_else(|_| panic!("caller path restore failed"));
        // TEMP privilege is removed again before inspection. The remaining actual temporary
        // namespace, not an unsafe current grant, must make this caller connection unsuitable.
        fixture.sql(&format!("GRANT TEMP ON DATABASE {} TO {}", fixture.database, fixture.role)).await;
        within(query("SET default_transaction_read_only = off").execute(&mut *connection)).await.unwrap_or_else(|_| panic!("owned temporary namespace setup failed"));
        within(query("CREATE TEMP TABLE catalog_temporary_marker (n integer)").execute(&mut *connection)).await.unwrap_or_else(|_| panic!("owned temporary marker setup failed"));
        fixture.sql(&format!("REVOKE TEMP ON DATABASE {} FROM {}", fixture.database, fixture.role)).await;
        within(query("SET default_transaction_read_only = on").execute(&mut *connection)).await.unwrap_or_else(|_| panic!("caller read-only setting restore failed"));
        let result = within(inspect_platform_database(fixture.config(), &mut connection)).await.map(|_| ());
        let temporary_kept: bool = within(query("SELECT pg_my_temp_schema() <> 0 AND to_regclass('pg_temp.catalog_temporary_marker') IS NOT NULL AS kept")
            .fetch_one(&mut *connection)).await.unwrap_or_else(|_| panic!("temporary namespace oracle failed")).get("kept");
        unsafe_results.push((result, temporary_kept));
        within(connection.close()).await.unwrap_or_else(|_| panic!("temporary namespace connection cleanup failed"));

        let original = fixture.observe().await
            .unwrap_or_else(|_| panic!("healthy original catalog observation unavailable"));
        let original_config = fixture.config().clone();
        let config = &original_config;
        let trusted_owner = fixture.additional_role().await;
        let mut owner_changes = Vec::new();
        for schema in &schemas {
            fixture.sql(&format!("ALTER SCHEMA {schema} OWNER TO {trusted_owner}")).await;
            let before = fixture.snapshot().await;
            let mut connection = within(fixture.runtime().acquire()).await
                .unwrap_or_else(|_| panic!("schema owner observation connection unavailable"));
            let verified = within(verify_platform_database(config, &mut connection, &original)).await;
            let fresh = within(inspect_platform_database(config, &mut connection)).await;
            let changed = fresh.as_ref().is_ok_and(|fresh| !original.matches(fresh));
            drop(connection);
            owner_changes.push((verified, changed, before == fixture.snapshot().await));
            fixture.sql(&format!("ALTER SCHEMA {schema} OWNER TO \"{owner}\"")).await;
            fixture.sql(&format!("GRANT USAGE ON SCHEMA {schema} TO {}", fixture.role)).await;
        }
        let other_scope = cyanrex_engine::models::collaboration::WorkspaceRef {
            authority_id: Uuid::from_u128(4203).try_into().unwrap(), ..scope()
        };
        let other_workspace = cyanrex_engine::models::collaboration::WorkspaceRef {
            workspace_id: Uuid::from_u128(4204).try_into().unwrap(), ..scope()
        };
        let changed = [
            PlatformHostPreflightConfig::new("https://another.example", config.scope(), config.namespaces().clone(), config.target().clone(), config.limits()).unwrap(),
            PlatformHostPreflightConfig::new(config.origin(), other_scope, config.namespaces().clone(), config.target().clone(), config.limits()).unwrap(),
            PlatformHostPreflightConfig::new(config.origin(), other_workspace, config.namespaces().clone(), config.target().clone(), config.limits()).unwrap(),
            PlatformHostPreflightConfig::new(config.origin(), config.scope(), PlatformNamespaces::new("another_source", &fixture.task, &fixture.artifacts).unwrap(), config.target().clone(), config.limits()).unwrap(),
            PlatformHostPreflightConfig::new(config.origin(), config.scope(), PlatformNamespaces::new(&fixture.source, "another_task", &fixture.artifacts).unwrap(), config.target().clone(), config.limits()).unwrap(),
            PlatformHostPreflightConfig::new(config.origin(), config.scope(), PlatformNamespaces::new(&fixture.source, &fixture.task, "another_artifacts").unwrap(), config.target().clone(), config.limits()).unwrap(),
            PlatformHostPreflightConfig::new(config.origin(), config.scope(), config.namespaces().clone(), PlatformDatabaseTarget::new("another_database", config.target().role(), config.target().system_identifier(), config.target().database_oid()).unwrap(), config.limits()).unwrap(),
            PlatformHostPreflightConfig::new(config.origin(), config.scope(), config.namespaces().clone(), PlatformDatabaseTarget::new(config.target().database(), "another_role", config.target().system_identifier(), config.target().database_oid()).unwrap(), config.limits()).unwrap(),
            PlatformHostPreflightConfig::new(config.origin(), config.scope(), config.namespaces().clone(), PlatformDatabaseTarget::new(config.target().database(), config.target().role(), config.target().system_identifier().wrapping_add(1).max(1), config.target().database_oid()).unwrap(), config.limits()).unwrap(),
            PlatformHostPreflightConfig::new(config.origin(), config.scope(), config.namespaces().clone(), PlatformDatabaseTarget::new(config.target().database(), config.target().role(), config.target().system_identifier(), config.target().database_oid().wrapping_add(1).max(1)).unwrap(), config.limits()).unwrap(),
            PlatformHostPreflightConfig::new(config.origin(), config.scope(), config.namespaces().clone(), config.target().clone(), SessionStockLimits::new(1, 8).unwrap()).unwrap(),
            PlatformHostPreflightConfig::new(config.origin(), config.scope(), config.namespaces().clone(), config.target().clone(), SessionStockLimits::new(2, 9).unwrap()).unwrap(),
        ];
        let mut changed_results = Vec::new();
        let mut connection = within(fixture.runtime().acquire()).await.unwrap_or_else(|_| panic!("configuration comparison connection unavailable"));
        for config in changed {
            changed_results.push(within(verify_platform_database(&config, &mut connection, &original)).await);
        }
        drop(connection);
        // A name restored to a healthy, same-policy empty schema must still have a new
        // physical identity. Never replace the original observation with that fresh observation.
        let old_oid: i64 = within(query("SELECT oid::bigint AS oid FROM pg_catalog.pg_namespace WHERE nspname = $1")
            .bind(&fixture.source).fetch_one(fixture.owner())).await.unwrap_or_else(|_| panic!("original namespace identity unavailable")).get("oid");
        fixture.sql(&format!("ALTER SCHEMA {} RENAME TO retained_old_source", fixture.source)).await;
        fixture.sql(&format!("CREATE SCHEMA {}", fixture.source)).await;
        fixture.sql(&format!("REVOKE ALL ON SCHEMA {} FROM PUBLIC", fixture.source)).await;
        fixture.sql(&format!("GRANT USAGE ON SCHEMA {} TO {}", fixture.source, fixture.role)).await;
        let new_oid: i64 = within(query("SELECT oid::bigint AS oid FROM pg_catalog.pg_namespace WHERE nspname = $1")
            .bind(&fixture.source).fetch_one(fixture.owner())).await.unwrap_or_else(|_| panic!("replacement namespace identity unavailable")).get("oid");
        let before = fixture.snapshot().await;
        let mut connection = within(fixture.runtime().acquire()).await.unwrap_or_else(|_| panic!("replacement comparison connection unavailable"));
        let replaced = within(verify_platform_database(&original_config, &mut connection, &original)).await;
        let fresh = within(inspect_platform_database(&original_config, &mut connection)).await;
        let differs = fresh.as_ref().is_ok_and(|fresh| !original.matches(fresh) && original != *fresh);
        drop(connection);
        let replacement = (replaced, old_oid != new_oid, differs, before == fixture.snapshot().await);
        let healthy = fixture.inspect_unchanged().await;
        (unsafe_results, changed_results, owner_changes, replacement, healthy)
    })).await;
    assert_eq!(unsafe_results.len(), 18);
    for (result, unchanged) in unsafe_results {
        assert_eq!(result, Err(Error::UnsafeNamespace));
        assert!(
            unchanged,
            "preflight must not reset or repair the caller's catalog state"
        );
    }
    assert_eq!(changed_results.len(), 12);
    for result in changed_results {
        assert_eq!(result, Err(Error::CatalogChanged));
    }
    assert_eq!(owner_changes.len(), 3);
    for (result, changed, unchanged) in owner_changes {
        assert_eq!(result, Err(Error::CatalogChanged));
        assert!(changed && unchanged);
    }
    assert_eq!(replacement.0, Err(Error::CatalogChanged));
    assert!(replacement.1 && replacement.2 && replacement.3);
    assert_eq!(healthy.0, Ok(()));
    assert!(healthy.1);
}
