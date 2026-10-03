use super::*;
use std::time::Duration;

async fn backend(tx: &mut sqlx_core::transaction::Transaction<'_, sqlx_postgres::Postgres>) -> i32 {
    query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
        .get("pid")
}

async fn blocked_by(f: &Fixture, blocker: i32) -> i32 {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let row = query(
                "SELECT pid FROM pg_stat_activity WHERE datname = current_database()
                AND $1 = ANY(pg_blocking_pids(pid)) ORDER BY pid LIMIT 1",
            )
            .bind(blocker)
            .fetch_optional(&f.auth.base.admin)
            .await
            .unwrap();
            if let Some(row) = row {
                return row.get("pid");
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("expected backend did not reach its exact synthetic blocker")
}

async fn wait_expired(f: &Fixture) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let expired: bool = query(&format!("SELECT clock_timestamp() >= expires_at AS expired FROM {}.sessions WHERE token = $1", f.auth.base.schema))
                .bind(source_fixture::token_hash(&f.auth.learner_token)).fetch_one(&f.auth.base.admin).await.unwrap().get("expired");
            if expired { return; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("synthetic Session did not expire before the database lock deadline");
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_catalog_tasks_reader_holds_task_lock_while_waiting_for_artifacts() {
    let f = Fixture::ready().await;
    let artifact = f
        .create_artifact(&f.auth.learner_token, b"Original catalogue input")
        .await;
    let first = f
        .create(&f.auth.learner_token, vec![artifact.reference.clone()])
        .await;
    let mut holder = f.artifact_pool.begin().await.unwrap();
    query("SELECT artifact_id FROM collaboration_artifacts WHERE artifact_id = $1 FOR UPDATE")
        .bind(artifact.reference.artifact_id.as_uuid())
        .fetch_one(&mut *holder)
        .await
        .unwrap();
    let holder_pid = backend(&mut holder).await;
    let source = f.auth.source.clone();
    let token = f.auth.learner_token.clone();
    let workspace = f.workspace.clone();
    let task_id = first.reference.task_id;
    let reader = tokio::spawn(async move {
        source
            .get_session_catalog_task(&token, &workspace, task_id)
            .await
    });
    let reader_pid = blocked_by(&f, holder_pid).await;
    let tasks = f.tasks.clone();
    let task_ref = first.reference;
    let owner = first.owner;
    let revision = first.revision;
    let writer = tokio::spawn(async move {
        tasks
            .transition(task_ref, owner, revision, TaskStatus::Ready)
            .await
    });
    blocked_by(&f, reader_pid).await;
    assert!(!reader.is_finished());
    assert!(!writer.is_finished());
    holder.rollback().await.unwrap();
    let result = reader.await.unwrap().unwrap().unwrap();
    let next = writer.await.unwrap().unwrap();
    assert_eq!(result.task, first);
    assert_eq!(result.inputs.len(), 1);
    assert_eq!(result.inputs[0].revision, artifact);
    assert_eq!(result.inputs[0].bytes, b"Original catalogue input");
    assert_eq!(next.definition, first.definition);
    assert_eq!(next.input_refs, first.input_refs);
    assert_eq!(next.status, TaskStatus::Ready);
    assert_eq!(u64::from(next.revision), 2);
    assert_eq!(
        f.tasks.get(next.reference, next.owner).await.unwrap(),
        Some(next)
    );
    assert_eq!(f.count_task("collaboration_task_outbox").await, 2);
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 1);
    assert_eq!(f.files(), 1);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_catalog_tasks_expiry_after_metadata_and_event_waits_returns_nothing() {
    for operation in ["create", "get", "transition"] {
        let f = Fixture::ready().await;
        let original = if operation == "create" {
            None
        } else {
            Some(f.create(&f.auth.learner_token, vec![]).await)
        };
        let mut holder = if operation == "create" {
            f.pause_task_event().await
        } else {
            // There are no Artifact refs. An empty iteration must not skip the configured
            // namespace's metadata lock, validation or the final post-wait Session check.
            let mut holder = f.artifact_pool.begin().await.unwrap();
            query("SELECT singleton FROM collaboration_artifact_schema WHERE singleton FOR UPDATE")
                .fetch_one(&mut *holder)
                .await
                .unwrap();
            holder
        };
        let holder_pid = backend(&mut holder).await;
        query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1")
            .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
        let source = f.auth.source.clone();
        let token = f.auth.learner_token.clone();
        let workspace = f.workspace.clone();
        let definition = reference();
        let requested = original.clone();
        let pending = tokio::spawn(async move {
            match operation {
                "create" => source
                    .create_session_catalog_task(
                        &token,
                        &workspace,
                        id(),
                        &definition,
                        "Expired empty-input task",
                        vec![],
                    )
                    .await
                    .map(|_| ()),
                "get" => source
                    .get_session_catalog_task(
                        &token,
                        &workspace,
                        requested.unwrap().reference.task_id,
                    )
                    .await
                    .map(|_| ()),
                _ => {
                    let task = requested.unwrap();
                    source
                        .transition_session_catalog_task(
                            &token,
                            &workspace,
                            task.reference.task_id,
                            task.revision,
                            TaskStatus::Ready,
                        )
                        .await
                        .map(|_| ())
                }
            }
        });
        blocked_by(&f, holder_pid).await;
        wait_expired(&f).await;
        assert!(!pending.is_finished());
        holder.rollback().await.unwrap();
        assert_eq!(
            pending.await.unwrap(),
            Err(SessionTaskError::Session(
                SessionCommandError::InvalidSession
            ))
        );
        f.auth.base.drain().await;
        for table in ["collaboration_tasks", "collaboration_task_outbox"] {
            assert_eq!(f.count_task(table).await, i64::from(original.is_some()));
        }
        if let Some(task) = original {
            assert_eq!(
                f.tasks.get(task.reference, task.owner).await.unwrap(),
                Some(task)
            );
        }
        assert_eq!(f.count_artifact("collaboration_artifacts").await, 0);
        assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 0);
        assert_eq!(f.files(), 0);
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_catalog_tasks_logout_order_preserves_single_transaction_authority() {
    for logout_first in [false, true] {
        let f = Fixture::ready().await;
        let source = f.auth.source.clone();
        let token = f.auth.learner_token.clone();
        let workspace = f.workspace.clone();
        let definition = reference();
        if logout_first {
            f.auth.sql("CREATE FUNCTION pause_catalog_logout() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
                PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 782); RETURN OLD; END $$").await;
            f.auth.sql("CREATE TRIGGER pause_catalog_logout BEFORE DELETE ON sessions FOR EACH ROW EXECUTE FUNCTION pause_catalog_logout()").await;
            let mut holder = f.auth.base.pool.begin().await.unwrap();
            query("SELECT pg_advisory_xact_lock(hashtext($1), 782)")
                .bind(&f.auth.base.schema)
                .execute(&mut *holder)
                .await
                .unwrap();
            let holder_pid = backend(&mut holder).await;
            let other = f.auth.base.reopen().await;
            let logout_token = token.clone();
            let logout = tokio::spawn(async move { other.logout(&logout_token).await });
            let logout_pid = blocked_by(&f, holder_pid).await;
            let create = tokio::spawn(async move {
                source
                    .create_session_catalog_task(
                        &token,
                        &workspace,
                        id(),
                        &definition,
                        "Cannot follow logout",
                        vec![],
                    )
                    .await
            });
            blocked_by(&f, logout_pid).await;
            assert!(!logout.is_finished());
            assert!(!create.is_finished());
            holder.rollback().await.unwrap();
            logout.await.unwrap().unwrap();
            assert_eq!(
                create.await.unwrap(),
                Err(SessionTaskError::Session(
                    SessionCommandError::InvalidSession
                ))
            );
        } else {
            let mut holder = f.pause_task_event().await;
            let holder_pid = backend(&mut holder).await;
            let create = tokio::spawn(async move {
                source
                    .create_session_catalog_task(
                        &token,
                        &workspace,
                        id(),
                        &definition,
                        "Admitted before logout",
                        vec![],
                    )
                    .await
            });
            let create_pid = blocked_by(&f, holder_pid).await;
            let other = f.auth.base.reopen().await;
            let logout_token = f.auth.learner_token.clone();
            let logout = tokio::spawn(async move { other.logout(&logout_token).await });
            blocked_by(&f, create_pid).await;
            assert!(!create.is_finished());
            assert!(!logout.is_finished());
            holder.rollback().await.unwrap();
            let task = create.await.unwrap().unwrap();
            logout.await.unwrap().unwrap();
            assert_eq!(
                f.tasks.get(task.reference, task.owner).await.unwrap(),
                Some(task)
            );
        }
        for table in ["collaboration_tasks", "collaboration_task_outbox"] {
            assert_eq!(f.count_task(table).await, i64::from(!logout_first));
        }
        assert_eq!(f.count_artifact("collaboration_artifacts").await, 0);
        assert_eq!(f.files(), 0);
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_catalog_tasks_cancelled_write_rolls_back_without_touching_inputs() {
    let f = Fixture::ready().await;
    let artifact = f
        .create_artifact(&f.auth.learner_token, b"Immutable catalogue input")
        .await;
    let mut holder = f.pause_task_event().await;
    let holder_pid = backend(&mut holder).await;
    let source = f.auth.source.clone();
    let token = f.auth.learner_token.clone();
    let workspace = f.workspace.clone();
    let definition_ref = reference();
    let input = artifact.reference.clone();
    let pending = tokio::spawn(async move {
        source
            .create_session_catalog_task(
                &token,
                &workspace,
                id(),
                &definition_ref,
                "Cancelled publication",
                vec![input],
            )
            .await
    });
    blocked_by(&f, holder_pid).await;
    pending.abort();
    assert!(pending.await.is_err_and(|error| error.is_cancelled()));
    holder.rollback().await.unwrap();
    f.auth.base.drain().await;
    f.assert_pool_restored().await;
    assert_eq!(f.count_task("collaboration_tasks").await, 0);
    assert_eq!(f.count_task("collaboration_task_outbox").await, 0);
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 1);
    assert_eq!(f.files(), 1);
    assert_eq!(
        f.artifacts
            .read(&artifact.reference, artifact.owner)
            .await
            .unwrap()
            .unwrap()
            .bytes,
        b"Immutable catalogue input"
    );
    let task = f
        .create(&f.auth.learner_token, vec![artifact.reference])
        .await;
    assert_eq!(u64::from(task.revision), 1);
    assert_eq!(task.definition, Some(definition(1)));
    assert_eq!(f.count_task("collaboration_task_outbox").await, 1);
    f.cleanup().await;
}
