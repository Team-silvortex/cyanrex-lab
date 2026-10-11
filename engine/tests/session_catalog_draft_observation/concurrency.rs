use super::fixture::*;
use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_observation_cancelled_artifact_and_task_reads_preserve_all_state_rows_files_and_pool(
) {
    for task in [false, true] {
        let f = Fixture::ready().await;
        let mut attempt = if task {
            let attempt = unknown_task(&f, 0, ITEMS).await;
            create_target(&f, &attempt, &f.auth.learner_token, 0, ITEMS).await;
            attempt
        } else {
            let attempt = unknown_artifact(&f, 0, ITEMS, 0).await;
            publish_target(&f, &attempt, &f.auth.learner_token, ITEMS, 0).await;
            attempt
        };
        let before = snapshot(&f).await;
        let saved = progress(&attempt);
        let mut holder = lock_resource(&f, task).await;
        let blocker = backend(&mut holder).await;
        let mut pending = Box::pin(attempt.observe_unconfirmed(&f.auth.learner_token));
        let waited = tokio::select! { _ = &mut pending => false, waiter = blocked_by(&f,blocker) => waiter.is_some() };
        drop(pending);
        holder.rollback().await.unwrap();
        f.auth.base.drain().await;
        f.assert_pool_restored().await;
        assert!(
            waited,
            "cancel only after the exact reader is demonstrably blocked"
        );
        assert_eq!(snapshot(&f).await, before);
        assert_eq!(progress(&attempt), saved);
        assert_eq!(
            observe_preserved(&f, &attempt).await,
            report(&attempt, SessionDraftObservedOutcome::MatchesPlannedCreate)
        );
        stopped(&f, &mut attempt).await;
        drop(attempt);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_observation_pending_creator_visibility_cannot_prove_previous_or_later_commit(
) {
    for task in [false, true] {
        for commit in [false, true] {
            let f = Fixture::ready().await;
            let mut attempt = if task {
                unknown_task(&f, 0, &[]).await
            } else {
                unknown_artifact(&f, 0, ITEMS, 0).await
            };
            let saved = progress(&attempt);
            let mut holder = if task {
                f.pause_task_event().await
            } else {
                pause_artifact_event(&f).await
            };
            let blocker = backend(&mut holder).await;
            let source = f.auth.source.clone();
            let token = f.auth.learner_token.clone();
            let artifacts = f.artifact_workspace.clone();
            let catalogue = workspace(&f);
            let task_id = attempt.task_ref().task_id;
            let reference = values()[0].reference.clone();
            let artifact = attempt.allocated_artifacts().first().cloned();
            // This is a separately explicit creator, not a fabricated lost acknowledgement.
            let creator = tokio::spawn(async move {
                if task {
                    source
                        .create_session_catalog_content_task(
                            &token,
                            &catalogue,
                            task_id,
                            &reference,
                            TaskContentManifest::new(plan(&[]).title(), vec![]).unwrap(),
                        )
                        .await
                        .map(|_| ())
                        .map_err(SessionDraftPublicationError::Task)
                } else {
                    let artifact = artifact.unwrap();
                    source
                        .create_session_artifact(
                            &token,
                            &artifacts,
                            artifact.artifact_id,
                            artifact.revision_id,
                            plan(ITEMS).publication_draft(0).unwrap(),
                        )
                        .await
                        .map(|_| ())
                        .map_err(SessionDraftPublicationError::Artifact)
                }
            });
            let creator_pid = blocked_by(&f, blocker).await;
            let mut pending = Box::pin(attempt.observe_unconfirmed(&f.auth.learner_token));
            let mut early = None;
            let waited = if let Some(pid) = creator_pid {
                tokio::select! { result = &mut pending => { early=Some(result); false }, waiter = blocked_by(&f,pid) => waiter.is_some() }
            } else {
                false
            };
            if !commit {
                creator.abort();
            }
            holder.rollback().await.unwrap();
            let created = creator.await;
            let first = match early {
                Some(result) => result,
                None => pending.as_mut().await,
            };
            drop(pending);
            f.auth.base.drain().await;
            f.assert_pool_restored().await;
            assert!(
                creator_pid.is_some(),
                "creator must reach its exact outbox barrier first"
            );
            let absent = report(&attempt, SessionDraftObservedOutcome::NotVisible);
            let matching = report(&attempt, SessionDraftObservedOutcome::MatchesPlannedCreate);
            if commit {
                assert!(created.unwrap().is_ok());
                assert!(
                    first == absent || (waited && first == matching),
                    "visibility is allowed to precede or follow that independent commit"
                );
                assert_eq!(observe_preserved(&f, &attempt).await, matching);
            } else {
                assert!(created.is_err());
                assert_eq!(first, absent);
                assert_eq!(observe_preserved(&f, &attempt).await, absent);
            }
            assert_eq!(progress(&attempt), saved);
            assert_eq!(
                counts(&f).await,
                if task {
                    if commit {
                        [0, 0, 0, 1, 1, 0]
                    } else {
                        [0, 0, 0, 0, 0, 0]
                    }
                } else {
                    if commit {
                        [1, 1, 1, 0, 0, 1]
                    } else {
                        [0, 0, 0, 0, 0, 1]
                    }
                }
            );
            stopped(&f, &mut attempt).await;
            drop(attempt);
            f.cleanup().await;
        }
    }
}
