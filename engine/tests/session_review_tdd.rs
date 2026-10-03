#![cfg(unix)]
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        artifact_store::{ArtifactDraft, ArtifactStore, ArtifactStoreError},
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthError, DurableAuthSource, SessionArtifactWorkspace,
            SessionBindCommand, SessionCommandError, SessionPolicyCommand, SessionReviewError,
            SessionReviewWorkspace,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, LegacyAccessPolicy, LegacyIdentityAction,
            LegacyIdentityCommand, StoredLegacyAccessPolicy, StoredLegacyIdentity,
        },
        review_store::{HumanReviewEdit, ReviewDraft, ReviewStore, ReviewStoreError},
    },
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use std::{fs, path::PathBuf};
use uuid::Uuid;

#[allow(dead_code)]
#[path = "durable_auth_source/fixture.rs"]
mod source_fixture;
use source_fixture::{authority, id, otp, PASSWORD};
#[allow(dead_code)]
#[path = "durable_collaboration/fixture.rs"]
mod identity_fixture;
use identity_fixture::scope;
#[path = "session_review/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "session_review/concurrency.rs"]
mod concurrency;
#[path = "session_review/faults.rs"]
mod faults;
#[path = "session_review/lifecycle.rs"]
mod lifecycle;

#[tokio::test]
async fn session_review_configuration_rejects_namespace_aliases_and_scope_mismatch_without_io() {
    let source = unavailable_source().await;
    let directory = private_directory();
    let artifacts = source
        .open_artifact_workspace("private_artifacts", scope(), &directory)
        .unwrap();
    for namespace in [
        "",
        "public",
        "pg_catalog",
        "a,b",
        "a.b",
        "Bad",
        "private_artifacts",
    ] {
        assert!(matches!(
            SessionReviewWorkspace::new(namespace, scope(), artifacts.clone(), policy()),
            Err(SessionReviewError::InvalidNamespace)
        ));
    }
    for other in [
        WorkspaceRef {
            workspace_id: id(),
            ..scope()
        },
        WorkspaceRef {
            authority_id: id(),
            ..scope()
        },
    ] {
        assert!(matches!(
            SessionReviewWorkspace::new("private_reviews", other, artifacts.clone(), policy()),
            Err(SessionReviewError::Review(ReviewStoreError::ScopeMismatch))
        ));
    }
    assert!(
        SessionReviewWorkspace::new("private_reviews", scope(), artifacts.clone(), policy())
            .is_ok()
    );
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
    drop(artifacts);
    fs::remove_dir(directory).unwrap();
}

#[tokio::test]
async fn unavailable_session_reviews_never_publish_an_unauthenticated_or_volatile_judgment() {
    let source = unavailable_source().await;
    let directory = private_directory();
    let workspace = offline_workspace(&source, &directory);
    let target = synthetic_ref();
    assert_eq!(
        source
            .create_session_review(
                "invalid",
                &workspace,
                id(),
                vec![target.clone()],
                vec![],
                edit(HumanReviewVerdict::Approved, "")
            )
            .await,
        Err(SessionReviewError::Session(
            SessionCommandError::InvalidSession
        ))
    );
    let token = Uuid::new_v4().to_string();
    let unavailable = SessionReviewError::Session(SessionCommandError::Authentication(
        DurableAuthError::StorageUnavailable,
    ));
    assert_eq!(
        source
            .create_session_review(
                &token,
                &workspace,
                id(),
                vec![target],
                vec![],
                edit(HumanReviewVerdict::Approved, "")
            )
            .await,
        Err(unavailable)
    );
    let reference = ReviewRef {
        workspace: scope(),
        review_id: id(),
        revision: 1.try_into().unwrap(),
    };
    assert_eq!(
        source
            .read_session_review(&token, &workspace, reference)
            .await,
        Err(unavailable)
    );
    assert_eq!(
        source
            .revise_session_review(
                &token,
                &workspace,
                reference,
                edit(HumanReviewVerdict::Commented, "No fallback")
            )
            .await,
        Err(unavailable)
    );
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
    drop(workspace);
    fs::remove_dir(directory).unwrap();
}

