use super::*;
use std::time::Duration;

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

async fn wait_for_backend_blocked_by(f: &Fixture, blocker_pid: i32) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let waiting: i64 = query(
                "SELECT count(*) AS n FROM pg_stat_activity
                 WHERE datname = current_database() AND $1 = ANY(pg_blocking_pids(pid))",
            )
            .bind(blocker_pid)
            .fetch_one(&f.auth.base.admin)
            .await
            .unwrap()
            .get("n");
            if waiting > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("trusted artifact writer did not reach the synthetic event lock");
}

async fn assert_counts(f: &Fixture, artifacts: i64, revisions: i64, events: i64) {
    assert_eq!(f.count("collaboration_artifacts").await, artifacts);
    assert_eq!(f.count("collaboration_artifact_revisions").await, revisions);
    assert_eq!(f.count("collaboration_artifact_outbox").await, events);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_artifacts_logout_waits_for_admitted_publication_commit() {
    let f = Fixture::ready().await;
    let blocker = f.pause().await;
    let source = f.auth.source.clone();
    let token = f.auth.learner_token.clone();
    let workspace = f.workspace.clone();
    let pending = tokio::spawn(async move {
        source
            .create_session_artifact(&token, &workspace, id(), id(), draft(b"Before logout"))
            .await
    });
    f.auth.base.wait_blocked().await;
    assert_eq!(f.files(), 1);
    let other = f.auth.base.reopen().await;
    let token = f.auth.learner_token.clone();
    let logout = tokio::spawn(async move { other.logout(&token).await });
    wait_for_source_waiters(&f, 2).await;
    assert!(!pending.is_finished());
    assert!(!logout.is_finished());
    blocker.rollback().await.unwrap();
    let revision = pending.await.unwrap().unwrap();
    logout.await.unwrap().unwrap();
    assert_eq!(
        f.artifacts
            .read(&revision.reference, revision.owner)
            .await
            .unwrap()
            .unwrap()
            .bytes,
        b"Before logout"
    );
    assert_eq!(
        f.auth
            .source
            .read_session_artifact(&f.auth.learner_token, &f.workspace, &revision.reference)
            .await,
        Err(SessionArtifactError::Session(
            SessionCommandError::InvalidSession
        ))
    );
    assert_counts(&f, 1, 1, 1).await;
    assert_eq!(f.files(), 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_artifacts_waiting_behind_logout_never_write_blobs() {
    let f = Fixture::ready().await;
    f.auth
        .sql(
            "CREATE FUNCTION pause_artifact_logout() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 752); RETURN OLD; END $$",
        )
        .await;
    f.auth
        .sql(
            "CREATE TRIGGER pause_artifact_logout BEFORE DELETE ON sessions
        FOR EACH ROW EXECUTE FUNCTION pause_artifact_logout()",
        )
        .await;
    let mut blocker = f.auth.base.pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext($1), 752)")
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
    let workspace = f.workspace.clone();
    let pending = tokio::spawn(async move {
        other
            .create_session_artifact(&token, &workspace, id(), id(), draft(b"Behind logout"))
            .await
    });
    wait_for_source_waiters(&f, 2).await;
    assert!(!pending.is_finished());
    assert!(!logout.is_finished());
    assert_eq!(f.files(), 0);
    blocker.rollback().await.unwrap();
    logout.await.unwrap().unwrap();
    assert_eq!(
        pending.await.unwrap(),
        Err(SessionArtifactError::Session(
            SessionCommandError::InvalidSession
        ))
    );
    assert_counts(&f, 0, 0, 0).await;
    assert_eq!(f.files(), 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_artifacts_expiry_after_registry_and_event_waits_never_publishes() {
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
        let workspace = f.workspace.clone();
        let pending = tokio::spawn(async move {
            source
                .create_session_artifact(&token, &workspace, id(), id(), draft(b"Expired"))
                .await
        });
        f.auth.base.wait_blocked().await;
        let retained_files = usize::from(wait_at_event);
        assert_eq!(f.files(), retained_files);
        // Crossing the database's actual expiry condition is not a guessed scheduling delay.
        wait_for_session_expiry(&f).await;
        assert!(!pending.is_finished());
        blocker.rollback().await.unwrap();
        assert_eq!(
            pending.await.unwrap(),
            Err(SessionArtifactError::Session(
                SessionCommandError::InvalidSession
            ))
        );
        f.auth.base.drain().await;
        assert_counts(&f, 0, 0, 0).await;
        // The completed immutable file is retained; an error is not permission to remove it.
        assert_eq!(f.files(), retained_files);
        f.assert_pool_restored().await;
        f.cleanup().await;
    }

    let f = Fixture::ready().await;
    let first = f
        .create(
            &f.auth.learner_token,
            b"Private bytes must not escape after expiry",
        )
        .await;
    let mut holder = f.pool.begin().await.unwrap();
    query("SELECT artifact_id FROM collaboration_artifacts WHERE artifact_id = $1 FOR UPDATE")
        .bind(first.reference.artifact_id.as_uuid())
        .fetch_one(&mut *holder)
        .await
        .unwrap();
    query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1")
        .bind(source_fixture::token_hash(&f.auth.learner_token))
        .execute(&f.auth.base.pool).await.unwrap();
    let source = f.auth.source.clone();
    let token = f.auth.learner_token.clone();
    let workspace = f.workspace.clone();
    let reference = first.reference.clone();
    let pending = tokio::spawn(async move {
        source
            .read_session_artifact(&token, &workspace, &reference)
            .await
    });
    // This head lock is after initial authentication; pending bytes still require a fresh Session.
    f.auth.base.wait_blocked().await;
    wait_for_session_expiry(&f).await;
    assert!(!pending.is_finished());
    holder.rollback().await.unwrap();
    assert_eq!(
        pending.await.unwrap(),
        Err(SessionArtifactError::Session(
            SessionCommandError::InvalidSession
        ))
    );
    f.auth.base.drain().await;
    f.assert_pool_restored().await;
    assert_counts(&f, 1, 1, 1).await;
    assert_eq!(f.files(), 1);
    let stored = f
        .artifacts
        .read(&first.reference, first.owner)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.revision, first);
    assert_eq!(stored.bytes, b"Private bytes must not escape after expiry");
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_artifacts_cancelled_revision_retains_file_and_restores_source_pool() {
    let f = Fixture::ready().await;
    let first = f
        .create(&f.auth.learner_token, b"Published old content")
        .await;
    let blocker = f.pause().await;
    let source = f.auth.source.clone();
    let token = f.auth.learner_token.clone();
    let workspace = f.workspace.clone();
    let reference = first.reference.clone();
    let pending = tokio::spawn(async move {
        source
            .revise_session_artifact(
                &token,
                &workspace,
                &reference,
                id(),
                draft(b"Unpublished edit"),
            )
            .await
    });
    f.auth.base.wait_blocked().await;
    assert_eq!(f.files(), 2);
    pending.abort();
    assert!(pending.await.is_err_and(|error| error.is_cancelled()));
    blocker.rollback().await.unwrap();
    f.auth.base.drain().await;
    f.assert_pool_restored().await;
    assert_counts(&f, 1, 1, 1).await;
    assert_eq!(f.files(), 2);
    let content = f
        .auth
        .source
        .read_session_artifact(&f.auth.learner_token, &f.workspace, &first.reference)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(content.revision, first);
    assert_eq!(content.bytes, b"Published old content");
    // The cancelled attempt must not have advanced the head or consumed the next sequence.
    let next = f
        .auth
        .source
        .revise_session_artifact(
            &f.auth.learner_token,
            &f.workspace,
            &first.reference,
            id(),
            draft(b"Later committed edit"),
        )
        .await
        .unwrap();
    assert_eq!(u64::from(next.sequence), 2);
    assert_eq!(next.parent, Some(first.reference));
    assert_counts(&f, 1, 2, 2).await;
    assert_eq!(f.files(), 3);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_artifacts_membership_suspension_waits_then_denies_read_and_revision() {
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
    let workspace = f.workspace.clone();
    let pending = tokio::spawn(async move {
        source
            .create_session_artifact(
                &token,
                &workspace,
                id(),
                id(),
                draft(b"Admitted while active"),
            )
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
    let revision = pending.await.unwrap().unwrap();
    let receipt = suspend.await.unwrap().unwrap();
    assert_eq!(
        receipt.entry.after.policy.membership.status,
        MembershipStatus::Suspended
    );
    // Authentication remains valid; loss of membership must independently reject every access.
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
        .read_session_artifact(&f.auth.learner_token, &f.workspace, &revision.reference)
        .await
        .is_err());
    assert!(f
        .auth
        .source
        .revise_session_artifact(
            &f.auth.learner_token,
            &f.workspace,
            &revision.reference,
            id(),
            draft(b"Denied edit")
        )
        .await
        .is_err());
    assert!(f
        .auth
        .source
        .create_session_artifact(
            &f.auth.learner_token,
            &f.workspace,
            id(),
            id(),
            draft(b"Denied create")
        )
        .await
        .is_err());
    assert_counts(&f, 1, 1, 1).await;
    assert_eq!(f.files(), 1);
    assert_eq!(
        f.artifacts
            .read(&revision.reference, revision.owner)
            .await
            .unwrap()
            .unwrap()
            .bytes,
        b"Admitted while active"
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_artifacts_exact_read_waits_for_trusted_revision_without_mixed_head() {
    let f = Fixture::ready().await;
    let first = f
        .create(&f.auth.learner_token, b"Exact original bytes")
        .await;
    let mut blocker = f.pause().await;
    let blocker_pid: i32 = query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut *blocker)
        .await
        .unwrap()
        .get("pid");
    let artifacts = f.artifacts.clone();
    let reference = first.reference.clone();
    let owner = first.owner;
    let writer = tokio::spawn(async move {
        artifacts
            .revise(&reference, id(), owner, draft(b"Concurrent next bytes"))
            .await
    });
    // This trusted-store connection has no source application_name: identify the actual blocker.
    wait_for_backend_blocked_by(&f, blocker_pid).await;
    assert_eq!(f.files(), 2);
    let source = f.auth.source.clone();
    let token = f.auth.learner_token.clone();
    let workspace = f.workspace.clone();
    let reference = first.reference.clone();
    let reader = tokio::spawn(async move {
        source
            .read_session_artifact(&token, &workspace, &reference)
            .await
    });
    f.auth.base.wait_blocked().await;
    assert!(!writer.is_finished());
    assert!(!reader.is_finished());
    blocker.rollback().await.unwrap();
    let next = writer.await.unwrap().unwrap();
    let content = reader.await.unwrap().unwrap().unwrap();
    assert_eq!(content.revision, first);
    assert_eq!(content.bytes, b"Exact original bytes");
    assert_eq!(next.parent, Some(first.reference));
    let latest = f
        .auth
        .source
        .read_session_artifact(&f.auth.learner_token, &f.workspace, &next.reference)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(latest.revision, next);
    assert_eq!(latest.bytes, b"Concurrent next bytes");
    assert_counts(&f, 1, 2, 2).await;
    assert_eq!(f.files(), 2);
    f.assert_pool_restored().await;
    f.cleanup().await;
}
