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
    .expect("synthetic Session did not expire before the database lock deadline");
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_inputs_reader_holds_task_lock_while_waiting_for_artifacts() {
    let f = Fixture::ready().await;
    let artifact = f
        .create_artifact(&f.auth.learner_token, b"Locked input bytes")
        .await;
    let task = f
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
    let task_id = task.reference.task_id;
    let reader = tokio::spawn(async move {
        source
            .get_session_input_task(&token, &workspace, task_id)
            .await
    });
    let reader_pid = blocked_by(&f, holder_pid).await;
    let tasks = f.tasks.clone();
    let reference = task.reference;
    let owner = task.owner;
    let revision = task.revision;
    let writer = tokio::spawn(async move {
        tasks
            .transition(reference, owner, revision, TaskStatus::Ready)
            .await
    });
    // Identify the exact reader transaction as blocker, rather than merely noticing any wait.
    blocked_by(&f, reader_pid).await;
    assert!(!reader.is_finished());
    assert!(!writer.is_finished());
    holder.rollback().await.unwrap();
    let viewed = reader.await.unwrap().unwrap().unwrap();
    let ready = writer.await.unwrap().unwrap();
    assert_eq!(viewed.task, task);
    assert_eq!(viewed.inputs.len(), 1);
    assert_eq!(viewed.inputs[0].revision, artifact);
    assert_eq!(viewed.inputs[0].bytes, b"Locked input bytes");
    assert_eq!(ready.status, TaskStatus::Ready);
    assert_eq!(u64::from(ready.revision), 2);
    assert_eq!(
        f.tasks.get(ready.reference, ready.owner).await.unwrap(),
        Some(ready)
    );
    assert_eq!(f.count_task("collaboration_task_outbox").await, 2);
    assert_eq!(f.files(), 1);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_inputs_reads_original_revision_after_trusted_artifact_writer() {
    let f = Fixture::ready().await;
    let first = f
        .create_artifact(&f.auth.learner_token, b"Exact original input")
        .await;
    let task = f
        .create(&f.auth.learner_token, vec![first.reference.clone()])
        .await;
    f.sql_artifact(
        "CREATE FUNCTION pause_input_artifact() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 762); RETURN NEW; END $$",
    )
    .await;
    f.sql_artifact(
        "CREATE TRIGGER pause_input_artifact BEFORE INSERT ON collaboration_artifact_outbox
        FOR EACH ROW EXECUTE FUNCTION pause_input_artifact()",
    )
    .await;
    let mut holder = f.artifact_pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext($1), 762)")
        .bind(&f.artifact_schema)
        .execute(&mut *holder)
        .await
        .unwrap();
    let holder_pid = backend(&mut holder).await;
    let artifacts = f.artifacts.clone();
    let parent = first.reference.clone();
    let owner = first.owner;
    let writer = tokio::spawn(async move {
        artifacts
            .revise(&parent, id(), owner, draft(b"New artifact version"))
            .await
    });
    let writer_pid = blocked_by(&f, holder_pid).await;
    let source = f.auth.source.clone();
    let token = f.auth.learner_token.clone();
    let workspace = f.workspace.clone();
    let task_id = task.reference.task_id;
    let reader = tokio::spawn(async move {
        source
            .get_session_input_task(&token, &workspace, task_id)
            .await
    });
    blocked_by(&f, writer_pid).await;
    assert!(!reader.is_finished());
    assert!(!writer.is_finished());
    holder.rollback().await.unwrap();
    let next = writer.await.unwrap().unwrap();
    let viewed = reader.await.unwrap().unwrap().unwrap();
    assert_eq!(viewed.task, task);
    assert_eq!(viewed.inputs.len(), 1);
    assert_eq!(viewed.inputs[0].revision, first);
    assert_eq!(viewed.inputs[0].bytes, b"Exact original input");
    assert_eq!(next.parent, Some(first.reference));
    assert_eq!(f.count_task("collaboration_task_outbox").await, 1);
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 2);
    assert_eq!(f.files(), 2);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_inputs_logout_order_preserves_single_transaction_authority() {
    for logout_first in [false, true] {
        let f = Fixture::ready().await;
        let artifact = f
            .create_artifact(&f.auth.learner_token, b"Input persists across logout")
            .await;
        let source = f.auth.source.clone();
        let token = f.auth.learner_token.clone();
        let workspace = f.workspace.clone();
        let reference = artifact.reference.clone();
        if logout_first {
            f.auth.sql("CREATE FUNCTION pause_input_logout() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
                PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 763); RETURN OLD; END $$").await;
            f.auth
                .sql(
                    "CREATE TRIGGER pause_input_logout BEFORE DELETE ON sessions
                FOR EACH ROW EXECUTE FUNCTION pause_input_logout()",
                )
                .await;
            let mut holder = f.auth.base.pool.begin().await.unwrap();
            query("SELECT pg_advisory_xact_lock(hashtext($1), 763)")
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
                    .create_session_input_task(
                        &token,
                        &workspace,
                        id(),
                        "Must not follow logout",
                        vec![reference],
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
            assert_eq!(f.count_task("collaboration_tasks").await, 0);
            assert_eq!(f.count_task("collaboration_task_outbox").await, 0);
        } else {
            let mut holder = f.pause_task_event().await;
            let holder_pid = backend(&mut holder).await;
            let create = tokio::spawn(async move {
                source
                    .create_session_input_task(
                        &token,
                        &workspace,
                        id(),
                        "Admitted before logout",
                        vec![reference],
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
            assert_eq!(f.count_task("collaboration_task_outbox").await, 1);
        }
        assert_eq!(f.files(), 1);
        assert_eq!(
            f.artifacts
                .read(&artifact.reference, artifact.owner)
                .await
                .unwrap()
                .unwrap()
                .bytes,
            b"Input persists across logout"
        );
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_inputs_expiry_after_input_and_event_waits_returns_nothing() {
    for operation in ["create", "get", "transition"] {
        let f = Fixture::ready().await;
        let artifact = f
            .create_artifact(&f.auth.learner_token, b"Do not expose after expiry")
            .await;
        let task = if operation == "create" {
            None
        } else {
            Some(
                f.create(&f.auth.learner_token, vec![artifact.reference.clone()])
                    .await,
            )
        };
        let mut holder = if operation == "create" {
            f.pause_task_event().await
        } else {
            let mut holder = f.artifact_pool.begin().await.unwrap();
            query(
                "SELECT artifact_id FROM collaboration_artifacts WHERE artifact_id = $1 FOR UPDATE",
            )
            .bind(artifact.reference.artifact_id.as_uuid())
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
        let reference = artifact.reference.clone();
        let requested = task.clone();
        let pending = tokio::spawn(async move {
            match operation {
                "create" => source
                    .create_session_input_task(
                        &token,
                        &workspace,
                        id(),
                        "Expired publication",
                        vec![reference],
                    )
                    .await
                    .map(|_| ()),
                "get" => source
                    .get_session_input_task(
                        &token,
                        &workspace,
                        requested.unwrap().reference.task_id,
                    )
                    .await
                    .map(|_| ()),
                _ => {
                    let task = requested.unwrap();
                    source
                        .transition_session_input_task(
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
        assert_eq!(
            f.count_task("collaboration_tasks").await,
            i64::from(task.is_some())
        );
        assert_eq!(
            f.count_task("collaboration_task_outbox").await,
            i64::from(task.is_some())
        );
        if let Some(task) = task {
            assert_eq!(
                f.tasks.get(task.reference, task.owner).await.unwrap(),
                Some(task)
            );
        }
        assert_eq!(f.files(), 1);
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_inputs_membership_revocation_waits_and_denies_future_access() {
    let f = Fixture::ready().await;
    let artifact = f
        .create_artifact(&f.auth.learner_token, b"Not shared by membership")
        .await;
    let current = f.auth.current(f.auth.learner.binding.principal_id).await;
    let mut desired = current.policy;
    desired.membership.status = MembershipStatus::Suspended;
    let request = SessionPolicyCommand {
        command_id: id(),
        expected_revision: Some(current.revision),
        desired,
    };
    let mut holder = f.pause_task_event().await;
    let holder_pid = backend(&mut holder).await;
    let source = f.auth.source.clone();
    let token = f.auth.learner_token.clone();
    let workspace = f.workspace.clone();
    let reference = artifact.reference.clone();
    let pending = tokio::spawn(async move {
        source
            .create_session_input_task(&token, &workspace, id(), "Admitted member", vec![reference])
            .await
    });
    let pending_pid = blocked_by(&f, holder_pid).await;
    let other = f.auth.base.reopen().await;
    let manager = f.auth.manager_token.clone();
    let revoke =
        tokio::spawn(async move { other.apply_session_policy_command(&manager, &request).await });
    blocked_by(&f, pending_pid).await;
    assert!(!pending.is_finished());
    assert!(!revoke.is_finished());
    holder.rollback().await.unwrap();
    let task = pending.await.unwrap().unwrap();
    revoke.await.unwrap().unwrap();
    assert!(f
        .auth
        .source
        .validate_session(&f.auth.learner_token)
        .await
        .unwrap()
        .is_some());
    assert!(f
        .auth
        .source
        .get_session_input_task(&f.auth.learner_token, &f.workspace, task.reference.task_id)
        .await
        .is_err());
    assert!(f
        .auth
        .source
        .transition_session_input_task(
            &f.auth.learner_token,
            &f.workspace,
            task.reference.task_id,
            task.revision,
            TaskStatus::Ready,
        )
        .await
        .is_err());
    assert!(f
        .auth
        .source
        .create_session_input_task(
            &f.auth.learner_token,
            &f.workspace,
            id(),
            "Suspended member",
            vec![artifact.reference.clone()],
        )
        .await
        .is_err());
    assert_eq!(
        f.tasks.get(task.reference, task.owner).await.unwrap(),
        Some(task)
    );
    assert_eq!(f.count_task("collaboration_task_outbox").await, 1);
    assert_eq!(f.files(), 1);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_inputs_cancelled_write_rolls_back_without_touching_input_files() {
    let f = Fixture::ready().await;
    let artifact = f
        .create_artifact(&f.auth.learner_token, b"Existing immutable input")
        .await;
    let mut holder = f.pause_task_event().await;
    let holder_pid = backend(&mut holder).await;
    let source = f.auth.source.clone();
    let token = f.auth.learner_token.clone();
    let workspace = f.workspace.clone();
    let reference = artifact.reference.clone();
    let pending = tokio::spawn(async move {
        source
            .create_session_input_task(
                &token,
                &workspace,
                id(),
                "Cancelled publication",
                vec![reference],
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
        b"Existing immutable input"
    );
    let task = f
        .create(&f.auth.learner_token, vec![artifact.reference])
        .await;
    assert_eq!(u64::from(task.revision), 1);
    assert_eq!(f.count_task("collaboration_task_outbox").await, 1);
    f.cleanup().await;
}
