use super::*;
use std::{os::unix::fs::PermissionsExt, time::Duration};

type Tx<'a> = sqlx_core::transaction::Transaction<'a, sqlx_postgres::Postgres>;
async fn backend(tx: &mut Tx<'_>) -> i32 {
    query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
        .get("pid")
}
async fn blocked_by(f: &Fixture, blocker: i32) -> i32 {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(row) = query(
                "SELECT pid FROM pg_stat_activity WHERE datname = current_database()
                AND $1 = ANY(pg_blocking_pids(pid)) ORDER BY pid LIMIT 1",
            )
            .bind(blocker)
            .fetch_optional(&f.auth.base.admin)
            .await
            .unwrap()
            {
                return row.get("pid");
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("content command did not reach the exact synthetic blocker")
}
fn spawn_replace(
    f: &Fixture,
    task: &TaskContentSnapshot,
    value: TaskContentManifest,
) -> tokio::task::JoinHandle<Result<TaskContentSnapshot, SessionTaskError>> {
    let source = f.auth.source.clone();
    let token = f.auth.learner_token.clone();
    let workspace = f.workspace.clone();
    let task = task.task.clone();
    tokio::spawn(async move {
        source
            .replace_session_content_task(
                &token,
                &workspace,
                task.reference.task_id,
                task.revision,
                value,
            )
            .await
    })
}
async fn expire_soon(f: &Fixture) {
    query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1")
        .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
}
async fn wait_expired(f: &Fixture) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let expired: bool = query(&format!(
                "SELECT clock_timestamp() >= expires_at AS expired
                FROM {}.sessions WHERE token = $1",
                f.auth.base.schema
            ))
            .bind(source_fixture::token_hash(&f.auth.learner_token))
            .fetch_one(&f.auth.base.admin)
            .await
            .unwrap()
            .get("expired");
            if expired {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("synthetic session did not expire");
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_competing_edits_have_one_task_manifest_and_event_winner() {
    let (f, task, old, new) = pair().await;
    let mut holder = f.pause_task_event().await;
    let blocker = backend(&mut holder).await;
    let first = spawn_replace(&f, &task, manifest("First", vec![new.reference]));
    let first_pid = blocked_by(&f, blocker).await;
    let second = spawn_replace(&f, &task, manifest("Second", vec![old.reference]));
    blocked_by(&f, first_pid).await;
    holder.rollback().await.unwrap();
    let winner = first.await.unwrap().unwrap();
    assert_eq!(
        second.await.unwrap(),
        Err(SessionTaskError::Task(TaskStoreError::StaleRevision))
    );
    assert_eq!(u64::from(winner.task.revision), 2);
    assert_eq!(winner.manifest.title(), "First");
    f.unchanged(&winner, 2).await;
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_opposite_old_new_unions_use_one_artifact_lock_order() {
    let (f, first, old, new) = pair().await;
    let second = f.create(vec![new.reference.clone()]).await;
    let a = spawn_replace(&f, &first, manifest("A takes B", vec![new.reference]));
    let b = spawn_replace(&f, &second, manifest("B takes A", vec![old.reference]));
    let (one, two) = tokio::time::timeout(Duration::from_secs(5), async { tokio::join!(a, b) })
        .await
        .unwrap();
    let one = one.unwrap().unwrap();
    let two = two.unwrap().unwrap();
    assert_eq!(u64::from(one.task.revision), 2);
    assert_eq!(u64::from(two.task.revision), 2);
    f.unchanged(&one, 4).await;
    f.unchanged(&two, 4).await;
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_expiry_after_task_artifact_event_and_empty_metadata_waits_never_commits(
) {
    for stage in ["task", "artifact", "event", "empty_metadata"] {
        let (f, populated, old, new) = pair().await;
        let task = if stage == "empty_metadata" {
            f.create(vec![]).await
        } else {
            populated
        };
        let mut holder = match stage {
            "task" => {
                let mut tx = f.task_pool.begin().await.unwrap();
                query("SELECT task_id FROM collaboration_tasks WHERE task_id = $1 FOR UPDATE")
                    .bind(task.task.reference.task_id.as_uuid())
                    .fetch_one(&mut *tx)
                    .await
                    .unwrap();
                tx
            }
            "artifact" => {
                let mut tx = f.artifact_pool.begin().await.unwrap();
                query("SELECT artifact_id FROM collaboration_artifacts WHERE artifact_id = $1 FOR UPDATE")
                    .bind(old.reference.artifact_id.as_uuid()).fetch_one(&mut *tx).await.unwrap();
                tx
            }
            "empty_metadata" => {
                let mut tx = f.artifact_pool.begin().await.unwrap();
                query("SELECT singleton FROM collaboration_artifact_schema FOR UPDATE")
                    .fetch_one(&mut *tx)
                    .await
                    .unwrap();
                tx
            }
            _ => f.pause_task_event().await,
        };
        let blocker = backend(&mut holder).await;
        expire_soon(&f).await;
        let refs = if stage == "empty_metadata" {
            vec![]
        } else {
            vec![new.reference]
        };
        let pending = spawn_replace(&f, &task, manifest("Expired edit", refs));
        blocked_by(&f, blocker).await;
        wait_expired(&f).await;
        assert!(!pending.is_finished());
        holder.rollback().await.unwrap();
        assert_eq!(
            pending.await.unwrap(),
            Err(SessionTaskError::Session(
                SessionCommandError::InvalidSession
            )),
            "{stage}"
        );
        f.unchanged(&task, if stage == "empty_metadata" { 2 } else { 1 })
            .await;
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_expired_read_wait_returns_no_retained_bytes() {
    let (f, task, old, _) = pair().await;
    let mut holder = f.artifact_pool.begin().await.unwrap();
    query("SELECT artifact_id FROM collaboration_artifacts WHERE artifact_id = $1 FOR UPDATE")
        .bind(old.reference.artifact_id.as_uuid())
        .fetch_one(&mut *holder)
        .await
        .unwrap();
    let blocker = backend(&mut holder).await;
    expire_soon(&f).await;
    let source = f.auth.source.clone();
    let token = f.auth.learner_token.clone();
    let workspace = f.workspace.clone();
    let task_id = task.task.reference.task_id;
    let pending = tokio::spawn(async move {
        source
            .get_session_content_task(&token, &workspace, task_id)
            .await
    });
    blocked_by(&f, blocker).await;
    wait_expired(&f).await;
    holder.rollback().await.unwrap();
    assert_eq!(
        pending.await.unwrap(),
        Err(SessionTaskError::Session(
            SessionCommandError::InvalidSession
        ))
    );
    f.unchanged(&task, 1).await;
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_logout_order_preserves_one_transaction_authority() {
    for logout_first in [false, true] {
        let (f, task, _, new) = pair().await;
        let other = f.auth.base.reopen().await;
        let logout_token = f.auth.learner_token.clone();
        if logout_first {
            f.auth.sql("CREATE FUNCTION pause_content_logout() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
                PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 811); RETURN OLD; END $$").await;
            f.auth.sql("CREATE TRIGGER pause_content_logout BEFORE DELETE ON sessions FOR EACH ROW EXECUTE FUNCTION pause_content_logout()").await;
            let mut holder = f.auth.base.pool.begin().await.unwrap();
            query("SELECT pg_advisory_xact_lock(hashtext($1), 811)")
                .bind(&f.auth.base.schema)
                .execute(&mut *holder)
                .await
                .unwrap();
            let blocker = backend(&mut holder).await;
            let logout = tokio::spawn(async move { other.logout(&logout_token).await });
            let logout_pid = blocked_by(&f, blocker).await;
            let pending = spawn_replace(&f, &task, manifest("After logout", vec![new.reference]));
            blocked_by(&f, logout_pid).await;
            holder.rollback().await.unwrap();
            logout.await.unwrap().unwrap();
            assert_eq!(
                pending.await.unwrap(),
                Err(SessionTaskError::Session(
                    SessionCommandError::InvalidSession
                ))
            );
            f.unchanged(&task, 1).await;
        } else {
            let mut holder = f.pause_task_event().await;
            let blocker = backend(&mut holder).await;
            let pending = spawn_replace(&f, &task, manifest("Before logout", vec![new.reference]));
            let pending_pid = blocked_by(&f, blocker).await;
            let logout = tokio::spawn(async move { other.logout(&logout_token).await });
            blocked_by(&f, pending_pid).await;
            holder.rollback().await.unwrap();
            let saved = pending.await.unwrap().unwrap();
            logout.await.unwrap().unwrap();
            f.unchanged(&saved, 2).await;
        }
        assert_eq!(f.files(), 2);
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_membership_revocation_waits_then_denies_future_bytes_and_edits() {
    let (f, task, _, new) = pair().await;
    let current = f.auth.current(f.auth.learner.binding.principal_id).await;
    let mut desired = current.policy;
    desired.membership.status = MembershipStatus::Suspended;
    let command = SessionPolicyCommand {
        command_id: id(),
        expected_revision: Some(current.revision),
        desired,
    };
    let mut holder = f.pause_task_event().await;
    let blocker = backend(&mut holder).await;
    let pending = spawn_replace(&f, &task, manifest("Admitted", vec![new.reference]));
    let pending_pid = blocked_by(&f, blocker).await;
    let other = f.auth.base.reopen().await;
    let manager = f.auth.manager_token.clone();
    let revoke =
        tokio::spawn(async move { other.apply_session_policy_command(&manager, &command).await });
    blocked_by(&f, pending_pid).await;
    holder.rollback().await.unwrap();
    let saved = pending.await.unwrap().unwrap();
    revoke.await.unwrap().unwrap();
    assert!(f
        .get(&f.auth.learner_token, saved.task.reference.task_id)
        .await
        .is_err());
    assert!(f
        .replace(&f.auth.learner_token, &saved, manifest("Denied", vec![]))
        .await
        .is_err());
    assert!(f
        .transition(&f.auth.learner_token, &saved, TaskStatus::Ready)
        .await
        .is_err());
    f.unchanged(&saved, 2).await;
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_cancellation_at_event_wait_rolls_back_and_restores_the_source_pool(
) {
    let (f, task, _, new) = pair().await;
    let mut holder = f.pause_task_event().await;
    let blocker = backend(&mut holder).await;
    let pending = spawn_replace(&f, &task, manifest("Cancelled", vec![new.reference]));
    blocked_by(&f, blocker).await;
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    holder.rollback().await.unwrap();
    f.auth.base.drain().await;
    f.unchanged(&task, 1).await;
    assert_eq!(f.files(), 2);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_post_write_blob_changes_are_detected_without_deleting_other_content(
) {
    for which in ["old", "new"] {
        let (f, task, old, new) = pair().await;
        let path = f.blob(if which == "old" {
            &old.reference
        } else {
            &new.reference
        });
        let mut holder = f.pause_task_event().await;
        let blocker = backend(&mut holder).await;
        let pending = spawn_replace(
            &f,
            &task,
            manifest("Recheck bytes", vec![new.reference.clone()]),
        );
        blocked_by(&f, blocker).await;
        let mut damaged = fs::read(&path).unwrap();
        damaged[0] ^= 1;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&path, &damaged).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        holder.rollback().await.unwrap();
        assert_eq!(
            pending.await.unwrap(),
            Err(SessionTaskError::Artifact(
                ArtifactStoreError::BlobUnavailable
            ))
        );
        f.unchanged(&task, 1).await;
        assert_eq!(
            fs::read(&path).unwrap(),
            damaged,
            "failed Task edit does not roll back or repair files"
        );
        let unaffected = if which == "old" { new } else { old };
        assert!(f
            .artifacts
            .read(&unaffected.reference, unaffected.owner)
            .await
            .unwrap()
            .is_some());
        assert_eq!(f.files(), 2);
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}
