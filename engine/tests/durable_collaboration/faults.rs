use super::*;

async fn trigger(f: &Fixture, table: &str, timing: &str, body: &str) {
    f.sql(&format!("CREATE FUNCTION session_command_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN {body} END $$")).await;
    f.sql(&format!("CREATE TRIGGER session_command_fault {timing} ON {table} FOR EACH ROW EXECUTE FUNCTION session_command_fault()")).await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_commands_failed_audit_rolls_back_binding_and_policy() {
    for table in ["collaboration_identity_audit", "collaboration_policy_audit"] {
        let f = Fixture::ready().await;
        let target = if table == "collaboration_policy_audit" {
            Some(f.bind_target().await)
        } else {
            None
        };
        trigger(&f, table, "BEFORE INSERT", "RETURN NULL;").await;
        if let Some(target) = target {
            assert!(f
                .source
                .apply_session_policy_command(
                    &f.manager_token,
                    &policy_command(target.binding.principal_id, true)
                )
                .await
                .is_err());
            assert!(f
                .store
                .legacy_access(scope(), target.binding.principal_id)
                .await
                .unwrap()
                .is_none());
        } else {
            assert!(f
                .source
                .bind_session_account(&f.manager_token, &f.bind_command())
                .await
                .is_err());
            assert_eq!(f.count("collaboration_principals").await, 2);
        }
        assert_eq!(f.count(table).await, 2);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_commands_deferred_commit_failure_never_publishes_receipts() {
    for table in ["collaboration_identity_audit", "collaboration_policy_audit"] {
        let f = Fixture::ready().await;
        let target = if table == "collaboration_policy_audit" {
            Some(f.bind_target().await)
        } else {
            None
        };
        f.sql("CREATE FUNCTION command_commit_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic commit failure'; END $$").await;
        f.sql(&format!("CREATE CONSTRAINT TRIGGER command_commit_fault AFTER INSERT ON {table} DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION command_commit_fault()")).await;
        if let Some(target) = target {
            assert!(f
                .source
                .apply_session_policy_command(
                    &f.manager_token,
                    &policy_command(target.binding.principal_id, true)
                )
                .await
                .is_err());
            assert!(f
                .store
                .legacy_access(scope(), target.binding.principal_id)
                .await
                .unwrap()
                .is_none());
        } else {
            assert!(f
                .source
                .bind_session_account(&f.manager_token, &f.bind_command())
                .await
                .is_err());
            assert_eq!(f.count("collaboration_principals").await, 2);
        }
        assert_eq!(f.count(table).await, 2);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_commands_cancelled_write_rolls_back_policy_and_audit() {
    let f = Fixture::ready().await;
    let target = f.bind_target().await;
    let mut blocker = f.base.pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 70601)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    trigger(
        &f,
        "collaboration_policy_audit",
        "BEFORE INSERT",
        "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70601); RETURN NEW;",
    )
    .await;
    let source = f.source.clone();
    let token = f.manager_token.clone();
    let cmd = policy_command(target.binding.principal_id, true);
    let pending =
        tokio::spawn(async move { source.apply_session_policy_command(&token, &cmd).await });
    f.base.wait_blocked().await;
    pending.abort();
    assert!(pending.await.is_err_and(|e| e.is_cancelled()));
    blocker.rollback().await.unwrap();
    f.base.drain().await;
    assert!(f
        .store
        .legacy_access(scope(), target.binding.principal_id)
        .await
        .unwrap()
        .is_none());
    assert_eq!(f.count("collaboration_policy_audit").await, 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_commands_logout_waits_for_admitted_command_commit() {
    let f = Fixture::ready().await;
    let target = f.bind_target().await;
    let mut blocker = f.base.pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 70602)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    trigger(
        &f,
        "collaboration_policy_audit",
        "BEFORE INSERT",
        "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70602); RETURN NEW;",
    )
    .await;
    let source = f.source.clone();
    let token = f.manager_token.clone();
    let cmd = policy_command(target.binding.principal_id, true);
    let pending =
        tokio::spawn(async move { source.apply_session_policy_command(&token, &cmd).await });
    f.base.wait_blocked().await;
    let other = f.base.reopen().await;
    let token = f.manager_token.clone();
    let mut logout = tokio::spawn(async move { other.logout(&token).await });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut logout)
            .await
            .is_err()
    );
    blocker.rollback().await.unwrap();
    pending.await.unwrap().unwrap();
    logout.await.unwrap().unwrap();
    assert!(
        f.current(target.binding.principal_id)
            .await
            .policy
            .deployment_granted
    );
    assert_eq!(
        f.source
            .bind_session_account(&f.manager_token, &f.bind_command())
            .await,
        Err(SessionCommandError::InvalidSession)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_commands_waiting_behind_logout_cannot_write() {
    let f = Fixture::ready().await;
    let mut blocker = f.base.pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 70603)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    trigger(
        &f,
        "sessions",
        "BEFORE DELETE",
        "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70603); RETURN OLD;",
    )
    .await;
    let source = f.source.clone();
    let token = f.manager_token.clone();
    let pending = tokio::spawn(async move { source.logout(&token).await });
    f.base.wait_blocked().await;
    let other = f.base.reopen().await;
    let token = f.manager_token.clone();
    let cmd = f.bind_command();
    let mut command = tokio::spawn(async move { other.bind_session_account(&token, &cmd).await });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut command)
            .await
            .is_err()
    );
    blocker.rollback().await.unwrap();
    pending.await.unwrap().unwrap();
    assert_eq!(
        command.await.unwrap(),
        Err(SessionCommandError::InvalidSession)
    );
    assert_eq!(f.count("collaboration_identity_audit").await, 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_commands_expiry_is_checked_after_lock_wait_and_before_commit() {
    for wait_at_audit in [false, true] {
        let f = Fixture::ready().await;
        let mut blocker = f.base.pool.begin().await.unwrap();
        if wait_at_audit {
            query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 70604)")
                .execute(&mut *blocker)
                .await
                .unwrap();
            trigger(
                &f,
                "collaboration_identity_audit",
                "BEFORE INSERT",
                "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70604); RETURN NEW;",
            )
            .await;
        } else {
            query("SELECT authority_id FROM collaboration_authorities WHERE authority_id = $1 FOR UPDATE").bind(authority().as_uuid()).fetch_one(&mut *blocker).await.unwrap();
        }
        query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '500 milliseconds' WHERE token = $1")
            .bind(source_fixture::token_hash(&f.manager_token)).execute(&f.base.pool).await.unwrap();
        let source = f.source.clone();
        let token = f.manager_token.clone();
        let cmd = f.bind_command();
        let pending = tokio::spawn(async move { source.bind_session_account(&token, &cmd).await });
        f.base.wait_blocked().await;
        query("SELECT pg_sleep(0.55)")
            .execute(&f.base.admin)
            .await
            .unwrap();
        blocker.rollback().await.unwrap();
        assert_eq!(
            pending.await.unwrap(),
            Err(SessionCommandError::InvalidSession)
        );
        assert_eq!(f.count("collaboration_identity_audit").await, 2);
        assert_eq!(f.count("collaboration_principals").await, 2);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_commands_missing_audit_never_uses_cached_authority() {
    let f = Fixture::ready().await;
    let cmd = f.bind_command();
    f.source
        .bind_session_account(&f.manager_token, &cmd)
        .await
        .unwrap();
    f.sql("ALTER TABLE collaboration_policy_audit RENAME TO missing_policy_audit")
        .await;
    assert!(f
        .source
        .bind_session_account(&f.manager_token, &cmd)
        .await
        .is_err());
    assert!(f
        .source
        .validate_session(&f.manager_token)
        .await
        .unwrap()
        .is_some());
    f.sql("ALTER TABLE missing_policy_audit RENAME TO collaboration_policy_audit")
        .await;
    assert!(
        f.source
            .bind_session_account(&f.manager_token, &cmd)
            .await
            .unwrap()
            .replayed
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_commands_post_write_rechecks_actor_session_and_target_source() {
    for table in ["collaboration_identity_audit", "collaboration_policy_audit"] {
        for body in [
            "DELETE FROM sessions; RETURN NEW;",
            "UPDATE sessions SET expires_at = clock_timestamp() - INTERVAL '1 second'; RETURN NEW;",
            "DELETE FROM users WHERE username = 'source-target'; RETURN NEW;",
        ] {
            let f = Fixture::ready().await;
            let target = if table == "collaboration_policy_audit" {
                Some(f.bind_target().await)
            } else {
                None
            };
            let principals = f.count("collaboration_principals").await;
            trigger(&f, table, "BEFORE INSERT", body).await;
            if let Some(target) = target {
                assert!(f
                    .source
                    .apply_session_policy_command(
                        &f.manager_token,
                        &policy_command(target.binding.principal_id, true)
                    )
                    .await
                    .is_err());
                assert!(f
                    .store
                    .legacy_access(scope(), target.binding.principal_id)
                    .await
                    .unwrap()
                    .is_none());
            } else {
                assert!(f
                    .source
                    .bind_session_account(&f.manager_token, &f.bind_command())
                    .await
                    .is_err());
            }
            assert_eq!(f.count(table).await, 2);
            assert_eq!(f.count("collaboration_principals").await, principals);
            assert_eq!(f.base.count("sessions").await, 3);
            assert_eq!(f.base.count("users").await, 3);
            assert!(f
                .source
                .validate_session(&f.manager_token)
                .await
                .unwrap()
                .is_some());
            f.cleanup().await;
        }
    }
}
