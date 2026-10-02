use super::*;

pub async fn trigger(f: &Fixture, table: &str, timing: &str, body: &str) {
    f.sql(&format!("CREATE FUNCTION deletion_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN {body} END $$")).await;
    f.sql(&format!("CREATE TRIGGER deletion_fault {timing} ON {table} FOR EACH ROW EXECUTE FUNCTION deletion_fault()")).await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_deletion_suppressed_writes_roll_back_source_identity_and_audit() {
    for (table, timing) in [
        ("sessions", "BEFORE DELETE"),
        ("users", "BEFORE DELETE"),
        ("collaboration_legacy_identities", "BEFORE UPDATE"),
        ("collaboration_principals", "BEFORE UPDATE"),
        ("collaboration_identity_audit", "BEFORE INSERT"),
    ] {
        let f = Fixture::ready().await;
        trigger(&f, table, timing, "RETURN NULL;").await;
        assert!(
            f.source
                .delete_session_account(&f.manager_token, &deletion(&f.learner))
                .await
                .is_err(),
            "{table}"
        );
        assert_unchanged(&f, 2).await;
        assert_eq!(f.count("collaboration_policy_audit").await, 2);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_deletion_post_write_checks_reject_trigger_side_effects() {
    for (table, timing, body) in [
        ("collaboration_identity_audit", "BEFORE INSERT", "DELETE FROM sessions WHERE username = 'source-manager'; RETURN NEW;"),
        ("collaboration_identity_audit", "BEFORE INSERT", "UPDATE sessions SET expires_at = clock_timestamp() - INTERVAL '1 second'; RETURN NEW;"),
        ("collaboration_identity_audit", "BEFORE INSERT", "DELETE FROM users WHERE username = 'source-learner'; RETURN NEW;"),
        ("users", "AFTER DELETE", "UPDATE collaboration_principals SET status = 'active' WHERE principal_id IN (SELECT principal_id FROM collaboration_legacy_identities WHERE username = OLD.username); RETURN OLD;"),
        ("users", "AFTER DELETE", "UPDATE collaboration_deployment_grants SET status = 'revoked'; RETURN OLD;"),
    ] {
        let f = Fixture::ready().await;
        trigger(&f, table, timing, body).await;
        assert!(f.source.delete_session_account(&f.manager_token, &deletion(&f.learner)).await.is_err(), "{body}");
        assert_unchanged(&f, 2).await;
        assert!(f.current(f.manager.binding.principal_id).await.policy.deployment_granted);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_deletion_deferred_commit_failure_does_not_publish_success() {
    for (table, operation) in [
        ("users", "DELETE"),
        ("collaboration_identity_audit", "INSERT"),
    ] {
        let f = Fixture::ready().await;
        f.sql("CREATE FUNCTION deletion_commit_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic commit failure'; END $$").await;
        f.sql(&format!("CREATE CONSTRAINT TRIGGER deletion_commit_fault AFTER {operation} ON {table} DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION deletion_commit_fault()")).await;
        assert!(f
            .source
            .delete_session_account(&f.manager_token, &deletion(&f.learner))
            .await
            .is_err());
        assert_unchanged(&f, 2).await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_deletion_cancellation_before_commit_rolls_back_every_component() {
    let f = Fixture::ready().await;
    let mut blocker = f.base.pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 70801)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    trigger(
        &f,
        "users",
        "AFTER DELETE",
        "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70801); RETURN OLD;",
    )
    .await;
    let source = f.source.clone();
    let token = f.manager_token.clone();
    let cmd = deletion(&f.learner);
    let pending = tokio::spawn(async move { source.delete_session_account(&token, &cmd).await });
    f.base.wait_blocked().await;
    pending.abort();
    assert!(pending.await.is_err_and(|e| e.is_cancelled()));
    blocker.rollback().await.unwrap();
    f.base.drain().await;
    assert_unchanged(&f, 2).await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_deletion_expiry_after_lock_or_delete_wait_rolls_back() {
    for after_delete in [false, true] {
        let f = Fixture::ready().await;
        let mut blocker = f.base.pool.begin().await.unwrap();
        if after_delete {
            query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 70802)")
                .execute(&mut *blocker)
                .await
                .unwrap();
            trigger(
                &f,
                "users",
                "AFTER DELETE",
                "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70802); RETURN OLD;",
            )
            .await;
        } else {
            query("SELECT authority_id FROM collaboration_authorities FOR UPDATE")
                .fetch_one(&mut *blocker)
                .await
                .unwrap();
        }
        query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '500 milliseconds' WHERE token = $1")
            .bind(source_fixture::token_hash(&f.manager_token)).execute(&f.base.pool).await.unwrap();
        let source = f.source.clone();
        let token = f.manager_token.clone();
        let cmd = deletion(&f.learner);
        let pending =
            tokio::spawn(async move { source.delete_session_account(&token, &cmd).await });
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
        assert_unchanged(&f, 2).await;
        f.cleanup().await;
    }
}
