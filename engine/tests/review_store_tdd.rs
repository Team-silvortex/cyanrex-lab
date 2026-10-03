use cyanrex_engine::{
    models::collaboration::*,
    services::review_store::{HumanReviewEdit, ReviewDraft, ReviewStore, ReviewStoreError},
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};

#[path = "review_store/faults.rs"]
mod faults;
#[path = "review_store/fixture.rs"]
mod fixture;
#[path = "review_store/integration.rs"]
mod integration;
use fixture::*;

#[test]
fn review_drafts_require_exact_bounded_targets_and_distinct_judgments() {
    let target = artifact();
    assert!(ReviewDraft::human(
        vec![],
        vec![],
        policy(),
        edit(HumanReviewVerdict::Approved, "")
    )
    .is_err());
    assert!(ReviewDraft::human(
        vec![target.clone(); 33],
        vec![],
        policy(),
        edit(HumanReviewVerdict::Approved, "")
    )
    .is_err());
    let mut conflicting = target.clone();
    conflicting.sha256 = "b".repeat(64).parse().unwrap();
    assert!(ReviewDraft::human(
        vec![target.clone(), conflicting.clone()],
        vec![],
        policy(),
        edit(HumanReviewVerdict::Approved, "")
    )
    .is_err());
    assert!(ReviewDraft::human(
        vec![target.clone()],
        vec![conflicting],
        policy(),
        edit(HumanReviewVerdict::Approved, "")
    )
    .is_err());
    assert!(ReviewDraft::human(
        vec![target.clone()],
        vec![target.clone()],
        policy(),
        edit(HumanReviewVerdict::Commented, "Use same evidence")
    )
    .is_ok());
    assert!(HumanReviewEdit::new(HumanReviewVerdict::Commented, "").is_err());
    assert!(HumanReviewEdit::new(HumanReviewVerdict::ChangesRequested, " \n").is_err());
    assert!(HumanReviewEdit::new(HumanReviewVerdict::Approved, "").is_ok());
    assert!(HumanReviewEdit::new(HumanReviewVerdict::Approved, "x".repeat(8193)).is_err());
    assert!(HumanReviewEdit::new(HumanReviewVerdict::Approved, "bad\0comment").is_err());
    assert!(ReviewDraft::rule(vec![target.clone()], vec![], assessment()).is_ok());
    let mut bad = assessment();
    bad.policy.name = "other.policy".parse().unwrap();
    assert!(ReviewDraft::rule(vec![target.clone()], vec![], bad).is_err());
    let mut bad = assessment();
    bad.outcome.feedback = vec!["x".repeat(2048); 9];
    assert!(ReviewDraft::rule(vec![target], vec![], bad).is_err());
}

#[test]
fn review_wire_contract_cannot_treat_rule_pass_as_human_approval() {
    let human = serde_json::json!({ "source": "human", "policy": policy(), "verdict": "approved", "comment": "" });
    assert!(serde_json::from_value::<ReviewJudgment>(human.clone()).is_ok());
    let mut rule = serde_json::json!({ "source": "rule", "assessment": assessment() });
    assert!(serde_json::from_value::<ReviewJudgment>(rule.clone()).is_ok());
    rule["verdict"] = "approved".into();
    assert!(serde_json::from_value::<ReviewJudgment>(rule).is_err());
    for source in ["agent", "latest", "approved"] {
        let mut invalid = human.clone();
        invalid["source"] = source.into();
        assert!(serde_json::from_value::<ReviewJudgment>(invalid).is_err());
    }
    for status in ["accepted", "completed"] {
        assert!(serde_json::from_value::<TaskStatus>(status.into()).is_err());
    }
}

#[tokio::test]
async fn unavailable_review_storage_does_not_return_a_volatile_judgment() {
    let pool = PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(100))
        .connect_lazy("postgres://unavailable:unavailable@127.0.0.1:1/unavailable")
        .unwrap();
    let store = ReviewStore::new(pool, scope());
    assert_eq!(
        store.create(id(), reviewer(), draft(artifact())).await,
        Err(ReviewStoreError::StorageUnavailable)
    );
}

