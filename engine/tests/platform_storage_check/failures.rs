use super::{
    catalog_fixture::Fixture,
    marker_fixture::{prepare, snapshot},
    process::{passed, rejected, Input},
    support::{marker_state, no_connections},
};
use serde_json::json;
use std::fs;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_storage_check_rejects_empty_targets_wrong_coordinates_password_and_unsafe_roles(
) {
    let (failures, restored, unchanged, no_backend, input_unchanged) =
        Fixture::checked(|fixture| {
            Box::pin(async move {
                let input = Input::new(fixture);
                let empty = fixture.snapshot().await;
                let mut failures = vec![input.spawn(None).finish(false).await];
                let empty_unchanged = empty == fixture.snapshot().await;
                prepare(fixture).await;
                let before = snapshot(fixture).await;
                let mut no_repair = true;
                for (index, field, value) in [
                    (
                        0,
                        "system_identifier",
                        json!(fixture.config().target().system_identifier() + 1),
                    ),
                    (
                        1,
                        "database_oid",
                        json!(fixture.config().target().database_oid() + 1),
                    ),
                    (2, "password", json!("synthetic-wrong-password")),
                ] {
                    let mut document = input.document.clone();
                    document["database"][field] = value;
                    let path = input.write_variant(&format!("wrong-{index}.json"), &document);
                    let original_input = fs::read(&path).unwrap();
                    failures.push(input.spawn(Some(&path)).finish(false).await);
                    no_repair &= fs::read(&path).unwrap() == original_input;
                }
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
                for (change, restore) in changes {
                    fixture.sql(&change).await;
                    let changed = snapshot(fixture).await;
                    failures.push(input.spawn(None).finish(false).await);
                    no_repair &= changed == snapshot(fixture).await;
                    fixture.sql(&restore).await;
                }
                let restored = input.spawn(None).finish(false).await;
                let no_backend = no_connections(fixture).await;
                let unchanged = no_repair && empty_unchanged && before == snapshot(fixture).await;
                let input_unchanged = input.finish();
                (failures, restored, unchanged, no_backend, input_unchanged)
            })
        })
        .await;
    assert_eq!(failures.len(), 7);
    for run in &failures {
        rejected(run);
    }
    passed(&restored);
    assert!(unchanged && no_backend && input_unchanged);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_storage_check_rejects_marker_version_scope_permission_and_shape_without_repair(
) {
    let (failures, restored, no_repair, unchanged, no_backend, input_unchanged) = Fixture::checked(|fixture| {
        Box::pin(async move {
            prepare(fixture).await;
            let input = Input::new(fixture);
            let before = snapshot(fixture).await;
            let before_metadata = marker_state(fixture).await;
            let marker = format!("{}.collaboration_auth_source_schema", fixture.source);
            let changes = [
                (format!("UPDATE {marker} SET version=99 WHERE singleton"),
                    format!("UPDATE {marker} SET version=2 WHERE singleton")),
                (format!("UPDATE {marker} SET authority_id='00000000-0000-0000-0000-000000008219' WHERE singleton"),
                    format!("UPDATE {marker} SET authority_id='{}' WHERE singleton", fixture.config().scope().authority_id.as_uuid())),
                (format!("REVOKE SELECT ON {marker} FROM {}", fixture.role),
                    format!("GRANT SELECT ON {marker} TO {}", fixture.role)),
                (format!("ALTER TABLE {marker} ENABLE ROW LEVEL SECURITY"),
                    format!("ALTER TABLE {marker} DISABLE ROW LEVEL SECURITY")),
            ];
            let mut failures = Vec::new();
            let mut no_repair = true;
            for (change, restore) in changes {
                fixture.sql(&change).await;
                let changed_rows = snapshot(fixture).await;
                let changed_metadata = marker_state(fixture).await;
                failures.push(input.spawn(None).finish(false).await);
                no_repair &= changed_rows == snapshot(fixture).await
                    && changed_metadata == marker_state(fixture).await;
                fixture.sql(&restore).await;
            }
            let restored = input.spawn(None).finish(false).await;
            let no_backend = no_connections(fixture).await;
            let unchanged = before == snapshot(fixture).await && before_metadata == marker_state(fixture).await;
            let input_unchanged = input.finish();
            (failures, restored, no_repair, unchanged, no_backend, input_unchanged)
        })
    }).await;
    assert_eq!(failures.len(), 4);
    for run in &failures {
        rejected(run);
    }
    passed(&restored);
    assert!(no_repair && unchanged && no_backend && input_unchanged);
}
