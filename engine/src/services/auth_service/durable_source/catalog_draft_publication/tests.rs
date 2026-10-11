use super::*;
use crate::models::collaboration::*;
use crate::services::auth_service::durable_source::{
    SessionArtifactError, SessionDraftPublicationStep, SessionTaskError,
};
use crate::services::task_catalog::TaskDefinitionCatalog;
use chrono::{TimeZone, Utc};
use sqlx_postgres::PgPoolOptions;
use std::{
    fs,
    future::{pending, poll_fn, ready, Future},
    path::PathBuf,
    task::Poll,
    time::Duration,
};
use uuid::Uuid;

#[path = "../../../../../tests/session_catalog_draft_publication/fixture.rs"]
mod fixture;
use fixture::*;

fn attempt(f: &Offline, texts: &[&str]) -> SessionCatalogTaskDraftPublication {
    f.source
        .prepare_catalog_task_draft_publication(
            TOKEN,
            f.workspace.clone(),
            &definition().reference,
            plan(texts),
        )
        .unwrap()
}
fn owner(attempt: &SessionCatalogTaskDraftPublication) -> PrincipalRef {
    PrincipalRef {
        authority_id: attempt.task_ref().workspace.authority_id,
        principal_id: Uuid::from_u128(33).try_into().unwrap(),
    }
}
fn revision(attempt: &SessionCatalogTaskDraftPublication, index: usize) -> ArtifactRevision {
    ArtifactRevision {
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
    }
}
fn snapshot(attempt: &SessionCatalogTaskDraftPublication) -> TaskContentSnapshot {
    let manifest = attempt.publication_manifest().unwrap();
    TaskContentSnapshot {
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
    }
}
async fn confirm_first(attempt: &mut SessionCatalogTaskDraftPublication) -> ArtifactRevision {
    let returned = revision(attempt, 0);
    assert_eq!(
        attempt
            .await_artifact(0, ready(Ok(returned.clone())))
            .await
            .unwrap(),
        SessionDraftPublicationProgress::ArtifactConfirmed { index: 0 }
    );
    returned
}

#[tokio::test]
async fn exact_receipts_confirm_one_step_and_preserve_some_unbound_definition_including_empty() {
    let f = Offline::new().await;
    for texts in [vec![], vec!["same", "same"]] {
        let mut attempt = attempt(&f, &texts);
        for index in 0..texts.len() {
            let returned = revision(&attempt, index);
            assert_eq!(
                attempt
                    .await_artifact(index, ready(Ok(returned.clone())))
                    .await
                    .unwrap(),
                SessionDraftPublicationProgress::ArtifactConfirmed { index }
            );
            assert_eq!(attempt.confirmed_artifacts()[index], returned);
            assert_eq!(attempt.state(), &SessionDraftPublicationState::Ready);
            assert!(attempt.confirmed_task().is_none());
        }
        let returned = snapshot(&attempt);
        assert_eq!(
            returned.manifest.input_refs(),
            attempt.allocated_artifacts()
        );
        assert_eq!(
            attempt
                .await_task(returned.manifest.clone(), ready(Ok(returned.clone())))
                .await
                .unwrap(),
            SessionDraftPublicationProgress::TaskConfirmed
        );
        assert_eq!(attempt.confirmed_task(), Some(&returned));
        assert_eq!(attempt.state(), &SessionDraftPublicationState::Complete);
        assert_eq!(
            attempt.advance(TOKEN).await,
            Err(SessionDraftPublicationError::Stopped)
        );
    }
}

