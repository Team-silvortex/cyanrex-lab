use super::*;
use std::time::Duration;

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
    .expect("replacement did not reach the exact synthetic blocker")
}

fn spawn_replace(
    f: &Fixture,
    task: &TaskSnapshot,
    refs: Vec<ArtifactRef>,
) -> tokio::task::JoinHandle<Result<TaskSnapshot, SessionTaskError>> {
    let source = f.auth.source.clone();
    let token = f.auth.learner_token.clone();
    let workspace = f.workspace.clone();
    let task = task.clone();
    tokio::spawn(async move {
        source
            .replace_session_input_task_inputs(
                &token,
                &workspace,
                task.reference.task_id,
                task.revision,
                refs,
            )
            .await
    })
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
    .expect("synthetic Session did not expire within the lock deadline");
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_revision_two_writers_have_one_revision_and_outbox_winner() {
    let (f, task, old, new) = pair().await;
    let mut holder = f.pause_task_event().await;
    let blocker = backend(&mut holder).await;
    let first = spawn_replace(&f, &task, vec![new.reference.clone()]);
    let first_pid = blocked_by(&f, blocker).await;
    let second = spawn_replace(
        &f,
        &task,
        vec![old.reference.clone(), new.reference.clone()],
    );
    blocked_by(&f, first_pid).await;
    assert!(!first.is_finished());
    assert!(!second.is_finished());
    holder.rollback().await.unwrap();
    let winner = first.await.unwrap().unwrap();
    assert_eq!(
        second.await.unwrap(),
        Err(SessionTaskError::Task(TaskStoreError::StaleRevision))
    );
    assert_eq!(winner.input_refs, vec![new.reference.clone()]);
    assert_eq!(u64::from(winner.revision), 2);
    unchanged(&f, &winner, 2).await;
    assert_eq!(
        event_types(&f, &winner).await,
        ["cyanrex.task.created", "cyanrex.task.inputs_replaced"]
    );
    published(&f, &old, &new).await;
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_revision_waiting_for_old_task_lock_observes_fresh_revision() {
    let (f, task, old, new) = pair().await;
    let mut holder = f.pause_task_event().await;
    let blocker = backend(&mut holder).await;
    let tasks = f.tasks.clone();
    let before = task.clone();
    let trusted = tokio::spawn(async move {
        tasks
            .transition(
                before.reference,
                before.owner,
                before.revision,
                TaskStatus::Ready,
            )
            .await
    });
    let trusted_pid = blocked_by(&f, blocker).await;
    let replacing = spawn_replace(&f, &task, vec![new.reference.clone()]);
    blocked_by(&f, trusted_pid).await;
    assert!(!replacing.is_finished());
    holder.rollback().await.unwrap();
    let winner = trusted.await.unwrap().unwrap();
    assert_eq!(winner.status, TaskStatus::Ready);
    assert_eq!(
        replacing.await.unwrap(),
        Err(SessionTaskError::Task(TaskStoreError::StaleRevision))
    );
    assert_eq!(winner.input_refs, task.input_refs);
    unchanged(&f, &winner, 2).await;
    assert_eq!(
        event_types(&f, &winner).await,
        ["cyanrex.task.created", "cyanrex.task.status_changed"]
    );
    published(&f, &old, &new).await;
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_revision_expiry_after_task_artifact_and_event_waits_never_commits() {
    for wait in ["task", "artifact", "event"] {
        let (f, task, old, new) = pair().await;
        let mut holder = match wait {
            "task" => {
                let mut tx = f.task_pool.begin().await.unwrap();
                query("SELECT task_id FROM collaboration_tasks WHERE task_id = $1 FOR UPDATE")
                    .bind(task.reference.task_id.as_uuid())
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
            _ => f.pause_task_event().await,
        };
        let blocker = backend(&mut holder).await;
        query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1")
            .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
        let pending = spawn_replace(&f, &task, vec![new.reference.clone()]);
        blocked_by(&f, blocker).await;
        wait_expired(&f).await;
        assert!(!pending.is_finished());
        holder.rollback().await.unwrap();
        assert_eq!(
            pending.await.unwrap(),
            Err(SessionTaskError::Session(
                SessionCommandError::InvalidSession
            )),
            "{wait}"
        );
        unchanged(&f, &task, 1).await;
        published(&f, &old, &new).await;
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_revision_cancellation_at_event_wait_rolls_back_and_restores_pool() {
    let (f, task, old, new) = pair().await;
    let mut holder = f.pause_task_event().await;
    let blocker = backend(&mut holder).await;
    let pending = spawn_replace(&f, &task, vec![new.reference.clone()]);
    blocked_by(&f, blocker).await;
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    holder.rollback().await.unwrap();
    f.auth.base.drain().await;
    unchanged(&f, &task, 1).await;
    published(&f, &old, &new).await;
    f.assert_pool_restored().await;
    f.cleanup().await;
}
