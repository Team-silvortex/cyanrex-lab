use super::fixture::*;
use std::{fs, os::unix::fs::PermissionsExt};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_content_corrupt_metadata_or_exact_bytes_never_returns_partial_content(
) {
    for fault in [
        "task-outbox",
        "artifact-revision",
        "artifact-outbox",
        "missing",
        "corrupt",
        "permissions",
        "digest",
        "foreign-owner",
        "scope",
        "binary",
    ] {
        Fixture::checked(|f| Box::pin(async move {
            let p = Prepared::new(f).await;
            let first = p.artifact(0, b"Valid first private payload").await;
            let last = p.artifact(usize::from(fault == "foreign-owner"), if fault == "binary" { &[0xff] } else { b"Later exact private payload" }).await;
            let mut reference = last.reference.clone();
            if fault == "digest" { reference.sha256 = "b".repeat(64).parse().unwrap(); }
            let task = p.task_manifest(10, 0, manifest("全部校验后才重开", vec![first.reference, reference])).await;
            let connector = p.connector().await;
            if !matches!(fault, "digest" | "binary" | "foreign-owner") { assert!(p.read(&connector, 0, &task).await.unwrap().is_some()); }
            match fault {
                "task-outbox" => f.sql(&format!("UPDATE {}.collaboration_task_outbox SET manifest='{{}}'", f.task)).await,
                "artifact-revision" => f.sql(&format!("UPDATE {}.collaboration_artifact_revisions SET snapshot='{{}}' WHERE revision_id='{}'", f.artifacts, last.reference.revision_id)).await,
                "artifact-outbox" => f.sql(&format!("DELETE FROM {}.collaboration_artifact_outbox WHERE revision_id='{}'", f.artifacts, last.reference.revision_id)).await,
                "missing" => fs::remove_file(p.blob(&last.reference)).unwrap(),
                "corrupt" => {
                    let path = p.blob(&last.reference);
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
                    fs::write(&path, b"Different immutable bytes!").unwrap();
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
                }
                "permissions" => fs::set_permissions(p.blob(&last.reference), fs::Permissions::from_mode(0o600)).unwrap(),
                "scope" => {
                    let foreign = uuid::Uuid::new_v4();
                    for table in ["collaboration_tasks", "collaboration_task_outbox"] {
                        f.sql(&format!("UPDATE {}.{table} SET manifest=jsonb_set(manifest::jsonb,'{{payload,0,artifact,workspace,workspace_id}}','\"{foreign}\"')::text,
                            snapshot=jsonb_set(snapshot::jsonb,'{{input_refs,0,workspace,workspace_id}}','\"{foreign}\"')::text", f.task)).await;
                    }
                }
                _ => {}
            }
            assert!(p.read(&connector, 0, &task).await.is_err(), "fault {fault} must reject the entire private content result");
            drop(connector);
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })).await;
    }
}
