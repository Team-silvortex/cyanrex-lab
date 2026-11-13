use super::{browser_fixture, fixture::*};
use cyanrex_engine::{
    models::collaboration::ArtifactRevision, services::task_store::TaskContentSnapshot,
};
use sha2::{Digest, Sha256};
use sqlx_core::{query::query, row::Row};
use std::{fs, os::unix::fs::MetadataExt};

type FileState = (String, u64, u64, u32, u64, Vec<u8>);

pub(super) async fn immutable_rows(f: &Fixture) -> Vec<(String, String)> {
    let mut rows = browser_fixture::resources(f).await;
    for table in ["collaboration_tasks", "collaboration_task_outbox"] {
        let name = format!("{}.{table}", f.task);
        rows.retain(|(table, _)| table != &name);
        let digest: String = within(query(&format!("SELECT md5(coalesce(string_agg(to_jsonb(t)::text,'|' ORDER BY to_jsonb(t)::text),'')) AS digest FROM {name} t WHERE task_id<>$1"))
            .bind(task_id(10).as_uuid()).fetch_one(f.owner())).await.unwrap().get("digest");
        rows.push((name, digest));
    }
    rows.retain(|(name, _)| {
        ![
            "collaboration_artifacts",
            "collaboration_artifact_revisions",
            "collaboration_artifact_outbox",
        ]
        .iter()
        .any(|table| name == &format!("{}.{table}", f.artifacts))
    });
    rows.sort();
    rows
}

