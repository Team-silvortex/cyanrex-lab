use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_inspection_cancelled_read_preserves_checkpoint_sql_files_and_pool() {
    for task in [false, true] {
        let f = Fixture::ready().await;
        let (attempt, checkpoint) = visible(&f, task).await;
        drop(attempt);
        let saved = checkpoint.to_json().unwrap();
        let before = snapshot(&f).await;
        let mut holder = schema_lock(&f, task).await;
        let blocker = backend(&mut holder).await;
        let mut pending = Box::pin(inspect(&f, &f.auth.learner_token, &checkpoint));
        let waited = tokio::select! {
            _ = &mut pending => false,
            waiter = blocked_by(&f, blocker) => waiter.is_some(),
        };
        drop(pending);
        holder.rollback().await.unwrap();
        f.auth.base.drain().await;
        f.assert_pool_restored().await;
        let after_cancel = snapshot(&f).await;
        let later = inspect(&f, &f.auth.learner_token, &checkpoint).await;
        let after_later = snapshot(&f).await;
        let expected = report(
            &checkpoint,
            SessionDraftCheckpointObservedOutcome::MatchesCheckpointMetadata,
        );
        let unchanged = checkpoint.to_json().unwrap();
        f.cleanup().await;
        assert!(
            waited,
            "cancel after the inspection reaches its exact resource metadata blocker"
        );
        assert_eq!(before, after_cancel);
        assert_eq!(before, after_later);
        assert_eq!(later, expected);
        assert_eq!(saved, unchanged);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_inspection_pending_creator_visibility_never_reserves_or_recovers_targets() {
    for mode in ["abort", "commit", "deferred_failure"] {
        let f = Fixture::ready().await;
        let attempt = unknown_artifact(&f, ITEMS, 0).await;
        let checkpoint = checkpoint(&attempt);
        let reference = attempt.allocated_artifacts()[0].clone();
        drop(attempt);
        let saved = checkpoint.to_json().unwrap();
        let source = f.auth.source.clone();
        let workspace = f.artifact_workspace.clone();
        let token = f.auth.learner_token.clone();
        let draft = plan(ITEMS).publication_draft(0).unwrap();
        if mode == "deferred_failure" {
            f.sql_artifact("CREATE SEQUENCE draft_fault_calls").await;
            f.sql_artifact(&format!(
                "CREATE FUNCTION draft_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
                 PERFORM nextval('{}.draft_fault_calls'); RAISE EXCEPTION 'synthetic commit failure'; END $$",
                f.artifact_schema,
            )).await;
            f.sql_artifact("CREATE CONSTRAINT TRIGGER draft_fault AFTER INSERT ON collaboration_artifact_outbox
                DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION draft_fault()").await;
        }
        let mut holder = pause_artifact_event(&f).await;
        let blocker = backend(&mut holder).await;
        let creator = tokio::spawn(async move {
            source
                .create_session_artifact(
                    &token,
                    &workspace,
                    reference.artifact_id,
                    reference.revision_id,
                    draft,
                )
                .await
        });
        let creator_pid = blocked_by(&f, blocker).await;
        let mut pending = Box::pin(inspect(&f, &f.auth.learner_token, &checkpoint));
        let mut early = None;
        let read_waited = if let Some(pid) = creator_pid {
            tokio::select! {
                result = &mut pending => { early = Some(result); false },
                waiter = blocked_by(&f, pid) => waiter.is_some(),
            }
        } else {
            false
        };
        // An uncommitted insert may be invisible at READ COMMITTED. Neither early None nor a
        // later match certifies this separate creator's provenance or authorizes another write.
        if mode == "abort" {
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
        let hit = if mode == "deferred_failure" {
            Some(artifact_fault_calls(&f).await)
        } else {
            None
        };
        let before = snapshot(&f).await;
        let final_read = inspect(&f, &f.auth.learner_token, &checkpoint).await;
        let after = snapshot(&f).await;
        let absent = report(
            &checkpoint,
            SessionDraftCheckpointObservedOutcome::NotVisible,
        );
        let matching = report(
            &checkpoint,
            SessionDraftCheckpointObservedOutcome::MatchesCheckpointMetadata,
        );
        let unchanged = checkpoint.to_json().unwrap();
        let remaining = counts(&f).await;
        f.cleanup().await;
        assert!(
            creator_pid.is_some(),
            "creator must reach the exact outbox wait before inspection"
        );
        if mode == "commit" {
            assert!(created.unwrap().is_ok());
            assert!(first == absent || (read_waited && first == matching));
            assert_eq!(final_read, matching);
            assert_eq!(remaining, [1, 1, 1, 0, 0, 1]);
        } else {
            if mode == "abort" {
                assert!(created.is_err());
            } else {
                assert_eq!(
                    created.unwrap(),
                    Err(SessionArtifactError::Session(
                        SessionCommandError::Authentication(DurableAuthError::StorageUnavailable),
                    ))
                );
                assert_eq!(hit, Some((true, 1)));
            }
            assert_eq!(first, absent);
            assert_eq!(final_read, absent);
            assert_eq!(remaining, [0, 0, 0, 0, 0, 1]);
        }
        assert_eq!(before, after);
        assert_eq!(saved, unchanged);
    }
}
