use super::*;
use std::time::Duration;

fn workspace(f: &Fixture) -> SessionTaskWorkspace {
    SessionTaskWorkspace::new(&f.schema, scope()).unwrap()
}

async fn wait_for_source_waiters(f: &Fixture, expected: i64) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let waiting: i64 = query(
                "SELECT count(*) AS n FROM pg_stat_activity
                 WHERE application_name = $1 AND wait_event_type = 'Lock'",
            )
            .bind(&f.auth.base.schema)
            .fetch_one(&f.auth.base.admin)
            .await
            .unwrap()
            .get("n");
            if waiting >= expected {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("source operations did not reach their synthetic locks");
}

async fn wait_for_session_expiry(f: &Fixture) {
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
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("synthetic session did not expire before the lock deadline");
}

pub(super) async fn assert_source_pool_restored(f: &Fixture) {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut connections = Vec::new();
        // Hold every connection so the cancelled command's connection cannot hide in the pool.
        for _ in 0..4 {
            let mut connection = f.auth.base.pool.acquire().await.unwrap();
            let row = query(
                "SELECT current_schema()::text AS namespace,
                 current_setting('search_path') AS path",
            )
            .fetch_one(&mut *connection)
            .await
            .unwrap();
            assert_eq!(row.get::<String, _>("namespace"), f.auth.base.schema);
            assert_eq!(row.get::<String, _>("path"), f.auth.base.schema);
            connections.push(connection);
        }
    })
    .await
    .expect("source pool did not return cancelled connections");
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_tasks_logout_waits_for_admitted_command_commit() {
    let f = Fixture::ready().await;
    let blocker = f.pause().await;
    let source = f.auth.source.clone();
    let token = f.auth.learner_token.clone();
    let workspace = workspace(&f);
    let pending = tokio::spawn(async move {
        source
            .create_session_manual_task(&token, &workspace, id(), "Admitted before logout")
            .await
    });
    f.auth.base.wait_blocked().await;
    let other = f.auth.base.reopen().await;
    let token = f.auth.learner_token.clone();
    let logout = tokio::spawn(async move { other.logout(&token).await });
    wait_for_source_waiters(&f, 2).await;
    assert!(!pending.is_finished());
    assert!(!logout.is_finished());
    blocker.rollback().await.unwrap();
    let task = pending.await.unwrap().unwrap();
    logout.await.unwrap().unwrap();
    assert_eq!(
        f.tasks.get(task.reference, task.owner).await.unwrap(),
        Some(task)
    );
    assert_eq!(
        f.auth
            .source
            .create_session_manual_task(&f.auth.learner_token, &f.workspace, id(), "After logout")
            .await,
        Err(SessionTaskError::Session(
            SessionCommandError::InvalidSession
        ))
    );
    assert_eq!(f.count("collaboration_tasks").await, 1);
    assert_eq!(f.count("collaboration_task_outbox").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_tasks_waiting_behind_logout_cannot_write() {
    let f = Fixture::ready().await;
    f.auth
        .sql(
            "CREATE FUNCTION pause_task_logout() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 742); RETURN OLD; END $$",
        )
        .await;
    f.auth
        .sql(
            "CREATE TRIGGER pause_task_logout BEFORE DELETE ON sessions
        FOR EACH ROW EXECUTE FUNCTION pause_task_logout()",
        )
        .await;
    let mut blocker = f.auth.base.pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext($1), 742)")
        .bind(&f.auth.base.schema)
        .execute(&mut *blocker)
        .await
        .unwrap();
    let source = f.auth.source.clone();
    let token = f.auth.learner_token.clone();
    let logout = tokio::spawn(async move { source.logout(&token).await });
    f.auth.base.wait_blocked().await;
    let other = f.auth.base.reopen().await;
    let token = f.auth.learner_token.clone();
    let workspace = workspace(&f);
    let pending = tokio::spawn(async move {
        other
            .create_session_manual_task(&token, &workspace, id(), "Behind logout")
            .await
    });
    wait_for_source_waiters(&f, 2).await;
    assert!(!logout.is_finished());
    assert!(!pending.is_finished());
    blocker.rollback().await.unwrap();
    logout.await.unwrap().unwrap();
    assert_eq!(
        pending.await.unwrap(),
        Err(SessionTaskError::Session(
            SessionCommandError::InvalidSession
        ))
    );
    assert_eq!(f.count("collaboration_tasks").await, 0);
    assert_eq!(f.count("collaboration_task_outbox").await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_tasks_expiry_is_checked_after_registry_and_event_waits() {
    for wait_at_event in [false, true] {
        let f = Fixture::ready().await;
        let blocker = if wait_at_event {
            f.pause().await
        } else {
            let mut blocker = f.auth.base.pool.begin().await.unwrap();
            query("SELECT authority_id FROM collaboration_authorities WHERE authority_id = $1 FOR UPDATE")
                .bind(authority().as_uuid()).fetch_one(&mut *blocker).await.unwrap();
            blocker
        };
        query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1")
            .bind(source_fixture::token_hash(&f.auth.learner_token))
            .execute(&f.auth.base.pool).await.unwrap();
        let source = f.auth.source.clone();
        let token = f.auth.learner_token.clone();
        let workspace = workspace(&f);
        let pending = tokio::spawn(async move {
            source
                .create_session_manual_task(&token, &workspace, id(), "Expired while waiting")
                .await
        });
        f.auth.base.wait_blocked().await;
        // Wait for the database's expiry condition, not a guessed scheduling delay.
        wait_for_session_expiry(&f).await;
        assert!(!pending.is_finished());
        blocker.rollback().await.unwrap();
        assert_eq!(
            pending.await.unwrap(),
            Err(SessionTaskError::Session(
                SessionCommandError::InvalidSession
            ))
        );
        f.auth.base.drain().await;
        assert_eq!(f.count("collaboration_tasks").await, 0);
        assert_eq!(f.count("collaboration_task_outbox").await, 0);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_tasks_cancelled_event_rolls_back_and_restores_source_pool() {
    let f = Fixture::ready().await;
    let blocker = f.pause().await;
    let source = f.auth.source.clone();
    let token = f.auth.learner_token.clone();
    let workspace = workspace(&f);
    let pending = tokio::spawn(async move {
        source
            .create_session_manual_task(&token, &workspace, id(), "Cancelled before publication")
            .await
    });
    f.auth.base.wait_blocked().await;
    pending.abort();
    assert!(pending.await.is_err_and(|error| error.is_cancelled()));
    blocker.rollback().await.unwrap();
    f.auth.base.drain().await;
    assert_source_pool_restored(&f).await;
    assert_eq!(f.count("collaboration_tasks").await, 0);
    assert_eq!(f.count("collaboration_task_outbox").await, 0);
    let task = f.create(&f.auth.learner_token).await;
    assert_eq!(task.revision, 1.try_into().unwrap());
    assert_eq!(f.count("collaboration_tasks").await, 1);
    assert_eq!(f.count("collaboration_task_outbox").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_tasks_membership_suspension_waits_then_denies_future_commands() {
    let f = Fixture::ready().await;
    let current = f.auth.current(f.auth.learner.binding.principal_id).await;
    let mut desired = current.policy;
    desired.membership.status = MembershipStatus::Suspended;
    let request = SessionPolicyCommand {
        command_id: id(),
        expected_revision: Some(current.revision),
        desired,
    };
    let blocker = f.pause().await;
    let source = f.auth.source.clone();
    let token = f.auth.learner_token.clone();
    let workspace = workspace(&f);
    let pending = tokio::spawn(async move {
        source
            .create_session_manual_task(&token, &workspace, id(), "Admitted while active")
            .await
    });
    f.auth.base.wait_blocked().await;
    let other = f.auth.base.reopen().await;
    let token = f.auth.manager_token.clone();
    let suspend =
        tokio::spawn(async move { other.apply_session_policy_command(&token, &request).await });
    wait_for_source_waiters(&f, 2).await;
    assert!(!pending.is_finished());
    assert!(!suspend.is_finished());
    blocker.rollback().await.unwrap();
    let task = pending.await.unwrap().unwrap();
    let receipt = suspend.await.unwrap().unwrap();
    assert_eq!(
        receipt.entry.after.policy.membership.status,
        MembershipStatus::Suspended
    );
    // The Session remains current: losing membership, not logout, denies all private-task access.
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
        .create_session_manual_task(&f.auth.learner_token, &f.workspace, id(), "Suspended")
        .await
        .is_err());
    assert!(f
        .auth
        .source
        .get_session_manual_task(&f.auth.learner_token, &f.workspace, task.reference.task_id)
        .await
        .is_err());
    assert!(f
        .auth
        .source
        .transition_session_manual_task(
            &f.auth.learner_token,
            &f.workspace,
            task.reference.task_id,
            task.revision,
            TaskStatus::Ready,
        )
        .await
        .is_err());
    assert_eq!(
        f.tasks.get(task.reference, task.owner).await.unwrap(),
        Some(task)
    );
    assert_eq!(f.count("collaboration_tasks").await, 1);
    assert_eq!(f.count("collaboration_task_outbox").await, 1);
    f.cleanup().await;
}
