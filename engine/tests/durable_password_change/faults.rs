use super::*;

pub async fn trigger(f: &Fixture, table: &str, timing: &str, body: &str) {
    f.sql("CREATE SEQUENCE password_fault_calls").await;
    f.sql(&format!("CREATE FUNCTION password_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM nextval('password_fault_calls'); {body} END $$")).await;
    f.sql(&format!("CREATE TRIGGER password_fault {timing} ON {table} FOR EACH ROW EXECUTE FUNCTION password_fault()")).await;
}

async fn assert_trigger_fired(f: &Fixture) {
    let called: bool = query("SELECT is_called FROM password_fault_calls")
        .fetch_one(&f.pool)
        .await
        .unwrap()
        .get("is_called");
    assert!(
        called,
        "reauthentication must reach the intended write fault, not fail on replay first"
    );
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_change_suppressed_writes_roll_back_both_components() {
    for (table, timing) in [("users", "BEFORE UPDATE"), ("sessions", "BEFORE DELETE")] {
        let f = Fixture::ready().await;
        let (account, login) = f.seed_account_session().await;
        let before = state(&f).await;
        trigger(&f, table, timing, "RETURN NULL;").await;
        assert_eq!(
            f.source
                .change_session_password(
                    &login.token,
                    PASSWORD,
                    NEW_PASSWORD,
                    &otp(&account.bootstrap.secret)
                )
                .await,
            Err(DurableAuthError::StorageUnavailable)
        );
        assert_trigger_fired(&f).await;
        assert_eq!(state(&f).await, before);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_change_post_write_checks_reject_trigger_side_effects() {
    for (table, timing, body) in [
        ("users", "BEFORE UPDATE", "NEW.password_hash = OLD.password_hash; RETURN NEW;"),
        ("users", "BEFORE UPDATE", "NEW.totp_secret = 'JBSWY3DPEHPK3PXP'; RETURN NEW;"),
        ("users", "AFTER UPDATE", "DELETE FROM sessions; RETURN NEW;"),
        ("users", "AFTER UPDATE", "UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '1 day'; RETURN NEW;"),
        ("sessions", "AFTER DELETE", "INSERT INTO sessions (token, username, account_id, expires_at) VALUES (OLD.token, OLD.username, OLD.account_id, OLD.expires_at); RETURN OLD;"),
        ("sessions", "AFTER DELETE", "UPDATE users SET password_hash = '$argon2-corrupt'; RETURN OLD;"),
        ("sessions", "AFTER DELETE", "DELETE FROM users; RETURN OLD;"),
    ] {
        let f = Fixture::ready().await;
        let (account, login) = f.seed_account_session().await;
        let before = state(&f).await;
        trigger(&f, table, timing, body).await;
        assert!(f.source.change_session_password(&login.token, PASSWORD, NEW_PASSWORD, &otp(&account.bootstrap.secret)).await.is_err(), "{body}");
        assert_trigger_fired(&f).await;
        assert_eq!(state(&f).await, before);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_change_deferred_commit_failure_never_reports_success() {
    for (table, operation) in [("users", "UPDATE"), ("sessions", "DELETE")] {
        let f = Fixture::ready().await;
        let (account, login) = f.seed_account_session().await;
        let before = state(&f).await;
        f.sql("CREATE SEQUENCE password_fault_calls").await;
        f.sql("CREATE FUNCTION password_commit_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM nextval('password_fault_calls'); RAISE EXCEPTION 'synthetic commit failure'; END $$").await;
        f.sql(&format!("CREATE CONSTRAINT TRIGGER password_commit_fault AFTER {operation} ON {table} DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION password_commit_fault()")).await;
        assert_eq!(
            f.source
                .change_session_password(
                    &login.token,
                    PASSWORD,
                    NEW_PASSWORD,
                    &otp(&account.bootstrap.secret)
                )
                .await,
            Err(DurableAuthError::StorageUnavailable)
        );
        assert_trigger_fired(&f).await;
        assert_eq!(state(&f).await, before);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_change_cancellation_before_commit_rolls_back() {
    let f = Fixture::ready().await;
    let (account, login) = f.seed_account_session().await;
    let before = state(&f).await;
    let mut blocker = f.pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 70901)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    trigger(
        &f,
        "sessions",
        "AFTER DELETE",
        "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70901); RETURN OLD;",
    )
    .await;
    let source = f.source.clone();
    let code = otp(&account.bootstrap.secret);
    let pending = tokio::spawn(async move {
        source
            .change_session_password(&login.token, PASSWORD, NEW_PASSWORD, &code)
            .await
    });
    f.wait_blocked().await;
    pending.abort();
    assert!(pending.await.is_err_and(|error| error.is_cancelled()));
    blocker.rollback().await.unwrap();
    f.drain().await;
    assert_trigger_fired(&f).await;
    assert_eq!(state(&f).await, before);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_change_checks_expiry_after_writer_and_delete_waits() {
    for during_delete in [false, true] {
        let f = Fixture::ready().await;
        let (account, login) = f.seed_account_session().await;
        f.sql("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '1500 milliseconds'")
            .await;
        let before = state(&f).await;
        let mut blocker = f.pool.begin().await.unwrap();
        if during_delete {
            query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 70902)")
                .execute(&mut *blocker)
                .await
                .unwrap();
            trigger(
                &f,
                "sessions",
                "AFTER DELETE",
                "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70902); RETURN OLD;",
            )
            .await;
        } else {
            query("SELECT version FROM collaboration_auth_source_schema FOR SHARE")
                .fetch_one(&mut *blocker)
                .await
                .unwrap();
        }
        let source = f.source.clone();
        let code = otp(&account.bootstrap.secret);
        let pending = tokio::spawn(async move {
            source
                .change_session_password(&login.token, PASSWORD, NEW_PASSWORD, &code)
                .await
        });
        f.wait_blocked().await;
        // Sleep only until the test row's exact expiry; it must remain below the 2-second lock limit.
        let wait: f64 = query("SELECT GREATEST(0.0, EXTRACT(EPOCH FROM (max(expires_at) - clock_timestamp())))::float8 + 0.05 AS wait FROM sessions")
            .fetch_one(&f.pool).await.unwrap().get("wait");
        tokio::time::sleep(std::time::Duration::from_secs_f64(wait)).await;
        blocker.rollback().await.unwrap();
        assert_eq!(
            pending.await.unwrap(),
            Err(DurableAuthError::InvalidCredentials)
        );
        if during_delete {
            assert_trigger_fired(&f).await;
        }
        assert_eq!(state(&f).await, before);
        f.cleanup().await;
    }
}
