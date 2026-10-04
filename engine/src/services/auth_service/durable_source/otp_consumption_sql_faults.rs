use super::*;
use std::sync::atomic::Ordering;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_consumption_suppressed_writes_roll_back_and_leave_the_code_retryable() {
    for (rotate, table, timing) in [
        (false, "users", "BEFORE UPDATE"),
        (false, "sessions", "BEFORE INSERT"),
        (true, "users", "BEFORE UPDATE"),
        (true, "sessions", "BEFORE DELETE"),
    ] {
        let f = Fixture::ready().await;
        let token = if rotate {
            Some(setup_rotation(&f).await)
        } else {
            None
        };
        let code = f.code();
        let before = f.snapshot().await;
        fault(&f, table, timing, "RETURN NULL;").await;
        let result = command(&f.source, token.as_deref(), &code).await;
        let reached = calls(&f).await;
        let after = f.snapshot().await;
        sql(&f, &format!("DROP TRIGGER consumption_fault ON {table}")).await;
        let retry = command(&f.source, token.as_deref(), &code).await;
        f.cleanup().await;
        assert_eq!(reached, 1, "rotate={rotate} {table} {timing}");
        assert_eq!(result, Err(DurableAuthError::StorageUnavailable));
        assert_eq!(
            after, before,
            "watermark and command effects must roll back together"
        );
        assert_eq!(
            retry,
            Ok(()),
            "only after observed rollback and removal of the synthetic fault"
        );
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_consumption_rechecks_counter_after_update_and_session_trigger_tampering() {
    for rotate in [false, true] {
        for (table, timing, body) in [
            (
                "users",
                "BEFORE UPDATE",
                "NEW.otp_last_counter = OLD.otp_last_counter; RETURN NEW;",
            ),
            (
                "users",
                "BEFORE UPDATE",
                "NEW.otp_last_counter = NEW.otp_last_counter + 1; RETURN NEW;",
            ),
            if rotate {
                ("sessions", "AFTER DELETE", "UPDATE users SET otp_last_counter = otp_last_counter + 1 WHERE username = OLD.username; RETURN OLD;")
            } else {
                ("sessions", "AFTER INSERT", "UPDATE users SET otp_last_counter = otp_last_counter + 1 WHERE username = NEW.username; RETURN NEW;")
            },
        ] {
            let f = Fixture::ready().await;
            let token = if rotate {
                Some(setup_rotation(&f).await)
            } else {
                None
            };
            let before = f.snapshot().await;
            fault(&f, table, timing, body).await;
            let result = command(&f.source, token.as_deref(), &f.code()).await;
            let reached = calls(&f).await;
            let after = f.snapshot().await;
            f.cleanup().await;
            assert_eq!(reached, 1, "rotate={rotate} {body}");
            assert!(result.is_err(), "rotate={rotate} {body}");
            assert_eq!(after, before, "rotate={rotate} {body}");
        }
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_consumption_deferred_commit_failure_rolls_back_the_counter_and_effects() {
    for rotate in [false, true] {
        let f = Fixture::ready().await;
        let token = if rotate {
            Some(setup_rotation(&f).await)
        } else {
            None
        };
        let before = f.snapshot().await;
        sql(&f, "CREATE SEQUENCE consumption_fault_hits").await;
        sql(&f, "CREATE FUNCTION consumption_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM nextval('consumption_fault_hits'); RAISE EXCEPTION 'synthetic OTP commit failure'; END $$").await;
        sql(&f, "CREATE CONSTRAINT TRIGGER consumption_fault AFTER UPDATE ON users DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION consumption_fault()").await;
        let result = command(&f.source, token.as_deref(), &f.code()).await;
        let reached = calls(&f).await;
        let after = f.snapshot().await;
        f.cleanup().await;
        assert_eq!(
            reached, 1,
            "the injected fault must execute at deferred commit"
        );
        assert_eq!(result, Err(DurableAuthError::StorageUnavailable));
        assert_eq!(after, before);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_consumption_cancellation_at_post_write_wait_rolls_back_pending_consumption() {
    for rotate in [false, true] {
        let f = Fixture::ready().await;
        let token = if rotate {
            Some(setup_rotation(&f).await)
        } else {
            None
        };
        let before = f.snapshot().await;
        f.trigger(if rotate {
            "AFTER DELETE"
        } else {
            "AFTER INSERT"
        })
        .await;
        let mut blocker = f.pool.begin().await.unwrap();
        let pid: i32 = query("SELECT pg_backend_pid() AS pid")
            .fetch_one(&mut *blocker)
            .await
            .unwrap()
            .get("pid");
        query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 76531)")
            .execute(&mut *blocker)
            .await
            .unwrap();
        let source = f.source.clone();
        let code = f.code();
        let submitted = code.clone();
        let pending_token = token.clone();
        let pending =
            tokio::spawn(
                async move { command(&source, pending_token.as_deref(), &submitted).await },
            );
        f.wait_blocked(pid).await;
        let reached = f.trigger_calls().await;
        pending.abort();
        let cancelled = pending.await.is_err_and(|error| error.is_cancelled());
        blocker.rollback().await.unwrap();
        drain(&f).await;
        let after = f.snapshot().await;
        sql(&f, "DROP TRIGGER otp_fault ON sessions").await;
        let retry = command(&f.source, token.as_deref(), &code).await;
        f.cleanup().await;
        assert_eq!(reached, 1, "write must reach its exact advisory blocker");
        assert!(cancelled);
        assert_eq!(
            after, before,
            "this observed pre-commit cancellation must roll back"
        );
        assert_eq!(
            retry,
            Ok(()),
            "retry is tested only after rollback was independently observed"
        );
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_consumption_real_collision_pins_highest_step_and_cannot_retarget_at_commit() {
    const LOW: i64 = 910_737;
    const HIGH: i64 = 910_738;
    const CODE: &str = "911617";
    let f = Fixture::ready().await;
    f.clock.store(LOW * 30, Ordering::SeqCst);
    let accepted = command(&f.source, None, CODE).await;
    let stored = counter(&f, username().as_str()).await;
    let replay = command(&f.source, None, CODE).await;
    f.cleanup().await;
    assert_eq!(accepted, Ok(()));
    assert_eq!(stored, HIGH);
    assert_eq!(replay, Err(DurableAuthError::InvalidCredentials));

    let f = Fixture::ready().await;
    f.clock.store((LOW - 1) * 30, Ordering::SeqCst);
    let before = f.snapshot().await;
    f.trigger("AFTER INSERT").await;
    let mut blocker = f.pool.begin().await.unwrap();
    let pid: i32 = query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut *blocker)
        .await
        .unwrap()
        .get("pid");
    query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 76531)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let source = f.source.clone();
    let pending = tokio::spawn(async move { command(&source, None, CODE).await });
    f.wait_blocked(pid).await;
    let fresh_before = f.source.verify_current_totp(SECRET, CODE);
    f.clock.store(LOW * 30, Ordering::SeqCst);
    let fresh_after = f.source.verify_current_totp(SECRET, CODE);
    blocker.rollback().await.unwrap();
    let result = pending.await.unwrap();
    let reached = f.trigger_calls().await;
    let after = f.snapshot().await;
    f.cleanup().await;
    assert!(
        fresh_before && fresh_after,
        "only the selected counter changes, not code freshness"
    );
    assert_eq!(reached, 1);
    assert_eq!(result, Err(DurableAuthError::InvalidCredentials));
    assert_eq!(
        after, before,
        "a pending LOW proposal must not become HIGH at commit"
    );
}