#[tokio::test]
async fn artifact_receipt_requires_each_creation_coordinate_and_metadata_field() {
    let f = Offline::new().await;
    for field in 0..12 {
        let mut attempt = attempt(&f, &["same", "same"]);
        let mut returned = revision(&attempt, 0);
        match field {
            0 => returned.reference.artifact_id = Uuid::new_v4().try_into().unwrap(),
            1 => returned.reference.revision_id = Uuid::new_v4().try_into().unwrap(),
            2 => returned.reference.workspace.workspace_id = Uuid::new_v4().try_into().unwrap(),
            3 => returned.reference.sha256 = "0".repeat(64).parse().unwrap(),
            4 => returned.owner.authority_id = Uuid::new_v4().try_into().unwrap(),
            5 => returned.byte_length += 1,
            6 => returned.sequence = 2.try_into().unwrap(),
            7 => returned.parent = Some(attempt.allocated_artifacts()[1].clone()),
            8 => returned.kind = "sample.other".parse().unwrap(),
            9 => returned.title = "Unexpected".into(),
            10 => returned.media_type = "application/octet-stream".into(),
            11 => returned = revision(&attempt, 1),
            _ => unreachable!(),
        }
        assert_eq!(
            attempt.await_artifact(0, ready(Ok(returned))).await,
            Err(SessionDraftPublicationError::InvalidConfirmation),
            "field {field}"
        );
        assert!(attempt.confirmed_artifacts().is_empty());
        assert_eq!(
            attempt.state(),
            &SessionDraftPublicationState::Unconfirmed(SessionDraftPublicationStep::Artifact {
                index: 0,
                reference: attempt.allocated_artifacts()[0].clone()
            })
        );
        assert_eq!(
            attempt.advance(TOKEN).await,
            Err(SessionDraftPublicationError::Stopped)
        );
    }
}

#[tokio::test]
async fn later_artifact_cannot_change_first_receipt_owner_and_error_preserves_prefix() {
    let f = Offline::new().await;
    for changed_owner in [false, true] {
        let mut attempt = attempt(&f, &["first", "second"]);
        let first = confirm_first(&mut attempt).await;
        if changed_owner {
            let mut returned = revision(&attempt, 1);
            returned.owner.principal_id = Uuid::new_v4().try_into().unwrap();
            assert_eq!(
                attempt.await_artifact(1, ready(Ok(returned))).await,
                Err(SessionDraftPublicationError::InvalidConfirmation)
            );
        } else {
            let failure = SessionArtifactError::Artifact(
                crate::services::artifact_store::ArtifactStoreError::Conflict,
            );
            assert_eq!(
                attempt.await_artifact(1, ready(Err(failure))).await,
                Err(SessionDraftPublicationError::Artifact(failure))
            );
        };
        assert_eq!(attempt.confirmed_artifacts(), &[first]);
        assert_eq!(
            attempt.state(),
            &SessionDraftPublicationState::Unconfirmed(SessionDraftPublicationStep::Artifact {
                index: 1,
                reference: attempt.allocated_artifacts()[1].clone()
            })
        );
        assert_eq!(
            attempt.advance(TOKEN).await,
            Err(SessionDraftPublicationError::Stopped)
        );
    }
}

