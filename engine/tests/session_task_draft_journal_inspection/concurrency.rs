use super::*;
use std::os::unix::fs::PermissionsExt;
#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_inspection_cancelled_and_concurrent_reads_leave_resources_untouched(
) {
    for rollback in [false, true] {
        let f = Fixture::ready().await;
        let mut attempt = f.journaled(ITEMS).await;
        attempt.advance(&f.auth.learner_token).await.unwrap();
        let task = attempt.task_ref();
        let second_ref = attempt.allocated_artifacts()[1].clone();
        if rollback {
            f.resource_fault(false, "RETURN NULL;", false).await;
        }
        let mut holder = f.pause_phase("UPDATE").await;
        let blocker = backend(&mut holder).await;
        let mut writer = Box::pin(attempt.advance(&f.auth.learner_token));
        let mut early_writer = None;
        let writer_pid = tokio::select! {
            result = &mut writer => { early_writer = Some(result); None },
            waiter = blocked_by(&f, blocker) => waiter,
        };
        let mut reader = Box::pin(inspect(&f, &f.auth.learner_token, task.task_id));
        let mut cancelled_waited = false;
        if let Some(pid) = writer_pid {
            cancelled_waited = tokio::select! { _ = &mut reader => false, waiter = blocked_by(&f, pid) => waiter.is_some() };
        }
        drop(reader);
        let writer_still_waiting: bool = query("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid = $1 AND $2 = ANY(pg_blocking_pids(pid))) AS waiting")
        .bind(writer_pid.unwrap_or(0)).bind(blocker).fetch_one(&f.auth.base.admin).await.unwrap().get("waiting");
        let mut reader = Box::pin(inspect(&f, &f.auth.learner_token, task.task_id));
        let mut early_reader = None;
        let mut second_waited = false;
        if let Some(pid) = writer_pid {
            second_waited = tokio::select! {
                result = &mut reader => { early_reader = Some(result); false },
                waiter = blocked_by(&f, pid) => waiter.is_some(),
            };
        }
        holder.rollback().await.unwrap();
        let written = match early_writer {
            Some(value) => value,
            None => writer.as_mut().await,
        };
        drop(writer);
        let observed = match early_reader {
            Some(value) => value,
            None => reader.as_mut().await,
        };
        drop(reader);
        f.auth.base.drain().await;
        f.assert_pool_restored().await;
        // Deliberately break resources after the writer completes. Inspection may report the
        // recorded prefix, but cannot claim to verify current Task/Artifact rows or their bytes.
        f.sql_task("UPDATE collaboration_task_schema SET version = 999")
            .await;
        f.sql_artifact("UPDATE collaboration_artifact_schema SET version = 999")
            .await;
        // Deliberately override read-only permissions only on these exact, test-owned files.
        // Production immutable blobs stay 0400; this is synthetic corruption, not a write API.
        let fault_injected: std::io::Result<()> = (|| {
            for entry in fs::read_dir(&f.directory)? {
                let path = entry?.path();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
                fs::write(&path, b"synthetic corrupt blob")?;
            }
            Ok(())
        })();
        let before = snapshot(&f).await;
        let without_resources = inspect(&f, &f.auth.learner_token, task.task_id).await;
        let repeated = inspect(&f, &f.auth.learner_token, task.task_id).await;
        let intact = before == snapshot(&f).await;
        let attempt_expected = if rollback {
            dispatch_fixture::unknown(&attempt) && attempt.confirmed_artifacts().len() == 1
        } else {
            attempt.state() == &SessionDraftPublicationState::Ready
                && attempt.confirmed_artifacts().len() == 2
        };
        let fault_hit = !rollback || f.resource_hit(false).await;
        f.cleanup().await;
        assert!(
            fault_injected.is_ok(),
            "synthetic corruption failed: {fault_injected:?}"
        );
        assert!(
            cancelled_waited
                && writer_still_waiting
                && second_waited
                && intact
                && attempt_expected
                && fault_hit
        );
        if rollback {
            assert!(written.is_err());
        } else {
            assert_eq!(
                written,
                Ok(SessionDraftPublicationProgress::ArtifactConfirmed { index: 1 })
            );
        }
        assert_eq!(without_resources, observed);
        assert_eq!(repeated, observed);
        let value = observed.unwrap().unwrap();
        assert_eq!(value.4, if rollback { 1 } else { 2 });
        assert_eq!(
            value.5,
            if rollback {
                Some(SessionDraftPublicationStep::Artifact {
                    index: 1,
                    reference: second_ref,
                })
            } else {
                None
            }
        );
        assert!(!value.6);
    }
}
