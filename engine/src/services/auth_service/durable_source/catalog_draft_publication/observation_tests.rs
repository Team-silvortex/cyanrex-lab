use super::super::{
    DurableAuthError, SessionArtifactError, SessionCommandError, SessionDraftObservedOutcome,
    SessionDraftPublicationObservation, SessionTaskContent, SessionTaskError,
};
use super::*;
use crate::{
    models::collaboration::*,
    services::{
        artifact_store::ArtifactStoreError, task_catalog::TaskDefinitionCatalog,
        task_store::TaskStoreError,
    },
};
use chrono::{TimeZone, Utc};
use sqlx_postgres::PgPoolOptions;
use std::{fs, future::ready, path::PathBuf, time::Duration};
use uuid::Uuid;

#[path = "../../../../../tests/session_catalog_draft_publication/fixture.rs"]
mod fixture;
use fixture::*;

fn new_attempt(f: &Offline, texts: &[&str], bound: bool) -> SessionCatalogTaskDraftPublication {
    let mut frozen = definition();
    if bound {
        frozen.evidence_schema = Some(VersionedName {
            name: "sample.document.text".parse().unwrap(),
            version: 1.try_into().unwrap(),
        });
        frozen.assessment_policy = Some(VersionedName {
            name: "sample.document.editorial".parse().unwrap(),
            version: 2.try_into().unwrap(),
        });
    }
    let catalog =
        TaskDefinitionCatalog::new(frozen.reference.package.clone(), vec![frozen.clone()]).unwrap();
    let scope = f.workspace.publication_scope();
    let artifacts = f
        .source
        .open_artifact_workspace("private_artifacts", scope, &f.root)
        .unwrap();
    let workspace = SessionCatalogTaskContentWorkspace::from_definitions(
        "private_tasks",
        scope,
        artifacts,
        &catalog,
    )
    .unwrap();
    f.source
        .prepare_catalog_task_draft_publication(TOKEN, workspace, &frozen.reference, plan(texts))
        .unwrap()
}

fn owner(attempt: &SessionCatalogTaskDraftPublication) -> PrincipalRef {
    PrincipalRef {
        authority_id: attempt.task_ref().workspace.authority_id,
        principal_id: Uuid::from_u128(33).try_into().unwrap(),
    }
}

fn artifact(attempt: &SessionCatalogTaskDraftPublication, index: usize) -> ArtifactContent {
    ArtifactContent {
        revision: ArtifactRevision {
            schema_version: CoreSchemaVersion,
            reference: attempt.allocated_artifacts()[index].clone(),
            owner: owner(attempt),
            sequence: 1.try_into().unwrap(),
            parent: None,
            kind: "cyanrex.task-text".parse().unwrap(),
            title: attempt.plan.title().into(),
            media_type: "text/plain".into(),
            byte_length: attempt.plan.items()[index].text().len() as u64,
            created_at: Utc.timestamp_opt(0, 0).unwrap(),
        },
        bytes: attempt.plan.items()[index].text().as_bytes().to_vec(),
    }
}

async fn confirm_prefix(attempt: &mut SessionCatalogTaskDraftPublication, count: usize) {
    for index in 0..count {
        let revision = artifact(attempt, index).revision;
        attempt
            .await_artifact(index, ready(Ok(revision)))
            .await
            .unwrap();
    }
}

