use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_reviews_reject_rule_source_and_unconfigured_human_policy() {
    let f = Fixture::ready().await;
    let token = &f.auth.learner_token;
    let target = f.create_artifact(token, b"Target").await;
    let mut changed_name = policy();
    changed_name.name = "sample.document.other".parse().unwrap();
    let mut changed_version = policy();
    changed_version.version = 2.try_into().unwrap();
    let drafts = [
        ReviewDraft::rule(vec![target.reference.clone()], vec![], assessment()).unwrap(),
        ReviewDraft::human(
            vec![target.reference.clone()],
            vec![],
            changed_name,
            edit(HumanReviewVerdict::Approved, ""),
        )
        .unwrap(),
        ReviewDraft::human(
            vec![target.reference],
            vec![],
            changed_version,
            edit(HumanReviewVerdict::Approved, ""),
        )
        .unwrap(),
    ];
    for draft in drafts {
        let record = f.reviews.create(id(), target.owner, draft).await.unwrap();
        assert_eq!(
            f.read(token, record.reference).await,
            Err(SessionReviewError::UnsupportedReview)
        );
        assert_eq!(
            f.revise(
                token,
                record.reference,
                edit(HumanReviewVerdict::Approved, "No conversion")
            )
            .await,
            Err(SessionReviewError::UnsupportedReview)
        );
        assert_eq!(
            f.read(&f.auth.manager_token, record.reference)
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            f.reviews
                .read(record.reference, record.reviewer)
                .await
                .unwrap(),
            Some(record)
        );
    }
    assert_eq!(f.count_review("collaboration_review_outbox").await, 3);
    assert_eq!(f.files(), 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_reviews_unbound_missing_membership_and_revoked_sessions_cannot_act() {
    let f = Fixture::ready().await;
    let target = f.create_artifact(&f.auth.learner_token, b"Private").await;
    let first = f
        .create(&f.auth.learner_token, vec![target.reference], vec![])
        .await;
    for bind_target in [false, true] {
        if bind_target {
            f.auth.bind_target().await;
        }
        assert!(f.read(&f.auth.target_token, first.reference).await.is_err());
        assert!(f
            .request(&f.auth.target_token, id(), first.targets.clone(), vec![])
            .await
            .is_err());
        assert!(f
            .revise(
                &f.auth.target_token,
                first.reference,
                edit(HumanReviewVerdict::Commented, "Not admitted")
            )
            .await
            .is_err());
    }
    f.auth.source.logout(&f.auth.learner_token).await.unwrap();
    let invalid = SessionReviewError::Session(SessionCommandError::InvalidSession);
    assert_eq!(
        f.read(&f.auth.learner_token, first.reference).await,
        Err(invalid)
    );
    assert_eq!(
        f.request(&f.auth.learner_token, id(), first.targets.clone(), vec![])
            .await,
        Err(invalid)
    );
    assert_eq!(
        f.revise(
            &f.auth.learner_token,
            first.reference,
            edit(HumanReviewVerdict::Commented, "Revoked")
        )
        .await,
        Err(invalid)
    );
    assert_eq!(f.count_review("collaboration_review_outbox").await, 1);
    assert_eq!(f.files(), 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_reviews_retired_and_recreated_accounts_cannot_inherit_judgments() {
    let f = Fixture::ready().await;
    let target = f
        .create_artifact(&f.auth.learner_token, b"Old account target")
        .await;
    let first = f
        .create(&f.auth.learner_token, vec![target.reference], vec![])
        .await;
    let old = &f.auth.learner;
    f.auth
        .store
        .apply_legacy_identity_command(&LegacyIdentityCommand {
            command_id: id(),
            actor: f.auth.manager.principal.reference,
            workspace: scope(),
            action: LegacyIdentityAction::Retire {
                username: old.binding.username.clone(),
                account_id: old.account_id,
                expected_principal: old.binding.principal_id,
            },
        })
        .await
        .unwrap();
    assert!(f
        .auth
        .source
        .validate_session(&f.auth.learner_token)
        .await
        .unwrap()
        .is_some());
    assert!(f
        .read(&f.auth.learner_token, first.reference)
        .await
        .is_err());
    assert!(f
        .request(&f.auth.learner_token, id(), first.targets.clone(), vec![])
        .await
        .is_err());
    assert!(f
        .revise(
            &f.auth.learner_token,
            first.reference,
            edit(HumanReviewVerdict::Commented, "Retired")
        )
        .await
        .is_err());
    let (token, principal) = f.replace_retired_learner().await;
    assert_ne!(principal, first.reviewer);
    assert_eq!(f.read(&token, first.reference).await.unwrap(), None);
    assert_eq!(
        f.revise(
            &token,
            first.reference,
            edit(HumanReviewVerdict::Commented, "Replacement")
        )
        .await,
        Err(SessionReviewError::Review(ReviewStoreError::NotFound))
    );
    assert_eq!(
        f.request(&token, id(), first.targets.clone(), vec![]).await,
        Err(SessionReviewError::Artifact(ArtifactStoreError::NotFound))
    );
    let own = f.create_artifact(&token, b"New account target").await;
    let replacement = f.create(&token, vec![own.reference], vec![]).await;
    assert_eq!(replacement.reviewer, principal);
    assert_eq!(
        f.reviews
            .read(first.reference, first.reviewer)
            .await
            .unwrap(),
        Some(first)
    );
    assert_eq!(f.count_review("collaboration_review_outbox").await, 2);
    assert_eq!(f.files(), 2);
    f.cleanup().await;
}
