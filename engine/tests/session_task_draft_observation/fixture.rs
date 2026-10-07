use super::*;
use sha2::{Digest, Sha256};
use std::os::unix::fs::{MetadataExt, PermissionsExt};

pub type Observed = Result<SessionDraftPublicationObservation, SessionDraftPublicationError>;
pub const ITEMS: &[(&str, &str, &str)] = &[("文档.txt", "future.lang", "hello\r\n😀")];

#[derive(Debug, PartialEq, Eq)]
pub struct Snapshot {
    tables: Vec<(String, String)>,
    files: Vec<(String, u64, u32, String)>,
    root: (u64, u64, u32),
}

pub async fn snapshot(f: &Fixture) -> Snapshot {
    let mut tables = Vec::new();
    for schema in [&f.auth.base.schema, &f.artifact_schema, &f.task_schema] {
        let names =
            query("SELECT tablename FROM pg_tables WHERE schemaname = $1 ORDER BY tablename")
                .bind(schema)
                .fetch_all(&f.auth.base.admin)
                .await
                .unwrap();
        for row in names {
            let table: String = row.get("tablename");
            let digest: String = query(&format!(
                "SELECT md5(COALESCE(string_agg(row_to_json(t)::text, '' ORDER BY row_to_json(t)::text), '')) AS digest
                 FROM \"{schema}\".\"{table}\" t"
            )).fetch_one(&f.auth.base.admin).await.unwrap().get("digest");
            tables.push((format!("{schema}.{table}"), digest));
        }
    }
    let mut files: Vec<_> = fs::read_dir(&f.directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let metadata = entry.metadata().unwrap();
            (
                entry.file_name().to_str().unwrap().to_owned(),
                metadata.len(),
                metadata.mode(),
                format!("{:x}", Sha256::digest(fs::read(entry.path()).unwrap())),
            )
        })
        .collect();
    files.sort();
    let root = fs::metadata(&f.directory).unwrap();
    Snapshot {
        tables,
        files,
        root: (root.dev(), root.ino(), root.mode()),
    }
}

pub async fn unknown_artifact(
    f: &Fixture,
    items: &[(&str, &str, &str)],
    prefix: usize,
) -> SessionTaskDraftPublication {
    let mut attempt = prepare(f, items);
    for _ in 0..prefix {
        attempt.advance(&f.auth.learner_token).await.unwrap();
    }
    f.sql_artifact("UPDATE collaboration_artifact_schema SET version = 999")
        .await;
    let failed = attempt.advance(&f.auth.learner_token).await;
    f.sql_artifact("UPDATE collaboration_artifact_schema SET version = 1")
        .await;
    assert_eq!(
        failed,
        Err(SessionDraftPublicationError::Artifact(
            SessionArtifactError::Artifact(ArtifactStoreError::UnsupportedSchema,)
        ))
    );
    attempt
}

pub async fn unknown_task(
    f: &Fixture,
    items: &[(&str, &str, &str)],
) -> SessionTaskDraftPublication {
    let mut attempt = prepare(f, items);
    for _ in items {
        attempt.advance(&f.auth.learner_token).await.unwrap();
    }
    f.sql_task("UPDATE collaboration_task_schema SET version = 999")
        .await;
    let failed = attempt.advance(&f.auth.learner_token).await;
    f.sql_task("UPDATE collaboration_task_schema SET version = 3")
        .await;
    assert_eq!(
        failed,
        Err(SessionDraftPublicationError::Task(SessionTaskError::Task(
            TaskStoreError::UnsupportedSchema,
        )))
    );
    attempt
}

pub async fn publish_target(
    f: &Fixture,
    attempt: &SessionTaskDraftPublication,
    items: &[(&str, &str, &str)],
    index: usize,
) -> ArtifactRevision {
    let reference = &attempt.allocated_artifacts()[index];
    f.auth
        .source
        .create_session_artifact(
            &f.auth.learner_token,
            &f.artifact_workspace,
            reference.artifact_id,
            reference.revision_id,
            plan(items).publication_draft(index).unwrap(),
        )
        .await
        .unwrap()
}

pub async fn create_target(
    f: &Fixture,
    attempt: &SessionTaskDraftPublication,
    items: &[(&str, &str, &str)],
) -> TaskContentSnapshot {
    let owner = f.auth.learner.principal.reference;
    let contents = attempt
        .confirmed_artifacts()
        .iter()
        .zip(items)
        .map(|(revision, item)| ArtifactContent {
            revision: revision.clone(),
            bytes: item.2.as_bytes().to_vec(),
        })
        .collect::<Vec<_>>();
    let manifest = plan(items)
        .bind_published(scope(), owner, &contents)
        .unwrap();
    f.auth
        .source
        .create_session_content_task(
            &f.auth.learner_token,
            &f.workspace,
            attempt.task_ref().task_id,
            manifest,
        )
        .await
        .unwrap()
}

pub fn report(
    attempt: &SessionTaskDraftPublication,
    result: SessionDraftObservedOutcome,
) -> Observed {
    let SessionDraftPublicationState::Unconfirmed(step) = attempt.state() else {
        panic!("fixture expected an unconfirmed step");
    };
    Ok(SessionDraftPublicationObservation {
        step: step.clone(),
        result,
    })
}

pub fn damage(f: &Fixture, reference: &ArtifactRef, missing: bool) {
    let path = f.blob(reference);
    if missing {
        fs::remove_file(path).unwrap();
    } else {
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&path, b"external synthetic damage").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
    }
}

pub async fn lock_artifact_schema(f: &Fixture) -> Tx<'_> {
    let mut tx = f.artifact_pool.begin().await.unwrap();
    query("SELECT singleton FROM collaboration_artifact_schema FOR UPDATE")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    tx
}