fn task(attempt: &SessionCatalogTaskDraftPublication) -> SessionTaskContent {
    let manifest = attempt.publication_manifest().unwrap();
    SessionTaskContent {
        snapshot: TaskContentSnapshot {
            task: TaskSnapshot {
                schema_version: CoreSchemaVersion,
                reference: attempt.task_ref(),
                owner: owner(attempt),
                title: manifest.title().into(),
                definition: Some(attempt.definition().clone()),
                input_refs: manifest.input_refs(),
                status: TaskStatus::Draft,
                revision: 1.try_into().unwrap(),
                created_at: Utc.timestamp_opt(0, 0).unwrap(),
                updated_at: Utc.timestamp_opt(0, 0).unwrap(),
            },
            manifest,
        },
        contents: (0..attempt.allocated_artifacts().len())
            .map(|index| artifact(attempt, index))
            .collect(),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Preserved {
    definition: TaskDefinition,
    state: SessionDraftPublicationState,
    task: TaskRef,
    allocated: Vec<ArtifactRef>,
    confirmed: Vec<ArtifactRevision>,
    completed: Option<TaskContentSnapshot>,
}
fn preserved(attempt: &SessionCatalogTaskDraftPublication) -> Preserved {
    Preserved {
        definition: attempt.definition().clone(),
        state: attempt.state().clone(),
        task: attempt.task_ref(),
        allocated: attempt.allocated_artifacts().to_vec(),
        confirmed: attempt.confirmed_artifacts().to_vec(),
        completed: attempt.confirmed_task().cloned(),
    }
}

#[tokio::test]
async fn catalogue_observation_requires_unknown_state_before_token_or_source_access() {
    let f = Offline::new().await;
    let mut attempt = new_attempt(&f, &[], false);
    let before = preserved(&attempt);
    drop(attempt.observe_unconfirmed(TOKEN));
    for token in [TOKEN, "invalid"] {
        assert_eq!(
            attempt.observe_unconfirmed(token).await,
            Err(SessionDraftPublicationError::NoUnconfirmedStep)
        );
    }
    assert_eq!(preserved(&attempt), before);
    let returned = task(&attempt).snapshot;
    attempt
        .await_task(returned.manifest.clone(), ready(Ok(returned)))
        .await
        .unwrap();
    let before = preserved(&attempt);
    for token in [TOKEN, "invalid"] {
        assert_eq!(
            attempt.observe_unconfirmed(token).await,
            Err(SessionDraftPublicationError::NoUnconfirmedStep)
        );
    }
    assert_eq!(preserved(&attempt), before);
}

#[tokio::test]
async fn catalogue_observation_keeps_original_session_and_typed_reader_errors_without_adoption() {
    let f = Offline::new().await;
    for final_task in [false, true] {
        let mut attempt = new_attempt(&f, &["prefix", "target"], true);
        if final_task {
            confirm_prefix(&mut attempt, 2).await;
            let expected = attempt.publication_manifest().unwrap();
            assert_eq!(
                attempt
                    .await_task(
                        expected,
                        ready(Err(SessionTaskError::Task(TaskStoreError::Conflict)))
                    )
                    .await,
                Err(SessionDraftPublicationError::Task(SessionTaskError::Task(
                    TaskStoreError::Conflict
                )))
            );
        } else {
            confirm_prefix(&mut attempt, 1).await;
            assert_eq!(
                attempt
                    .await_artifact(
                        1,
                        ready(Err(SessionArtifactError::Artifact(
                            ArtifactStoreError::Conflict
                        )))
                    )
                    .await,
                Err(SessionDraftPublicationError::Artifact(
                    SessionArtifactError::Artifact(ArtifactStoreError::Conflict)
                ))
            );
        }
        let before = preserved(&attempt);
        drop(attempt.observe_unconfirmed(TOKEN));
        for token in ["invalid", "4d20c6c1-e5f8-48a1-9655-e8fb82ad399c"] {
            assert_eq!(
                attempt.observe_unconfirmed(token).await,
                Err(SessionDraftPublicationError::SessionChanged)
            );
        }
        let session = SessionCommandError::Authentication(DurableAuthError::StorageUnavailable);
        let expected = if final_task {
            SessionDraftPublicationError::Task(SessionTaskError::Session(session))
        } else {
            SessionDraftPublicationError::Artifact(SessionArtifactError::Session(session))
        };
        assert_eq!(attempt.observe_unconfirmed(TOKEN).await, Err(expected));
        assert_eq!(preserved(&attempt), before);
        assert_eq!(
            attempt.advance(TOKEN).await,
            Err(SessionDraftPublicationError::Stopped)
        );
    }
}

#[tokio::test]
async fn artifact_supplied_comparison_requires_bytes_exact_target_and_confirmed_prefix_owner() {
    let f = Offline::new().await;
    let mut attempt = new_attempt(&f, &["one", "two"], false);
    confirm_prefix(&mut attempt, 1).await;
    let returned = artifact(&attempt, 1);
    let before = preserved(&attempt);
    assert_eq!(
        attempt.artifact_observation(1, None),
        SessionDraftObservedOutcome::NotVisible
    );
    assert_eq!(
        attempt.artifact_observation(1, Some(&returned)),
        SessionDraftObservedOutcome::MatchesPlannedCreate
    );
    // These deliberately forged supplied values test comparison only. A real reader can reject
    // inconsistent stored bytes/metadata as a typed error before producing any value.
    for field in 0..7 {
        let mut changed = returned.clone();
        match field {
            0 => changed.bytes = b"tow".to_vec(),
            1 => changed.revision.reference = attempt.allocated_artifacts()[0].clone(),
            2 => changed.revision.owner.principal_id = Uuid::from_u128(44).try_into().unwrap(),
            3 => changed.revision.title = "Changed".into(),
            4 => changed.revision.byte_length += 1,
            5 => changed.revision.sequence = 2.try_into().unwrap(),
            6 => changed.revision.parent = Some(attempt.allocated_artifacts()[0].clone()),
            _ => unreachable!(),
        }
        assert_ne!(changed, returned);
        assert_eq!(
            attempt.artifact_observation(1, Some(&changed)),
            SessionDraftObservedOutcome::DiffersFromPlannedCreate,
            "field {field}"
        );
    }
    assert_eq!(
        attempt.artifact_observation(usize::MAX, Some(&returned)),
        SessionDraftObservedOutcome::DiffersFromPlannedCreate
    );
    assert_eq!(preserved(&attempt), before);
}

#[tokio::test]
async fn task_supplied_comparison_requires_full_some_frozen_definition_for_bound_and_unbound_plans()
{
    let f = Offline::new().await;
    for bound in [false, true] {
        for texts in [vec![], vec!["text"]] {
            let mut attempt = new_attempt(&f, &texts, bound);
            confirm_prefix(&mut attempt, texts.len()).await;
            let returned = task(&attempt);
            let expected = &returned.snapshot.manifest;
            let before = preserved(&attempt);
            assert_eq!(
                attempt.task_observation(expected, None),
                SessionDraftObservedOutcome::NotVisible
            );
            assert_eq!(
                attempt.task_observation(expected, Some(&returned)),
                SessionDraftObservedOutcome::MatchesPlannedCreate
            );
            // Supplied-value comparison is not a catalogue-read admission path. Real same-ref
            // metadata/binding drift or manual None returns typed InvalidRecord, not Differs.
            for field in 0..9 {
                let mut changed = returned.clone();
                match field {
                    0 => changed.snapshot.task.definition = None,
                    1 => {
                        changed.snapshot.task.definition.as_mut().unwrap().summary =
                            "Changed".into()
                    }
                    2 => {
                        changed.snapshot.task.definition.as_mut().unwrap().title = "Changed".into()
                    }
                    3 => {
                        changed
                            .snapshot
                            .task
                            .definition
                            .as_mut()
                            .unwrap()
                            .reference
                            .package
                            .version = 9.try_into().unwrap()
                    }
                    4 => {
                        changed
                            .snapshot
                            .task
                            .definition
                            .as_mut()
                            .unwrap()
                            .reference
                            .package
                            .name = "sample.other".parse().unwrap()
                    }
                    5 => {
                        changed
                            .snapshot
                            .task
                            .definition
                            .as_mut()
                            .unwrap()
                            .reference
                            .name = "sample.document.other".parse().unwrap()
                    }
                    6 => {
                        changed
                            .snapshot
                            .task
                            .definition
                            .as_mut()
                            .unwrap()
                            .reference
                            .version = 9.try_into().unwrap()
                    }
                    7 => {
                        let definition = changed.snapshot.task.definition.as_mut().unwrap();
                        if bound {
                            definition.evidence_schema = None;
                            definition.assessment_policy = None;
                        } else {
                            definition.evidence_schema = Some(VersionedName {
                                name: "sample.document.text".parse().unwrap(),
                                version: 1.try_into().unwrap(),
                            });
                            definition.assessment_policy = definition.evidence_schema.clone();
                        }
                    }
                    8 => {
                        let definition = changed.snapshot.task.definition.as_mut().unwrap();
                        definition.evidence_schema = None;
                        definition.assessment_policy = Some(VersionedName {
                            name: "sample.document.changed".parse().unwrap(),
                            version: 1.try_into().unwrap(),
                        });
                    }
                    _ => unreachable!(),
                }
                assert_ne!(changed, returned);
                assert_eq!(
                    attempt.task_observation(expected, Some(&changed)),
                    SessionDraftObservedOutcome::DiffersFromPlannedCreate,
                    "bound {bound}, field {field}"
                );
            }
            assert_eq!(preserved(&attempt), before);
        }
    }
}

#[tokio::test]
async fn task_supplied_comparison_requires_complete_ordered_current_contents_and_create_metadata() {
    let f = Offline::new().await;
    let mut attempt = new_attempt(&f, &["same", "same"], false);
    confirm_prefix(&mut attempt, 2).await;
    let returned = task(&attempt);
    let expected = &returned.snapshot.manifest;
    let before = preserved(&attempt);
    assert_eq!(
        attempt.task_observation(expected, Some(&returned)),
        SessionDraftObservedOutcome::MatchesPlannedCreate
    );
    for field in 0..12 {
        let mut changed = returned.clone();
        match field {
            0 => changed.contents.swap(0, 1),
            1 => {
                changed.contents.pop();
            }
            2 => changed.contents.push(changed.contents[0].clone()),
            3 => changed.contents[0].bytes = b"smae".to_vec(),
            4 => {
                changed.contents[1].revision.owner.principal_id =
                    Uuid::from_u128(44).try_into().unwrap()
            }
            5 => changed.snapshot.task.status = TaskStatus::Ready,
            6 => changed.snapshot.task.revision = 2.try_into().unwrap(),
            7 => {
                let mut payload = expected.payload().to_vec();
                payload[0] = TextPayloadBinding::new(
                    payload[0].artifact().clone(),
                    "changed.txt",
                    "plaintext",
                )
                .unwrap();
                changed.snapshot.manifest =
                    TaskContentManifest::new(expected.title(), payload).unwrap();
            }
            8 => changed.snapshot.task.reference.task_id = Uuid::from_u128(44).try_into().unwrap(),
            9 => changed.snapshot.task.title = "Changed".into(),
            10 => changed.snapshot.task.input_refs.swap(0, 1),
            11 => changed.contents[0].revision.title = "Changed".into(),
            _ => unreachable!(),
        }
        assert_ne!(changed, returned);
        assert_eq!(
            attempt.task_observation(expected, Some(&changed)),
            SessionDraftObservedOutcome::DiffersFromPlannedCreate,
            "field {field}"
        );
    }
    assert_eq!(preserved(&attempt), before);
}

#[tokio::test]
async fn body_free_comparisons_do_not_upgrade_unknown_targets_to_confirmation_or_retry() {
    let f = Offline::new().await;
    let private_text = "PRIVATE planned body not present in the observation report";
    let mut attempt = new_attempt(&f, &[private_text], false);
    let failure = SessionArtifactError::Artifact(ArtifactStoreError::Conflict);
    assert_eq!(
        attempt.await_artifact(0, ready(Err(failure))).await,
        Err(SessionDraftPublicationError::Artifact(failure))
    );
    let before = preserved(&attempt);
    let SessionDraftPublicationState::Unconfirmed(step) = attempt.state() else {
        panic!("exact unknown target required")
    };
    for result in [
        SessionDraftObservedOutcome::MatchesPlannedCreate,
        SessionDraftObservedOutcome::DiffersFromPlannedCreate,
        SessionDraftObservedOutcome::NotVisible,
    ] {
        let report = SessionDraftPublicationObservation {
            step: step.clone(),
            result,
        };
        assert!(!format!("{report:?}").contains(private_text));
        assert!(!format!("{report:?}").contains(TOKEN));
        assert_eq!(report.clone(), report);
        assert_eq!(preserved(&attempt), before);
    }
    assert_eq!(
        attempt.advance(TOKEN).await,
        Err(SessionDraftPublicationError::Stopped)
    );
    assert_eq!(preserved(&attempt), before);
}
