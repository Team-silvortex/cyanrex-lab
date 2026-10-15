use super::{
    catalog_fixture::{within, Fixture},
    expectation_helpers::{config_variants, quote_identifier, Pins},
    marker_fixture::{prepare, snapshot},
    support::{
        attempt, binding, catalog_and_markers, endpoint, marker_state, no_connections, pid, Mode,
        Root, PASSWORD,
    },
};
use cyanrex_engine::platform_host::{
    inspect_platform_storage_markers, PlatformDatabaseConnector, PlatformStorageConnectionError,
    PlatformStorageConnector,
};
use sqlx_core::connection::Connection;
use std::{collections::BTreeSet, path::PathBuf};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_storage_connection_fresh_observed_and_expected_bindings_reject_mismatched_construction(
) {
    let (
        created,
        pids,
        identities,
        bad_configs,
        bad_oids,
        bad_paths,
        unchanged,
        no_backend,
        root_unchanged,
    ) = Fixture::checked(|fixture| {
        Box::pin(async move {
            prepare(fixture).await;
            let root = Root::new();
            let before = snapshot(fixture).await;
            let mut created = 0;
            let mut pids = Vec::new();
            let mut identities = true;
            for mode in [Mode::Observed, Mode::Expected] {
                if let Ok(binding) = binding(fixture, &root, mode).await {
                    created += 1;
                    let first = within(binding.connect()).await;
                    let second = within(binding.connect()).await;
                    match (first, second) {
                        (Ok(mut first), Ok(mut second)) => {
                            let a = pid(fixture, &mut first).await;
                            let b = pid(fixture, &mut second).await;
                            pids.extend([a.0, b.0]);
                            identities &= a.1 && b.1;
                            within(first.close()).await.unwrap();
                            within(second.close()).await.unwrap();
                        }
                        (first, second) => {
                            identities = false;
                            if let Ok(connection) = first {
                                within(connection.close()).await.unwrap();
                            }
                            if let Ok(connection) = second {
                                within(connection.close()).await.unwrap();
                            }
                        }
                    }
                }
            }
            let mut bad_configs = Vec::new();
            for config in config_variants(fixture) {
                let (catalog, markers) = catalog_and_markers(fixture).await;
                let database = PlatformDatabaseConnector::new(
                    fixture.config().clone(),
                    catalog,
                    endpoint(fixture),
                    PASSWORD,
                )
                .unwrap();
                bad_configs.push(
                    PlatformStorageConnector::new(
                        config,
                        database,
                        markers,
                        root.path(),
                        root.original(fixture.config()),
                    )
                    .map(|_| ()),
                );
            }
            let original = Pins::read(fixture).await;
            let mut bad_oids = Vec::new();
            for index in 0..8 {
                let (_, markers) = catalog_and_markers(fixture).await;
                let database = PlatformDatabaseConnector::with_catalog_expectation(
                    fixture.config().clone(),
                    original.wrong(index).expectation(fixture).unwrap(),
                    endpoint(fixture),
                    PASSWORD,
                )
                .unwrap();
                bad_oids.push(
                    PlatformStorageConnector::new(
                        fixture.config().clone(),
                        database,
                        markers,
                        root.path(),
                        root.original(fixture.config()),
                    )
                    .map(|_| ()),
                );
            }
            let mut bad_paths = Vec::new();
            for raw in [
                format!("{}/.", root.path().display()),
                format!("{}/", root.path().display()),
                format!("{}/../artifacts", root.path().display()),
                format!("{}//artifacts", root.path().parent().unwrap().display()),
            ] {
                let (catalog, markers) = catalog_and_markers(fixture).await;
                let database = PlatformDatabaseConnector::new(
                    fixture.config().clone(),
                    catalog,
                    endpoint(fixture),
                    PASSWORD,
                )
                .unwrap();
                bad_paths.push(
                    PlatformStorageConnector::new(
                        fixture.config().clone(),
                        database,
                        markers,
                        &PathBuf::from(raw),
                        root.original(fixture.config()),
                    )
                    .map(|_| ()),
                );
            }
            // Isolate the Observed branch: same complete config, but a genuine catalog
            // observed under another namespace owner must not match the old marker catalog.
            let (_, markers) = catalog_and_markers(fixture).await;
            let auxiliary = fixture.additional_role().await;
            let owner = quote_identifier(&fixture.owner_name().await);
            fixture
                .sql(&format!(
                    "ALTER SCHEMA {} OWNER TO {auxiliary}",
                    fixture.task
                ))
                .await;
            let changed_catalog = fixture.observe().await;
            fixture
                .sql(&format!("ALTER SCHEMA {} OWNER TO {owner}", fixture.task))
                .await;
            let database = PlatformDatabaseConnector::new(
                fixture.config().clone(),
                changed_catalog.unwrap(),
                endpoint(fixture),
                PASSWORD,
            )
            .unwrap();
            bad_oids.push(
                PlatformStorageConnector::new(
                    fixture.config().clone(),
                    database,
                    markers,
                    root.path(),
                    root.original(fixture.config()),
                )
                .map(|_| ()),
            );

            // Isolate the root's complete config binding, rather than relying on the
            // database/marker mismatch to reject an altered caller configuration first.
            for root_config in config_variants(fixture) {
                let (catalog, markers) = catalog_and_markers(fixture).await;
                let database = PlatformDatabaseConnector::new(
                    fixture.config().clone(),
                    catalog,
                    endpoint(fixture),
                    PASSWORD,
                )
                .unwrap();
                bad_paths.push(
                    PlatformStorageConnector::new(
                        fixture.config().clone(),
                        database,
                        markers,
                        root.path(),
                        root.original(&root_config),
                    )
                    .map(|_| ()),
                );
            }
            let unchanged =
                before == snapshot(fixture).await && Pins::read(fixture).await == original;
            let no_backend = no_connections(fixture).await;
            let root_unchanged = root.finish();
            (
                created,
                pids,
                identities,
                bad_configs,
                bad_oids,
                bad_paths,
                unchanged,
                no_backend,
                root_unchanged,
            )
        })
    })
    .await;
    // The inert constructor must fail only after all owned roots, connections and SQL fixtures clean up.
    assert_eq!(created, 2);
    assert_eq!(pids.len(), 4);
    assert_eq!(pids.iter().copied().collect::<BTreeSet<_>>().len(), 4);
    assert!(identities);
    assert_eq!(bad_configs.len(), 12);
    assert_eq!(bad_oids.len(), 9);
    assert_eq!(bad_paths.len(), 16);
    for result in bad_configs.into_iter().chain(bad_oids).chain(bad_paths) {
        assert_eq!(result, Err(PlatformStorageConnectionError::InvalidConfig));
    }
    assert!(unchanged && no_backend && root_unchanged);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_storage_connection_rejects_marker_replacement_before_first_call_and_later_drift(
) {
    let (first, valid_replacement, changed_identity, restored, rejected, final_restored,
        no_repair, unchanged, no_backend, root_unchanged) = Fixture::checked(|fixture| {
        Box::pin(async move {
            prepare(fixture).await;
            let root = Root::new();
            let before = snapshot(fixture).await;
            let before_metadata = marker_state(fixture).await;
            let (catalog, markers) = catalog_and_markers(fixture).await;
            let original_root = root.original(fixture.config());
            let marker = format!("{}.collaboration_auth_source_schema", fixture.source);
            let saved = format!("{}.collaboration_auth_source_schema_saved", fixture.source);
            fixture.sql(&format!("ALTER TABLE {marker} RENAME TO collaboration_auth_source_schema_saved")).await;
            fixture.sql(&format!("CREATE TABLE {marker} (singleton boolean NOT NULL PRIMARY KEY,
                version integer NOT NULL, authority_id uuid NOT NULL)")).await;
            fixture.sql(&format!("INSERT INTO {marker} VALUES (TRUE,2,'{}')",
                fixture.config().scope().authority_id.as_uuid())).await;
            fixture.sql(&format!("GRANT SELECT ON {marker} TO {}", fixture.role)).await;
            let mut connection = within(fixture.runtime().acquire()).await.unwrap();
            let current = within(inspect_platform_storage_markers(fixture.config(), &mut connection, &catalog)).await;
            let valid_replacement = current.is_ok();
            let changed_identity = current.as_ref().is_ok_and(|current| !markers.matches(current));
            drop(connection);
            let database = PlatformDatabaseConnector::new(fixture.config().clone(), catalog,
                endpoint(fixture), PASSWORD).unwrap();
            // Construct only after a valid same-name replacement exists; no marker read
            // at construction/first connect may replace the original observation.
            let binding = PlatformStorageConnector::new(fixture.config().clone(), database,
                markers, root.path(), original_root).unwrap_or_else(|_| panic!("original binding construction must remain pure"));
            let changed = snapshot(fixture).await;
            let changed_metadata = marker_state(fixture).await;
            let first = attempt(&binding).await;
            let mut no_repair = changed == snapshot(fixture).await && changed_metadata == marker_state(fixture).await;
            fixture.sql(&format!("DROP TABLE {marker}")).await;
            fixture.sql(&format!("ALTER TABLE {saved} RENAME TO collaboration_auth_source_schema")).await;
            let restored = attempt(&binding).await;
            let mut rejected = Vec::new();
            for (change, restore) in [
                (format!("UPDATE {marker} SET version=99 WHERE singleton"), format!("UPDATE {marker} SET version=2 WHERE singleton")),
                (format!("UPDATE {marker} SET authority_id='00000000-0000-0000-0000-000000009901' WHERE singleton"),
                    format!("UPDATE {marker} SET authority_id='{}' WHERE singleton", fixture.config().scope().authority_id.as_uuid())),
                (format!("REVOKE SELECT ON {marker} FROM {}", fixture.role), format!("GRANT SELECT ON {marker} TO {}", fixture.role)),
                (format!("ALTER TABLE {marker} ENABLE ROW LEVEL SECURITY"), format!("ALTER TABLE {marker} DISABLE ROW LEVEL SECURITY")),
            ] {
                fixture.sql(&change).await;
                let changed = snapshot(fixture).await;
                let changed_metadata = marker_state(fixture).await;
                rejected.push(attempt(&binding).await);
                no_repair &= changed == snapshot(fixture).await && changed_metadata == marker_state(fixture).await;
                fixture.sql(&restore).await;
            }
            let final_restored = attempt(&binding).await;
            drop(binding);
            let unchanged = before == snapshot(fixture).await && before_metadata == marker_state(fixture).await;
            let no_backend = no_connections(fixture).await;
            let root_unchanged = root.finish();
            (first, valid_replacement, changed_identity, restored, rejected, final_restored,
                no_repair, unchanged, no_backend, root_unchanged)
        })
    }).await;
    assert!(valid_replacement && changed_identity);
    assert_eq!(first, Err(PlatformStorageConnectionError::Unavailable));
    assert_eq!(restored, Ok(()));
    assert_eq!(final_restored, Ok(()));
    assert_eq!(rejected.len(), 4);
    assert!(rejected
        .into_iter()
        .all(|result| result == Err(PlatformStorageConnectionError::Unavailable)));
    assert!(no_repair && unchanged && no_backend && root_unchanged);
}