#[tokio::test]
async fn task_receipt_requires_complete_frozen_definition_not_only_reference_or_manual_mode() {
    let f = Offline::new().await;
    for bound in [false, true] {
        let mut frozen = definition();
        if bound {
            frozen.evidence_schema = Some(VersionedName {
                name: "sample.document.text".parse().unwrap(),
                version: 1.try_into().unwrap(),
            });
            frozen.assessment_policy = Some(VersionedName {
                name: "sample.document.policy".parse().unwrap(),
                version: 1.try_into().unwrap(),
            });
        }
        let catalog =
            TaskDefinitionCatalog::new(frozen.reference.package.clone(), vec![frozen.clone()])
                .unwrap();
        let artifacts = f
            .source
            .open_artifact_workspace(
                "private_artifacts",
                f.workspace.publication_scope(),
                &f.root,
            )
            .unwrap();
        let workspace = SessionCatalogTaskContentWorkspace::from_definitions(
            "private_tasks",
            f.workspace.publication_scope(),
            artifacts,
            &catalog,
        )
        .unwrap();
        let mut positive = f
            .source
            .prepare_catalog_task_draft_publication(
                TOKEN,
                workspace.clone(),
                &frozen.reference,
                plan(&["text"]),
            )
            .unwrap();
        confirm_first(&mut positive).await;
        let correct = snapshot(&positive);
        assert_eq!(
            positive
                .await_task(correct.manifest.clone(), ready(Ok(correct.clone())))
                .await
                .unwrap(),
            SessionDraftPublicationProgress::TaskConfirmed
        );
        assert_eq!(positive.confirmed_task(), Some(&correct));
        for field in 0..if bound { 22 } else { 18 } {
            let mut attempt = f
                .source
                .prepare_catalog_task_draft_publication(
                    TOKEN,
                    workspace.clone(),
                    &frozen.reference,
                    plan(&["text"]),
                )
                .unwrap();
            let first = confirm_first(&mut attempt).await;
            let mut returned = snapshot(&attempt);
            let expected = returned.manifest.clone();
            match field {
                0 => returned.task.definition = None,
                1 => returned.task.definition.as_mut().unwrap().title = "Changed".into(),
                2 => returned.task.definition.as_mut().unwrap().summary = "Changed".into(),
                3 => {
                    returned
                        .task
                        .definition
                        .as_mut()
                        .unwrap()
                        .reference
                        .package
                        .version = 9.try_into().unwrap()
                }
                4 => {
                    returned.task.definition.as_mut().unwrap().reference.version =
                        9.try_into().unwrap()
                }
                5 => returned.task.owner.principal_id = Uuid::new_v4().try_into().unwrap(),
                6 => returned.task.reference.task_id = Uuid::new_v4().try_into().unwrap(),
                7 => returned.task.revision = 2.try_into().unwrap(),
                8 => returned.task.status = TaskStatus::Ready,
                9 => {
                    let d = returned.task.definition.as_mut().unwrap();
                    d.evidence_schema = None;
                    d.assessment_policy = Some(VersionedName {
                        name: "sample.document.other".parse().unwrap(),
                        version: 1.try_into().unwrap(),
                    });
                }
                10 => {
                    let d = returned.task.definition.as_mut().unwrap();
                    if bound {
                        d.evidence_schema = None;
                        d.assessment_policy = None;
                    } else {
                        d.evidence_schema = Some(VersionedName {
                            name: "sample.document.text".parse().unwrap(),
                            version: 1.try_into().unwrap(),
                        });
                        d.assessment_policy = d.evidence_schema.clone();
                    }
                }
                11 => returned.task.input_refs.clear(),
                12 => {
                    returned
                        .task
                        .definition
                        .as_mut()
                        .unwrap()
                        .reference
                        .package
                        .name = "sample.other".parse().unwrap()
                }
                13 => {
                    returned.task.definition.as_mut().unwrap().reference.name =
                        "sample.document.other".parse().unwrap()
                }
                14 => {
                    returned.task.reference.workspace.workspace_id =
                        Uuid::new_v4().try_into().unwrap()
                }
                15 => returned.task.owner.authority_id = Uuid::new_v4().try_into().unwrap(),
                16 => returned.task.title = "Different task title".into(),
                17 => {
                    returned.manifest = TaskContentManifest::new(
                        expected.title(),
                        vec![TextPayloadBinding::new(
                            expected.input_refs()[0].clone(),
                            "changed-label.txt",
                            "future.language",
                        )
                        .unwrap()],
                    )
                    .unwrap()
                }
                18 => {
                    returned
                        .task
                        .definition
                        .as_mut()
                        .unwrap()
                        .evidence_schema
                        .as_mut()
                        .unwrap()
                        .name = "sample.document.other".parse().unwrap()
                }
                19 => {
                    returned
                        .task
                        .definition
                        .as_mut()
                        .unwrap()
                        .evidence_schema
                        .as_mut()
                        .unwrap()
                        .version = 9.try_into().unwrap()
                }
                20 => {
                    returned
                        .task
                        .definition
                        .as_mut()
                        .unwrap()
                        .assessment_policy
                        .as_mut()
                        .unwrap()
                        .name = "sample.document.other".parse().unwrap()
                }
                21 => {
                    returned
                        .task
                        .definition
                        .as_mut()
                        .unwrap()
                        .assessment_policy
                        .as_mut()
                        .unwrap()
                        .version = 9.try_into().unwrap()
                }
                _ => unreachable!(),
            }
            assert_eq!(
                attempt.await_task(expected, ready(Ok(returned))).await,
                Err(SessionDraftPublicationError::InvalidConfirmation),
                "bound {bound}, field {field}"
            );
            assert_eq!(attempt.confirmed_artifacts(), &[first]);
            assert!(attempt.confirmed_task().is_none());
            assert_eq!(
                attempt.state(),
                &SessionDraftPublicationState::Unconfirmed(SessionDraftPublicationStep::Task {
                    reference: attempt.task_ref()
                })
            );
        }
    }
}

