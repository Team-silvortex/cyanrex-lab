//! SQL MVCC observations do not enter the blocked Session service or change its lock queue.
use super::{fixture::*, storage};
use cyanrex_engine::{
    models::collaboration::{ArtifactRef, ArtifactRevision},
    services::task_store::TaskContentSnapshot,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx_core::{query::query, row::Row};
use std::{fs, os::unix::fs::MetadataExt};

type FileState = (String, u64, u64, u32, u64, Vec<u8>);

pub(super) async fn task_rows(f: &Fixture) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    for table in ["collaboration_tasks", "collaboration_task_outbox"] {
        let values = within(
            query(&format!(
                "SELECT to_jsonb(t)::text AS value FROM {}.{table} t ORDER BY to_jsonb(t)::text",
                f.task
            ))
            .fetch_all(f.owner()),
        )
        .await
        .unwrap();
        rows.extend(
            values
                .into_iter()
                .map(|row| (table.to_owned(), row.get("value"))),
        );
    }
    rows
}

pub(super) async fn assert_prefix(
    p: &Prepared<'_>,
    original: &TaskContentSnapshot,
    old_rows: &[(String, String)],
    old_files: &[FileState],
    confirmed: &Value,
    pending: &Value,
) {
    let f = p.base.base.base.f;
    let reference: ArtifactRef = serde_json::from_value(confirmed.clone()).unwrap();
    let pending: ArtifactRef = serde_json::from_value(pending.clone()).unwrap();
    assert_eq!(reference.workspace, original.task.reference.workspace);
    assert_eq!(pending.workspace, reference.workspace);
    assert_ne!(reference.artifact_id, pending.artifact_id);
    assert_ne!(reference.revision_id, pending.revision_id);
    assert_eq!(
        reference.sha256.to_string(),
        format!("{:x}", Sha256::digest(SAVED_TEXTS[0].as_bytes()))
    );
    assert_eq!(
        pending.sha256.to_string(),
        format!("{:x}", Sha256::digest(SAVED_TEXTS[1].as_bytes()))
    );
    for old in original.manifest.payload() {
        assert_ne!(old.artifact().artifact_id, reference.artifact_id);
        assert_ne!(old.artifact().revision_id, reference.revision_id);
        assert_ne!(old.artifact().artifact_id, pending.artifact_id);
        assert_ne!(old.artifact().revision_id, pending.revision_id);
    }
    let rows = storage::artifact_rows(f).await;
    assert_eq!(rows.len(), old_rows.len() + 3);
    assert!(
        old_rows.iter().all(|old| rows.contains(old)),
        "old Artifact rows and outbox must remain byte-exact"
    );
    let added = rows
        .iter()
        .filter(|row| !old_rows.contains(row))
        .collect::<Vec<_>>();
    let value = |table| {
        let selected = added
            .iter()
            .filter(|(name, _)| name == table)
            .collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        serde_json::from_str::<Value>(&selected[0].1).unwrap()
    };
    let head = value("collaboration_artifacts");
    assert_eq!(
        head,
        serde_json::json!({"artifact_id":reference.artifact_id,
        "owner_id":original.task.owner.principal_id,"head_revision_id":reference.revision_id,"sequence":1})
    );
    let revision = value("collaboration_artifact_revisions");
    assert_eq!(
        revision["artifact_id"],
        serde_json::json!(reference.artifact_id)
    );
    assert_eq!(
        revision["revision_id"],
        serde_json::json!(reference.revision_id)
    );
    assert_eq!(revision["sequence"], 1);
    let raw = revision["snapshot"].as_str().unwrap();
    let artifact: ArtifactRevision = serde_json::from_str(raw).unwrap();
    assert_eq!(artifact.reference, reference);
    assert_eq!(artifact.owner, original.task.owner);
    assert_eq!(artifact.title, original.task.title);
    assert_eq!(u8::from(artifact.schema_version), 1);
    assert_eq!(artifact.kind.as_str(), "cyanrex.task-text");
    assert_eq!(artifact.media_type, "text/plain");
    assert_eq!(artifact.byte_length, SAVED_TEXTS[0].len() as u64);
    assert_eq!(u64::from(artifact.sequence), 1);
    assert_eq!(artifact.parent, None);
    let event = value("collaboration_artifact_outbox");
    assert!(!uuid::Uuid::parse_str(event["event_id"].as_str().unwrap())
        .unwrap()
        .is_nil());
    assert_eq!(
        event["artifact_id"],
        serde_json::json!(reference.artifact_id)
    );
    assert_eq!(
        event["revision_id"],
        serde_json::json!(reference.revision_id)
    );
    assert_eq!(
        event["actor_id"],
        serde_json::json!(original.task.owner.principal_id)
    );
    assert_eq!(event["sequence"], 1);
    assert_eq!(event["event_type"], "cyanrex.artifact.created");
    assert_eq!(event["snapshot"], raw);
    let files = p.files();
    assert_eq!(files.len(), old_files.len() + 1);
    assert!(
        old_files.iter().all(|old| files.contains(old)),
        "old file content, inodes and permissions must remain exact"
    );
    let path = p.blob(&reference);
    let metadata = fs::symlink_metadata(&path).unwrap();
    assert!(metadata.file_type().is_file());
    assert_eq!(metadata.uid(), p.identity.uid[0]);
    assert_eq!(metadata.mode() & 0o777, 0o400);
    assert_eq!(metadata.nlink(), 1);
    assert_eq!(fs::read(path).unwrap(), SAVED_TEXTS[0].as_bytes());
    assert!(
        !p.blob(&pending).exists(),
        "the rejected second publication must not materialize bytes"
    );
}
