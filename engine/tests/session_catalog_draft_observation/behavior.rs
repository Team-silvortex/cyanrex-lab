use super::fixture::*;
use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_observation_preflight_ready_complete_and_replaced_session_never_reads() {
    let f = Fixture::ready().await;
    let mut ready = prepare_catalog(&f, 1, &[]);
    drop(ready.advance(&f.auth.learner_token));
    let mut unknown = unknown_artifact(&f, 1, ITEMS, 0).await;
    let account = f
        .auth
        .source
        .validate_session(&f.auth.learner_token)
        .await
        .unwrap()
        .unwrap()
        .account;
    let fresh = f.auth.base.seed_session(&account).await;
    let before = snapshot(&f).await;
    let state = progress(&unknown);
    let mut holder = f.auth.base.pool.begin().await.unwrap();
    query("SELECT singleton FROM collaboration_auth_source_schema FOR UPDATE")
        .fetch_one(&mut *holder)
        .await
        .unwrap();
    let results = tokio::time::timeout(Duration::from_secs(1), async {
        let mut results = Vec::new();
        for token in [&fresh.token, &f.auth.manager_token, &"invalid".to_string()] {
            results.push(unknown.observe_unconfirmed(token).await);
            assert_eq!(
                ready.observe_unconfirmed(token).await,
                Err(SessionDraftPublicationError::NoUnconfirmedStep)
            );
        }
        assert_eq!(
            ready.observe_unconfirmed(&f.auth.learner_token).await,
            Err(SessionDraftPublicationError::NoUnconfirmedStep)
        );
        results
    })
    .await;
    holder.rollback().await.unwrap();
    assert!(results
        .unwrap()
        .into_iter()
        .all(|result| result == Err(SessionDraftPublicationError::SessionChanged)));
    assert_eq!(snapshot(&f).await, before);
    assert_eq!(progress(&unknown), state);
    let unpolled = snapshot(&f).await;
    drop(unknown.observe_unconfirmed(&f.auth.learner_token));
    assert_eq!(snapshot(&f).await, unpolled);
    stopped(&f, &mut unknown).await;
    ready.advance(&f.auth.learner_token).await.unwrap();
    let before = snapshot(&f).await;
    let complete = progress(&ready);
    assert_eq!(
        ready.observe_unconfirmed(&f.auth.learner_token).await,
        Err(SessionDraftPublicationError::NoUnconfirmedStep)
    );
    assert_eq!(
        ready.observe_unconfirmed(&fresh.token).await,
        Err(SessionDraftPublicationError::NoUnconfirmedStep)
    );
    assert_eq!(snapshot(&f).await, before);
    assert_eq!(progress(&ready), complete);
    drop(ready);
    drop(unknown);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_observation_artifact_match_differs_historical_revision_and_prefix_scope()
{
    for different in [false, true] {
        let f = Fixture::ready().await;
        let mut attempt = prepare_catalog(&f, 0, ITEMS);
        let target = attempt.allocated_artifacts()[0].clone();
        let draft = if different {
            ArtifactDraft::new(
                "cyanrex.task-text".parse().unwrap(),
                "Legitimate different title",
                "text/plain",
                ITEMS[0].2.as_bytes().to_vec(),
            )
            .unwrap()
        } else {
            plan(ITEMS).publication_draft(0).unwrap()
        };
        let first = f
            .auth
            .source
            .create_session_artifact(
                &f.auth.learner_token,
                &f.artifact_workspace,
                target.artifact_id,
                target.revision_id,
                draft,
            )
            .await
            .unwrap();
        assert_eq!(
            attempt.advance(&f.auth.learner_token).await,
            Err(SessionDraftPublicationError::Artifact(
                SessionArtifactError::Artifact(ArtifactStoreError::Conflict)
            ))
        );
        let result = if different {
            SessionDraftObservedOutcome::DiffersFromPlannedCreate
        } else {
            SessionDraftObservedOutcome::MatchesPlannedCreate
        };
        assert_eq!(
            observe_preserved(&f, &attempt).await,
            report(&attempt, result)
        );
        if !different {
            let later = f
                .auth
                .source
                .revise_session_artifact(
                    &f.auth.learner_token,
                    &f.artifact_workspace,
                    &first.reference,
                    id(),
                    ArtifactDraft::new(
                        "cyanrex.task-text".parse().unwrap(),
                        plan(ITEMS).title(),
                        "text/plain",
                        b"Later revision".to_vec(),
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
            assert_ne!(later.reference, first.reference);
            assert_eq!(
                observe_preserved(&f, &attempt).await,
                report(&attempt, SessionDraftObservedOutcome::MatchesPlannedCreate),
                "exact historical revision remains observable after head advances"
            );
        }
        stopped(&f, &mut attempt).await;
        drop(attempt);
        f.cleanup().await;
    }
    let f = Fixture::ready().await;
    let items = [("first", "text", "same"), ("second", "text", "same")];
    let mut attempt = unknown_artifact(&f, 1, &items, 1).await;
    publish_target(&f, &attempt, &f.auth.learner_token, &items, 1).await;
    damage(&f, &attempt.confirmed_artifacts()[0].reference, false);
    assert_eq!(
        observe_preserved(&f, &attempt).await,
        report(&attempt, SessionDraftObservedOutcome::MatchesPlannedCreate),
        "Artifact observation must not claim a prefix revalidation"
    );
    stopped(&f, &mut attempt).await;
    drop(attempt);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_observation_task_full_definitions_versions_empty_order_and_edits_never_adopt(
) {
    for entry in 0..values().len() {
        for empty in [false, true] {
            let f = Fixture::ready().await;
            let items = if empty {
                vec![]
            } else {
                vec![
                    ("same.txt", "future.lang", "same😀"),
                    ("same.txt", "text", "same😀"),
                ]
            };
            let mut attempt = unknown_task(&f, entry, &items).await;
            let target = create_target(&f, &attempt, &f.auth.learner_token, entry, &items).await;
            assert_eq!(target.task.definition, Some(values()[entry].clone()));
            assert_eq!(
                observe_preserved(&f, &attempt).await,
                report(&attempt, SessionDraftObservedOutcome::MatchesPlannedCreate)
            );
            let manifest = TaskContentManifest::new(
                "Later edited task",
                target.manifest.payload().iter().rev().cloned().collect(),
            )
            .unwrap();
            let edited = f
                .auth
                .source
                .replace_session_catalog_content_task(
                    &f.auth.learner_token,
                    &workspace(&f),
                    target.task.reference.task_id,
                    target.task.revision,
                    manifest,
                )
                .await
                .unwrap();
            assert_eq!(
                observe_preserved(&f, &attempt).await,
                report(
                    &attempt,
                    SessionDraftObservedOutcome::DiffersFromPlannedCreate
                )
            );
            f.auth
                .source
                .transition_session_catalog_content_task(
                    &f.auth.learner_token,
                    &workspace(&f),
                    edited.task.reference.task_id,
                    edited.task.revision,
                    TaskStatus::Ready,
                )
                .await
                .unwrap();
            assert_eq!(
                observe_preserved(&f, &attempt).await,
                report(
                    &attempt,
                    SessionDraftObservedOutcome::DiffersFromPlannedCreate
                )
            );
            stopped(&f, &mut attempt).await;
            drop(attempt);
            f.cleanup().await;
        }
    }
    for other in [1, 2] {
        let f = Fixture::ready().await;
        let mut attempt = unknown_task(&f, 0, &[]).await;
        let target = create_target(&f, &attempt, &f.auth.learner_token, other, &[]).await;
        assert_ne!(target.task.definition.as_ref(), Some(attempt.definition()));
        assert_eq!(
            observe_preserved(&f, &attempt).await,
            report(
                &attempt,
                SessionDraftObservedOutcome::DiffersFromPlannedCreate
            ),
            "valid other package/version must not fallback to the attempted definition"
        );
        stopped(&f, &mut attempt).await;
        drop(attempt);
        f.cleanup().await;
    }
}
