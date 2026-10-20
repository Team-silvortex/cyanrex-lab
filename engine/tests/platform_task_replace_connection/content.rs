use super::fixture::*;
use std::{fs, os::unix::fs::PermissionsExt};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_replace_invalid_new_pins_and_corrupt_old_content_cannot_be_hidden()
{
    for fault in [
        "foreign",
        "digest",
        "conflicting-same-coordinate",
        "scope",
        "missing-new",
        "binary-new",
        "control-new",
        "oversize-new",
        "missing-old",
        "corrupt-old",
        "binary-old",
        "artifact-outbox",
    ] {
        Fixture::checked(|f| {
            Box::pin(async move {
                let p = Prepared::new(f).await;
                let old_artifact = p
                    .artifact(
                        0,
                        if fault == "binary-old" {
                            &[0xff]
                        } else {
                            b"Original teaching notes"
                        },
                    )
                    .await;
                let task = p
                    .task_manifest(
                        10,
                        0,
                        manifest(
                            "Saved private content",
                            vec![old_artifact.reference.clone()],
                        ),
                    )
                    .await;
                let bytes = match fault {
                    "binary-new" => vec![0xff],
                    "control-new" => b"bad\0text".to_vec(),
                    "oversize-new" => vec![b'x'; 256 * 1024 + 1],
                    _ => b"New teaching notes".to_vec(),
                };
                let artifact = p.artifact(usize::from(fault == "foreign"), &bytes).await;
                let mut reference = artifact.reference.clone();
                match fault {
                    "digest" => reference.sha256 = "b".repeat(64).parse().unwrap(),
                    "conflicting-same-coordinate" => {
                        reference = old_artifact.reference.clone();
                        reference.sha256 = "c".repeat(64).parse().unwrap();
                    }
                    "scope" => {
                        reference.workspace.workspace_id = uuid::Uuid::new_v4().try_into().unwrap()
                    }
                    "missing-new" => {
                        reference.revision_id = uuid::Uuid::new_v4().try_into().unwrap()
                    }
                    "missing-old" => fs::remove_file(p.blob(&old_artifact.reference)).unwrap(),
                    "corrupt-old" => {
                        let path = p.blob(&old_artifact.reference);
                        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
                        fs::write(&path, b"Corrupted original evidence").unwrap();
                        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
                    }
                    "artifact-outbox" => {
                        f.sql(&format!(
                            "DELETE FROM {}.collaboration_artifact_outbox WHERE revision_id='{}'",
                            f.artifacts, old_artifact.reference.revision_id
                        ))
                        .await
                    }
                    _ => {}
                }
                let connector = p.connector().await;
                let before = snapshot(f).await;
                let files = p.files();
                for requested in if matches!(
                    fault,
                    "missing-old" | "corrupt-old" | "binary-old" | "artifact-outbox"
                ) {
                    vec![
                        manifest("Cannot hide by title", vec![old_artifact.reference.clone()]),
                        manifest("Cannot remove final old payload", vec![]),
                        manifest("Cannot replace corrupt old", vec![reference.clone()]),
                    ]
                } else {
                    vec![manifest(
                        "Invalid requested content",
                        vec![reference.clone()],
                    )]
                } {
                    assert!(
                        p.replace(&connector, 0, &task, requested).await.is_err(),
                        "{fault} must fail"
                    );
                }
                assert_eq!(before, snapshot(f).await);
                assert_eq!(files, p.files());
                p.unchanged(&task, 1).await;
                drop(connector);
                assert!(super::storage_helpers::no_connections(f).await);
                assert!(p.finish());
            })
        })
        .await;
    }
}