#[test]
fn review_storage_has_no_teaching_or_live_authorization_shortcut() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for file in [
        "mod.rs",
        "draft.rs",
        "commands.rs",
        "records.rs",
        "schema.rs",
    ] {
        let source =
            std::fs::read_to_string(root.join("src/services/review_store").join(file)).unwrap();
        for forbidden in [
            "LearningStore",
            "EbpfTeachingPack",
            "LegacyRole",
            "std::env",
            "AppState",
        ] {
            assert!(!source.contains(forbidden), "{file}: {forbidden}");
        }
    }
    for file in ["src/state.rs", "src/application.rs"] {
        assert!(!std::fs::read_to_string(root.join(file))
            .unwrap()
            .contains("ReviewStore"));
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_reviews_preserve_pins_and_comment_history_after_reopen() {
    let f = Fixture::ready().await;
    let target = artifact();
    let original = f
        .store
        .create(id(), reviewer(), draft(target.clone()))
        .await
        .unwrap();
    assert_eq!(original.reference.revision, 1.try_into().unwrap());
    assert_eq!(original.supersedes, None);
    let revised = f
        .store
        .revise(
            original.reference,
            reviewer(),
            edit(HumanReviewVerdict::ChangesRequested, "Clarify paragraph 2"),
        )
        .await
        .unwrap();
    assert_eq!(revised.reference.revision, 2.try_into().unwrap());
    assert_eq!(revised.supersedes, Some(original.reference));
    assert_eq!(revised.targets, vec![target]);
    let reopened = ReviewStore::new(f.pool.clone(), scope());
    assert_eq!(
        reopened.read(original.reference, reviewer()).await.unwrap(),
        Some(original.clone())
    );
    assert_eq!(
        reopened.read(revised.reference, reviewer()).await.unwrap(),
        Some(revised)
    );
    assert_eq!(
        f.store
            .create(original.reference.review_id, reviewer(), draft(artifact()))
            .await,
        Err(ReviewStoreError::Conflict)
    );
    assert_eq!(f.count("collaboration_review_revisions").await, 2);
    assert_eq!(f.count("collaboration_review_outbox").await, 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_reviews_reviewer_scope_and_exact_revision_are_not_interchangeable() {
    let f = Fixture::ready().await;
    let original = f
        .store
        .create(id(), reviewer(), draft(artifact()))
        .await
        .unwrap();
    let other = PrincipalRef {
        principal_id: id(),
        ..reviewer()
    };
    assert_eq!(f.store.read(original.reference, other).await.unwrap(), None);
    assert_eq!(
        f.store
            .revise(
                original.reference,
                other,
                edit(HumanReviewVerdict::Approved, "")
            )
            .await,
        Err(ReviewStoreError::NotFound)
    );
    let missing = ReviewRef {
        revision: 2.try_into().unwrap(),
        ..original.reference
    };
    assert_eq!(f.store.read(missing, reviewer()).await.unwrap(), None);
    let foreign = WorkspaceRef {
        workspace_id: id(),
        ..scope()
    };
    assert_eq!(
        f.store
            .read(
                ReviewRef {
                    workspace: foreign,
                    ..original.reference
                },
                reviewer()
            )
            .await,
        Err(ReviewStoreError::ScopeMismatch)
    );
    let mut target = artifact();
    target.workspace = foreign;
    assert_eq!(
        f.store.create(id(), reviewer(), draft(target)).await,
        Err(ReviewStoreError::ScopeMismatch)
    );
    let bad = PrincipalRef {
        authority_id: id(),
        ..reviewer()
    };
    assert_eq!(
        f.store.create(id(), bad, draft(artifact())).await,
        Err(ReviewStoreError::ScopeMismatch)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_reviews_concurrent_amendments_have_one_winner() {
    let f = Fixture::ready().await;
    let first = f
        .store
        .create(id(), reviewer(), draft(artifact()))
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        f.store.revise(
            first.reference,
            reviewer(),
            edit(HumanReviewVerdict::Commented, "A")
        ),
        f.store.revise(
            first.reference,
            reviewer(),
            edit(HumanReviewVerdict::ChangesRequested, "B")
        ),
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert_eq!(
        if a.is_err() { a } else { b },
        Err(ReviewStoreError::StaleRevision)
    );
    assert_eq!(f.count("collaboration_review_revisions").await, 2);
    assert_eq!(f.count("collaboration_review_outbox").await, 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_reviews_rule_observations_cannot_be_amended_to_human_judgments() {
    let f = Fixture::ready().await;
    let result = assessment();
    let review = f
        .store
        .create(
            id(),
            reviewer(),
            ReviewDraft::rule(vec![artifact()], vec![], result.clone()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(review.judgment, ReviewJudgment::Rule { assessment: result });
    assert_eq!(
        f.store
            .revise(
                review.reference,
                reviewer(),
                edit(HumanReviewVerdict::Approved, "")
            )
            .await,
        Err(ReviewStoreError::NotAmendable)
    );
    assert_eq!(f.count("collaboration_review_outbox").await, 1);
    f.cleanup().await;
}
