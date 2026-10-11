use super::*;
use std::os::unix::fs::PermissionsExt;

pub use catalog_fixture::{values, workspace};
pub use observation_fixture::snapshot;
pub const ITEMS: &[(&str, &str, &str)] = &[("文档.txt", "future.lang", "hello\r\n😀")];

pub fn prepare_catalog(
    f: &Fixture,
    entry: usize,
    items: &[(&str, &str, &str)],
) -> SessionCatalogTaskDraftPublication {
    catalog_fixture::prepare(f, &values()[entry], items)
}

#[derive(Debug, PartialEq, Eq)]
pub struct Progress {
    state: SessionDraftPublicationState,
    definition: TaskDefinition,
    task: TaskRef,
    allocated: Vec<ArtifactRef>,
    artifacts: Vec<ArtifactRevision>,
    completed: Option<TaskContentSnapshot>,
}
pub fn progress(attempt: &SessionCatalogTaskDraftPublication) -> Progress {
    Progress {
        state: attempt.state().clone(),
        definition: attempt.definition().clone(),
        task: attempt.task_ref(),
        allocated: attempt.allocated_artifacts().to_vec(),
        artifacts: attempt.confirmed_artifacts().to_vec(),
        completed: attempt.confirmed_task().cloned(),
    }
}
pub fn report(
    attempt: &SessionCatalogTaskDraftPublication,
    result: SessionDraftObservedOutcome,
) -> Result<SessionDraftPublicationObservation, SessionDraftPublicationError> {
    let SessionDraftPublicationState::Unconfirmed(step) = attempt.state() else {
        panic!("expected a real dispatched unknown step")
    };
    Ok(SessionDraftPublicationObservation {
        step: step.clone(),
        result,
    })
}
pub async fn observe_preserved(
    f: &Fixture,
    attempt: &SessionCatalogTaskDraftPublication,
) -> Result<SessionDraftPublicationObservation, SessionDraftPublicationError> {
    let before = snapshot(f).await;
    let state = progress(attempt);
    let result = attempt.observe_unconfirmed(&f.auth.learner_token).await;
    assert_eq!(
        snapshot(f).await,
        before,
        "all source/resource tables and file bytes/modes remain unchanged"
    );
    assert_eq!(
        progress(attempt),
        state,
        "current comparison must never become a receipt or restored progress"
    );
    result
}
pub async fn stopped(f: &Fixture, attempt: &mut SessionCatalogTaskDraftPublication) {
    let before = snapshot(f).await;
    let state = progress(attempt);
    assert_eq!(
        attempt.advance(&f.auth.learner_token).await,
        Err(SessionDraftPublicationError::Stopped)
    );
    assert_eq!(snapshot(f).await, before);
    assert_eq!(progress(attempt), state);
}

// Only these synthetic tests temporarily make a real selected schema unsupported. Capture and
// restore its actual version, assert the update took effect and demand the exact typed failure.
pub async fn unknown_artifact(
    f: &Fixture,
    entry: usize,
    items: &[(&str, &str, &str)],
    prefix: usize,
) -> SessionCatalogTaskDraftPublication {
    let mut attempt = prepare_catalog(f, entry, items);
    for _ in 0..prefix {
        attempt.advance(&f.auth.learner_token).await.unwrap();
    }
    let version: i64 =
        query("SELECT version::bigint AS version FROM collaboration_artifact_schema")
            .fetch_one(&f.artifact_pool)
            .await
            .unwrap()
            .get("version");
    assert_eq!(version, 1);
    assert_eq!(
        query("UPDATE collaboration_artifact_schema SET version = 999")
            .execute(&f.artifact_pool)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    assert_eq!(
        attempt.advance(&f.auth.learner_token).await,
        Err(SessionDraftPublicationError::Artifact(
            SessionArtifactError::Artifact(ArtifactStoreError::UnsupportedSchema)
        ))
    );
    assert_eq!(
        query("UPDATE collaboration_artifact_schema SET version = $1")
            .bind(version)
            .execute(&f.artifact_pool)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    assert!(
        matches!(attempt.state(), SessionDraftPublicationState::Unconfirmed(SessionDraftPublicationStep::Artifact { index, .. }) if *index == prefix)
    );
    attempt
}
pub async fn unknown_task(
    f: &Fixture,
    entry: usize,
    items: &[(&str, &str, &str)],
) -> SessionCatalogTaskDraftPublication {
    let mut attempt = prepare_catalog(f, entry, items);
    for _ in items {
        attempt.advance(&f.auth.learner_token).await.unwrap();
    }
    let version: i64 = query("SELECT version::bigint AS version FROM collaboration_task_schema")
        .fetch_one(&f.task_pool)
        .await
        .unwrap()
        .get("version");
    assert_eq!(version, 3);
    assert_eq!(
        query("UPDATE collaboration_task_schema SET version = 999")
            .execute(&f.task_pool)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    assert_eq!(
        attempt.advance(&f.auth.learner_token).await,
        Err(SessionDraftPublicationError::Task(SessionTaskError::Task(
            TaskStoreError::UnsupportedSchema
        )))
    );
    assert_eq!(
        query("UPDATE collaboration_task_schema SET version = $1")
            .bind(version)
            .execute(&f.task_pool)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    assert!(matches!(
        attempt.state(),
        SessionDraftPublicationState::Unconfirmed(SessionDraftPublicationStep::Task { .. })
    ));
    attempt
}

pub async fn publish_target(
    f: &Fixture,
    attempt: &SessionCatalogTaskDraftPublication,
    token: &str,
    items: &[(&str, &str, &str)],
    index: usize,
) -> ArtifactRevision {
    let target = &attempt.allocated_artifacts()[index];
    f.auth
        .source
        .create_session_artifact(
            token,
            &f.artifact_workspace,
            target.artifact_id,
            target.revision_id,
            plan(items).publication_draft(index).unwrap(),
        )
        .await
        .unwrap()
}
pub async fn create_target(
    f: &Fixture,
    attempt: &SessionCatalogTaskDraftPublication,
    token: &str,
    entry: usize,
    items: &[(&str, &str, &str)],
) -> TaskContentSnapshot {
    let contents: Vec<_> = attempt
        .confirmed_artifacts()
        .iter()
        .zip(items)
        .map(|(revision, item)| ArtifactContent {
            revision: revision.clone(),
            bytes: item.2.as_bytes().to_vec(),
        })
        .collect();
    let owner = if token == f.auth.learner_token {
        f.auth.learner.principal.reference
    } else {
        f.auth.manager.principal.reference
    };
    let manifest = plan(items)
        .bind_published(scope(), owner, &contents)
        .unwrap();
    f.auth
        .source
        .create_session_catalog_content_task(
            token,
            &workspace(f),
            attempt.task_ref().task_id,
            &values()[entry].reference,
            manifest,
        )
        .await
        .unwrap()
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
pub async fn lock_resource(f: &Fixture, task: bool) -> Tx<'_> {
    let pool = if task { &f.task_pool } else { &f.artifact_pool };
    let table = if task {
        "collaboration_task_schema"
    } else {
        "collaboration_artifact_schema"
    };
    let mut tx = pool.begin().await.unwrap();
    query(&format!("SELECT singleton FROM {table} FOR UPDATE"))
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    tx
}