pub(super) async fn artifact_rows(f: &Fixture) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    for table in [
        "collaboration_artifacts",
        "collaboration_artifact_revisions",
        "collaboration_artifact_outbox",
    ] {
        let values = within(
            query(&format!(
                "SELECT to_jsonb(t)::text AS value FROM {}.{table} t ORDER BY to_jsonb(t)::text",
                f.artifacts
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

pub(super) async fn assert_saved(
    p: &Prepared<'_>,
    original: &TaskContentSnapshot,
    saved: &TaskContentSnapshot,
    original_artifacts: &[(String, String)],
    original_files: &[FileState],
) {
    let f = p.base.base.base.f;
    assert_eq!(u64::from(saved.task.revision), 2);
    assert_eq!(saved.task.title, SAVED_TITLE);
    assert_eq!(saved.manifest.title(), SAVED_TITLE);
    assert_eq!(saved.task.reference, original.task.reference);
    assert_eq!(saved.task.owner, original.task.owner);
    assert_eq!(saved.task.created_at, original.task.created_at);
    assert_eq!(saved.task.status, original.task.status);
    assert_eq!(saved.task.definition, None);
    assert_eq!(saved.task.input_refs, saved.manifest.input_refs());
    assert_ne!(saved.task.input_refs, original.task.input_refs);
    assert_eq!(saved.manifest.payload().len(), 2);
    let rows = artifact_rows(f).await;
    assert_eq!(rows.len(), original_artifacts.len() + 6);
    assert!(
        original_artifacts.iter().all(|old| rows.contains(old)),
        "old Artifact metadata/outbox must remain byte-exact"
    );
    let files = p.files();
    assert_eq!(files.len(), original_files.len() + 2);
    assert!(
        original_files.iter().all(|old| files.contains(old)),
        "old files/inodes/permissions must remain exact"
    );
    for (index, binding) in saved.manifest.payload().iter().enumerate() {
        let reference = binding.artifact();
        assert_eq!(binding.filename(), SAVED_NAMES[index]);
        assert_eq!(binding.language(), SAVED_LANGUAGES[index]);
        assert_eq!(reference.workspace, original.task.reference.workspace);
        assert!(original
            .manifest
            .payload()
            .iter()
            .all(|old| old.artifact().artifact_id != reference.artifact_id
                && old.artifact().revision_id != reference.revision_id));
        assert_eq!(
            reference.sha256.to_string(),
            format!("{:x}", Sha256::digest(SAVED_TEXTS[index].as_bytes()))
        );
        let head = within(query(&format!("SELECT owner_id,head_revision_id,sequence FROM {}.collaboration_artifacts WHERE artifact_id=$1", f.artifacts)).bind(reference.artifact_id.as_uuid()).fetch_one(f.owner())).await.unwrap();
        assert_eq!(
            head.get::<uuid::Uuid, _>("owner_id"),
            saved.task.owner.principal_id.as_uuid()
        );
        assert_eq!(
            head.get::<uuid::Uuid, _>("head_revision_id"),
            reference.revision_id.as_uuid()
        );
        assert_eq!(head.get::<i64, _>("sequence"), 1);
        let revision = within(query(&format!("SELECT artifact_id,sequence,snapshot FROM {}.collaboration_artifact_revisions WHERE revision_id=$1", f.artifacts)).bind(reference.revision_id.as_uuid()).fetch_one(f.owner())).await.unwrap();
        assert_eq!(
            revision.get::<uuid::Uuid, _>("artifact_id"),
            reference.artifact_id.as_uuid()
        );
        assert_eq!(revision.get::<i64, _>("sequence"), 1);
        let raw = revision.get::<String, _>("snapshot");
        let artifact: ArtifactRevision = serde_json::from_str(&raw).unwrap();
        assert_eq!(artifact.reference, *reference);
        assert_eq!(artifact.owner, saved.task.owner);
        assert_eq!(
            artifact.title, original.task.title,
            "publication title must come from original full baseline"
        );
        assert_eq!(artifact.kind.as_str(), "cyanrex.task-text");
        assert_eq!(artifact.media_type, "text/plain");
        assert_eq!(artifact.byte_length, SAVED_TEXTS[index].len() as u64);
        assert_eq!(u64::from(artifact.sequence), 1);
        assert_eq!(artifact.parent, None);
        let events = within(query(&format!("SELECT event_id,revision_id,sequence,actor_id,event_type,snapshot FROM {}.collaboration_artifact_outbox WHERE artifact_id=$1", f.artifacts)).bind(reference.artifact_id.as_uuid()).fetch_all(f.owner())).await.unwrap();
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert!(!event.get::<uuid::Uuid, _>("event_id").is_nil());
        assert_eq!(
            event.get::<uuid::Uuid, _>("revision_id"),
            reference.revision_id.as_uuid()
        );
        assert_eq!(event.get::<i64, _>("sequence"), 1);
        assert_eq!(
            event.get::<uuid::Uuid, _>("actor_id"),
            saved.task.owner.principal_id.as_uuid()
        );
        assert_eq!(
            event.get::<String, _>("event_type"),
            "cyanrex.artifact.created"
        );
        assert_eq!(event.get::<String, _>("snapshot"), raw);
        let path = p.blob(reference);
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(metadata.file_type().is_file());
        assert_eq!(metadata.mode() & 0o777, 0o400);
        assert_eq!(metadata.nlink(), 1);
        assert_eq!(metadata.uid(), p.identity.uid[0]);
        assert_eq!(fs::read(path).unwrap(), SAVED_TEXTS[index].as_bytes());
    }
    assert_ne!(
        saved.manifest.payload()[0].artifact().artifact_id,
        saved.manifest.payload()[1].artifact().artifact_id
    );
    assert_ne!(
        saved.manifest.payload()[0].artifact().revision_id,
        saved.manifest.payload()[1].artifact().revision_id
    );
}

pub(super) async fn exact_profile(f: &Fixture) {
    for (schema, table, insert, updates) in [
        (
            &f.task,
            "collaboration_task_schema",
            false,
            &["singleton"][..],
        ),
        (
            &f.task,
            "collaboration_tasks",
            false,
            &["task_id", "revision", "snapshot", "manifest"][..],
        ),
        (
            &f.task,
            "collaboration_task_outbox",
            true,
            &["event_id"][..],
        ),
        (
            &f.artifacts,
            "collaboration_artifact_schema",
            false,
            &["singleton"][..],
        ),
        (
            &f.artifacts,
            "collaboration_artifacts",
            true,
            &["artifact_id"][..],
        ),
        (
            &f.artifacts,
            "collaboration_artifact_revisions",
            true,
            &[][..],
        ),
        (&f.artifacts, "collaboration_artifact_outbox", true, &[][..]),
    ] {
        let name = format!("{schema}.{table}");
        let row = within(query("SELECT has_table_privilege($1,$2,'INSERT') AS i,has_table_privilege($1,$2,'UPDATE') AS u,has_table_privilege($1,$2,'DELETE') AS d,has_table_privilege($1,$2,'TRIGGER') AS t").bind(&f.role).bind(&name).fetch_one(f.owner())).await.unwrap();
        assert_eq!(
            row.get::<bool, _>("i"),
            insert,
            "exact INSERT profile for {table}"
        );
        assert!(!row.get::<bool, _>("u") && !row.get::<bool, _>("d") && !row.get::<bool, _>("t"));
        let columns = within(query("SELECT a.attname AS name,has_column_privilege($1,c.oid,a.attname,'UPDATE') AS allowed FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace JOIN pg_catalog.pg_attribute a ON a.attrelid=c.oid AND a.attnum>0 AND NOT a.attisdropped WHERE n.nspname=$2 AND c.relname=$3 ORDER BY a.attnum").bind(&f.role).bind(schema).bind(table).fetch_all(f.owner())).await.unwrap();
        for column in columns {
            assert_eq!(
                column.get::<bool, _>("allowed"),
                updates.contains(&column.get::<String, _>("name").as_str()),
                "exact UPDATE column profile for {table}"
            );
        }
    }
}