#[tokio::test]
async fn session_review_input_shape_is_bounded_before_any_database_work() {
    let source = unavailable_source().await;
    let directory = private_directory();
    let workspace = offline_workspace(&source, &directory);
    let target = synthetic_ref();
    let conflict = ArtifactRef {
        sha256: "b".repeat(64).parse().unwrap(),
        ..target.clone()
    };
    let too_many = (0..33).map(|_| synthetic_ref()).collect::<Vec<_>>();
    let token = Uuid::new_v4().to_string();
    for (targets, evidence) in [
        (vec![], vec![]),
        (too_many.clone(), vec![]),
        (vec![target.clone()], too_many),
        (vec![target.clone(), target.clone()], vec![]),
        (vec![target.clone()], vec![target.clone(), target.clone()]),
        (vec![target.clone(), conflict.clone()], vec![]),
        (vec![target.clone()], vec![conflict]),
    ] {
        assert_eq!(
            source
                .create_session_review(
                    &token,
                    &workspace,
                    id(),
                    targets,
                    evidence,
                    edit(HumanReviewVerdict::Approved, "")
                )
                .await,
            Err(SessionReviewError::Review(ReviewStoreError::InvalidInput))
        );
    }
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
    drop(workspace);
    fs::remove_dir(directory).unwrap();
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_reviews_pin_history_targets_evidence_and_configured_human_policy() {
    let f = Fixture::ready().await;
    let token = &f.auth.learner_token;
    let policy_state = f
        .auth
        .current(f.auth.learner.binding.principal_id)
        .await
        .policy;
    assert!(policy_state.membership.role_refs.is_empty());
    assert!(!policy_state.deployment_granted);
    let first = f.create_artifact(token, b"Original target").await;
    let evidence = f.create_artifact(token, b"Supporting document").await;
    let second = f
        .auth
        .source
        .revise_session_artifact(
            token,
            &f.artifact_workspace,
            &first.reference,
            id(),
            artifact_draft(b"Later target"),
        )
        .await
        .unwrap();
    let targets = vec![second.reference.clone(), first.reference.clone()];
    let refs = vec![evidence.reference.clone(), first.reference.clone()];
    let created = f.create(token, targets.clone(), refs.clone()).await;
    assert_eq!(created.reviewer, f.auth.learner.principal.reference);
    assert_eq!(created.targets, targets);
    assert_eq!(created.evidence, refs);
    assert_eq!(created.supersedes, None);
    assert_eq!(
        created.judgment,
        ReviewJudgment::Human {
            policy: policy(),
            verdict: HumanReviewVerdict::Approved,
            comment: "Checked document".into()
        }
    );
    f.auth
        .source
        .revise_session_artifact(
            token,
            &f.artifact_workspace,
            &second.reference,
            id(),
            artifact_draft(b"Newest target, not reviewed"),
        )
        .await
        .unwrap();
    let updated = f
        .revise(
            token,
            created.reference,
            edit(
                HumanReviewVerdict::ChangesRequested,
                "Please clarify paragraph two",
            ),
        )
        .await
        .unwrap();
    assert_eq!(updated.supersedes, Some(created.reference));
    assert_eq!(u64::from(updated.reference.revision), 2);
    assert_eq!(updated.targets, targets);
    assert_eq!(updated.evidence, refs);
    assert_eq!(updated.reviewer, created.reviewer);
    assert_eq!(
        updated.judgment,
        ReviewJudgment::Human {
            policy: policy(),
            verdict: HumanReviewVerdict::ChangesRequested,
            comment: "Please clarify paragraph two".into()
        }
    );
    assert_eq!(
        f.read(token, created.reference).await.unwrap(),
        Some(created)
    );
    assert_eq!(
        f.read(token, updated.reference).await.unwrap(),
        Some(updated.clone())
    );
    let future = ReviewRef {
        revision: 3.try_into().unwrap(),
        ..updated.reference
    };
    assert_eq!(f.read(token, future).await.unwrap(), None);
    assert_eq!(f.count_review("collaboration_review_outbox").await, 2);
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 4);
    assert_eq!(f.files(), 4);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_reviews_manager_grants_never_expand_private_review_or_artifact_access() {
    let f = Fixture::ready().await;
    let target = f
        .create_artifact(&f.auth.learner_token, b"Private target")
        .await;
    let record = f
        .create(
            &f.auth.learner_token,
            vec![target.reference.clone()],
            vec![],
        )
        .await;
    fs::remove_file(f.blob(&target.reference)).unwrap();
    assert_eq!(
        f.read(&f.auth.manager_token, record.reference)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        f.revise(
            &f.auth.manager_token,
            record.reference,
            edit(HumanReviewVerdict::Commented, "Manager cannot amend")
        )
        .await,
        Err(SessionReviewError::Review(ReviewStoreError::NotFound))
    );
    assert_eq!(
        f.request(&f.auth.manager_token, id(), vec![target.reference], vec![])
            .await,
        Err(SessionReviewError::Artifact(ArtifactStoreError::NotFound))
    );
    assert_eq!(
        f.read(&f.auth.learner_token, record.reference).await,
        Err(SessionReviewError::Artifact(
            ArtifactStoreError::BlobUnavailable
        ))
    );
    assert_eq!(f.count_review("collaboration_reviews").await, 1);
    assert_eq!(f.count_review("collaboration_review_outbox").await, 1);
    assert_eq!(f.files(), 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_reviews_validate_every_target_and_evidence_before_publishing() {
    let f = Fixture::ready().await;
    let token = &f.auth.learner_token;
    let own = f.create_artifact(token, b"Same bytes").await;
    let other = f
        .create_artifact(&f.auth.manager_token, b"Same bytes")
        .await;
    assert_eq!(own.reference.sha256, other.reference.sha256);
    let missing = ArtifactRef {
        artifact_id: id(),
        ..own.reference.clone()
    };
    let wrong_digest = ArtifactRef {
        sha256: "f".repeat(64).parse().unwrap(),
        ..own.reference.clone()
    };
    let foreign_scope = ArtifactRef {
        workspace: WorkspaceRef {
            workspace_id: id(),
            ..scope()
        },
        ..own.reference.clone()
    };
    for invalid in [
        other.reference,
        missing.clone(),
        wrong_digest,
        foreign_scope,
    ] {
        for as_evidence in [false, true] {
            let (targets, evidence) = if as_evidence {
                (vec![own.reference.clone()], vec![invalid.clone()])
            } else {
                (vec![invalid.clone()], vec![])
            };
            assert!(f.request(token, id(), targets, evidence).await.is_err());
            assert_eq!(f.count_review("collaboration_reviews").await, 0);
            assert_eq!(f.count_review("collaboration_review_revisions").await, 0);
            assert_eq!(f.count_review("collaboration_review_outbox").await, 0);
        }
    }
    // Trusted raw storage does not establish that evidence was ever resolved or authorized.
    let unresolved = f
        .reviews
        .create(
            id(),
            own.owner,
            ReviewDraft::human(
                vec![own.reference],
                vec![missing],
                policy(),
                edit(HumanReviewVerdict::Approved, ""),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        f.read(token, unresolved.reference).await,
        Err(SessionReviewError::Artifact(ArtifactStoreError::NotFound))
    );
    assert_eq!(
        f.revise(
            token,
            unresolved.reference,
            edit(HumanReviewVerdict::Commented, "Still missing")
        )
        .await,
        Err(SessionReviewError::Artifact(ArtifactStoreError::NotFound))
    );
    assert_eq!(f.count_review("collaboration_review_outbox").await, 1);
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 2);
    assert_eq!(f.files(), 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_reviews_accept_bounded_ordered_targets_with_identical_cross_list_evidence(
) {
    let f = Fixture::ready().await;
    let token = &f.auth.learner_token;
    let mut targets = Vec::new();
    for value in 0..32u8 {
        targets.push(f.create_artifact(token, &[value]).await.reference);
    }
    let evidence = targets.iter().rev().cloned().collect::<Vec<_>>();
    let record = f.create(token, targets.clone(), evidence.clone()).await;
    assert_eq!(record.targets, targets);
    assert_eq!(record.evidence, evidence);
    assert_eq!(f.read(token, record.reference).await.unwrap(), Some(record));
    assert_eq!(f.count_review("collaboration_review_outbox").await, 1);
    assert_eq!(f.files(), 32);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_reviews_duplicate_ids_and_concurrent_amendments_have_one_winner() {
    let f = Fixture::ready().await;
    let token = &f.auth.learner_token;
    let target = f.create_artifact(token, b"Fixed review subject").await;
    let first = f.create(token, vec![target.reference], vec![]).await;
    assert_eq!(
        f.request(
            token,
            first.reference.review_id,
            first.targets.clone(),
            vec![]
        )
        .await,
        Err(SessionReviewError::Review(ReviewStoreError::Conflict))
    );
    let (a, b) = tokio::join!(
        f.revise(
            token,
            first.reference,
            edit(HumanReviewVerdict::Commented, "Comment A")
        ),
        f.revise(
            token,
            first.reference,
            edit(HumanReviewVerdict::ChangesRequested, "Change B")
        )
    );
    let next = match (a, b) {
        (Ok(winner), Err(SessionReviewError::Review(ReviewStoreError::StaleRevision)))
        | (Err(SessionReviewError::Review(ReviewStoreError::StaleRevision)), Ok(winner)) => winner,
        other => panic!("one amendment must win: {other:?}"),
    };
    assert_eq!(next.supersedes, Some(first.reference));
    assert_eq!(next.targets, first.targets);
    assert_eq!(next.evidence, first.evidence);
    assert_eq!(
        f.read(token, first.reference).await.unwrap(),
        Some(first.clone())
    );
    assert_eq!(
        f.revise(
            token,
            first.reference,
            edit(HumanReviewVerdict::Approved, "Stale")
        )
        .await,
        Err(SessionReviewError::Review(ReviewStoreError::StaleRevision))
    );
    assert_eq!(f.count_review("collaboration_reviews").await, 1);
    assert_eq!(f.count_review("collaboration_review_revisions").await, 2);
    assert_eq!(f.count_review("collaboration_review_outbox").await, 2);
    assert_eq!(f.files(), 1);
    f.cleanup().await;
}