#[tokio::test]
async fn task_error_keeps_artifact_receipts_and_terminal_exact_task_target() {
    let f = Offline::new().await;
    let mut attempt = attempt(&f, &["text"]);
    let first = confirm_first(&mut attempt).await;
    let failure = SessionTaskError::Task(crate::services::task_store::TaskStoreError::Conflict);
    assert_eq!(
        attempt
            .await_task(attempt.publication_manifest().unwrap(), ready(Err(failure)))
            .await,
        Err(SessionDraftPublicationError::Task(failure))
    );
    assert_eq!(attempt.confirmed_artifacts(), &[first]);
    assert!(attempt.confirmed_task().is_none());
    assert_eq!(
        attempt.state(),
        &SessionDraftPublicationState::Unconfirmed(SessionDraftPublicationStep::Task {
            reference: attempt.task_ref()
        })
    );
    assert_eq!(
        attempt.advance(TOKEN).await,
        Err(SessionDraftPublicationError::Stopped)
    );
}

#[tokio::test]
async fn dropped_polled_artifact_and_task_futures_keep_prefix_and_stop_without_retry() {
    let f = Offline::new().await;
    for task_step in [false, true] {
        let mut attempt = attempt(&f, &["first", "second"]);
        let first = confirm_first(&mut attempt).await;
        let expected;
        if task_step {
            let returned = revision(&attempt, 1);
            attempt
                .await_artifact(1, ready(Ok(returned)))
                .await
                .unwrap();
            expected = SessionDraftPublicationStep::Task {
                reference: attempt.task_ref(),
            };
            let mut future =
                Box::pin(attempt.await_task(attempt.publication_manifest().unwrap(), pending()));
            poll_fn(|cx| {
                assert!(future.as_mut().poll(cx).is_pending());
                Poll::Ready(())
            })
            .await;
            drop(future);
            assert_eq!(attempt.confirmed_artifacts().len(), 2);
        } else {
            expected = SessionDraftPublicationStep::Artifact {
                index: 1,
                reference: attempt.allocated_artifacts()[1].clone(),
            };
            let mut future = Box::pin(attempt.await_artifact(1, pending()));
            poll_fn(|cx| {
                assert!(future.as_mut().poll(cx).is_pending());
                Poll::Ready(())
            })
            .await;
            drop(future);
            assert_eq!(attempt.confirmed_artifacts(), &[first]);
        }
        assert_eq!(
            attempt.state(),
            &SessionDraftPublicationState::Unconfirmed(expected)
        );
        assert!(attempt.confirmed_task().is_none());
        assert_eq!(
            attempt.advance(TOKEN).await,
            Err(SessionDraftPublicationError::Stopped)
        );
    }
}

#[tokio::test]
async fn unpolled_receipt_helpers_do_not_mark_or_accept_synthetic_confirmations() {
    let f = Offline::new().await;
    let mut attempt = attempt(&f, &["text"]);
    let returned = revision(&attempt, 0);
    drop(attempt.await_artifact(0, ready(Ok(returned))));
    assert_eq!(attempt.state(), &SessionDraftPublicationState::Ready);
    assert!(attempt.confirmed_artifacts().is_empty());
}

#[tokio::test]
async fn shared_manifest_helper_rejects_short_and_extra_receipt_prefix_before_zip() {
    let f = Offline::new().await;
    let mut attempt = attempt(&f, &["one", "two"]);
    let first = confirm_first(&mut attempt).await;
    assert!(
        attempt.publication_manifest().is_err(),
        "short prefix cannot produce a manifest"
    );
    let second = revision(&attempt, 1);
    attempt.await_artifact(1, ready(Ok(second))).await.unwrap();
    assert_eq!(
        attempt.publication_manifest().unwrap().input_refs(),
        attempt.allocated_artifacts()
    );
    attempt.confirmed.push(first);
    assert!(
        attempt.publication_manifest().is_err(),
        "an extra receipt cannot be silently truncated by zip"
    );
}
