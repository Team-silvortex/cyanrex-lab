use super::super::{DurableAuthError, DurableAuthSource};
use crate::sqlx_compat::{query, Row};

#[path = "otp_sql_support.rs"]
mod support;
use support::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_login_rechecks_freshness_after_source_writer_wait() {
    let f = Fixture::ready().await;
    let before = f.snapshot().await;
    let mut blocker = f.pool.begin().await.unwrap();
    let pid: i32 = query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut *blocker)
        .await
        .unwrap()
        .get("pid");
    // Shared snapshot reads can finish; only the post-password source writer fence waits.
    query("SELECT version FROM collaboration_auth_source_schema FOR SHARE")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let source = f.source.clone();
    let code = f.code();
    let submitted = code.clone();
    let pending = tokio::spawn(async move {
        source
            .login(&username(), PASSWORD, &submitted)
            .await
            .map(|_| ())
    });
    f.wait_blocked(pid).await;
    f.expire(&code);
    blocker.rollback().await.unwrap();
    let result = pending.await.unwrap();
    let after = f.snapshot().await;
    f.cleanup().await;
    assert_eq!(result, Err(DurableAuthError::InvalidCredentials));
    assert_eq!(after, before, "expired OTP must not publish a Session");
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_login_rechecks_freshness_after_session_insert_wait() {
    let f = Fixture::ready().await;
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
    let code = f.code();
    let submitted = code.clone();
    let pending = tokio::spawn(async move {
        source
            .login(&username(), PASSWORD, &submitted)
            .await
            .map(|_| ())
    });
    f.wait_blocked(pid).await;
    f.expire(&code);
    blocker.rollback().await.unwrap();
    let result = pending.await.unwrap();
    let calls = f.trigger_calls().await;
    let after = f.snapshot().await;
    f.cleanup().await;
    assert_eq!(
        calls, 1,
        "the pending INSERT must reach the post-write wait"
    );
    assert_eq!(result, Err(DurableAuthError::InvalidCredentials));
    assert_eq!(after, before, "the uncommitted Session must be rolled back");
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_rotation_rechecks_freshness_after_session_revoke_wait() {
    let f = Fixture::ready().await;
    let code = f.code();
    let login = f.source.login(&username(), PASSWORD, &code).await.unwrap();
    let code = f.next_code();
    let before = f.snapshot().await;
    f.trigger("AFTER DELETE").await;
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
    let submitted = code.clone();
    let token = login.token.clone();
    let pending = tokio::spawn(async move {
        source
            .change_session_password(&token, PASSWORD, NEW_PASSWORD, &submitted)
            .await
    });
    f.wait_blocked(pid).await;
    f.expire(&code);
    blocker.rollback().await.unwrap();
    let result = pending.await.unwrap();
    let calls = f.trigger_calls().await;
    let after = f.snapshot().await;
    let session = f.source.validate_session(&login.token).await.unwrap();
    f.cleanup().await;
    assert_eq!(
        calls, 1,
        "the pending DELETE must reach the post-write wait"
    );
    assert_eq!(result, Err(DurableAuthError::InvalidCredentials));
    assert_eq!(
        after, before,
        "both credential replacement and revocation must roll back"
    );
    assert_eq!(session, Some(login.session));
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_unchanged_clock_preserves_success_through_writer_and_trigger_waits() {
    for (rotate, trigger) in [(false, false), (false, true), (true, false), (true, true)] {
        let f = Fixture::ready().await;
        let code = f.code();
        let token = if rotate {
            Some(
                f.source
                    .login(&username(), PASSWORD, &code)
                    .await
                    .unwrap()
                    .token,
            )
        } else {
            None
        };
        // Setup login consumes its step. The operation under test gets a new step before
        // installing its exact writer/trigger barrier; the barrier itself still freezes time.
        let code = if rotate { f.next_code() } else { code };
        if trigger {
            f.trigger(if rotate {
                "AFTER DELETE"
            } else {
                "AFTER INSERT"
            })
            .await;
        }
        let mut blocker = f.pool.begin().await.unwrap();
        let pid: i32 = query("SELECT pg_backend_pid() AS pid")
            .fetch_one(&mut *blocker)
            .await
            .unwrap()
            .get("pid");
        query(if trigger {
            "SELECT pg_advisory_xact_lock(hashtext(current_schema()), 76531)"
        } else {
            "SELECT version FROM collaboration_auth_source_schema FOR SHARE"
        })
        .execute(&mut *blocker)
        .await
        .unwrap();
        let source = f.source.clone();
        let submitted = code.clone();
        let original_token = token.clone();
        let pending = tokio::spawn(async move {
            if let Some(token) = token {
                source
                    .change_session_password(&token, PASSWORD, NEW_PASSWORD, &submitted)
                    .await
            } else {
                source
                    .login(&username(), PASSWORD, &submitted)
                    .await
                    .map(|_| ())
            }
        });
        f.wait_blocked(pid).await;
        assert!(!pending.is_finished());
        assert!(f.source.verify_current_totp(SECRET, &code));
        blocker.rollback().await.unwrap();
        let result = pending.await.unwrap();
        let calls = if trigger { f.trigger_calls().await } else { 0 };
        let count = f.count_sessions().await;
        let old_session = if let Some(token) = original_token {
            f.source.validate_session(&token).await.unwrap()
        } else {
            None
        };
        // A successful rotation consumes this step; validate the replacement with a new one.
        let replacement_works = if rotate {
            let replacement_code = f.next_code();
            f.source
                .login(&username(), NEW_PASSWORD, &replacement_code)
                .await
                .is_ok()
        } else {
            true
        };
        f.cleanup().await;
        assert_eq!(result, Ok(()), "rotate={rotate}, trigger={trigger}");
        assert_eq!(calls, i64::from(trigger));
        assert_eq!(count, if rotate { 0 } else { 1 });
        assert_eq!(old_session, None);
        assert!(replacement_works);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_rotation_retains_its_existing_writer_wait_freshness_check() {
    let f = Fixture::ready().await;
    let code = f.code();
    let login = f.source.login(&username(), PASSWORD, &code).await.unwrap();
    let code = f.next_code();
    let before = f.snapshot().await;
    let mut blocker = f.pool.begin().await.unwrap();
    let pid: i32 = query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut *blocker)
        .await
        .unwrap()
        .get("pid");
    query("SELECT version FROM collaboration_auth_source_schema FOR SHARE")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let source = f.source.clone();
    let submitted = code.clone();
    let token = login.token;
    let pending = tokio::spawn(async move {
        source
            .change_session_password(&token, PASSWORD, NEW_PASSWORD, &submitted)
            .await
    });
    f.wait_blocked(pid).await;
    f.expire(&code);
    blocker.rollback().await.unwrap();
    let result = pending.await.unwrap();
    let after = f.snapshot().await;
    f.cleanup().await;
    assert_eq!(result, Err(DurableAuthError::InvalidCredentials));
    assert_eq!(after, before);
}

struct PasswordQueueRelease(Option<std::sync::mpsc::Sender<()>>);
impl Drop for PasswordQueueRelease {
    fn drop(&mut self) {
        if let Some(release) = self.0.take() {
            let _ = release.send(());
        }
    }
}

#[test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
fn postgres_otp_login_rechecks_freshness_while_password_executor_is_held() {
    use std::time::Duration;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let f = Fixture::ready().await;
        let before = f.snapshot().await;
        let application = format!("otp_password_{}", uuid::Uuid::new_v4().simple());
        let pool = f.separate_pool(&application).await;
        let source = f.independent_source(pool.clone());
        let (release, wait) = std::sync::mpsc::channel();
        let release = PasswordQueueRelease(Some(release));
        let (entered, started) = tokio::sync::oneshot::channel();
        let worker = tokio::task::spawn_blocking(move || {
            entered.send(()).unwrap();
            wait.recv_timeout(Duration::from_secs(10)).expect("test password queue was not released");
        });
        tokio::time::timeout(Duration::from_secs(2), started).await.unwrap().unwrap();
        let code = f.code();
        let submitted = code.clone();
        let pending = tokio::spawn(async move {
            source.login(&username(), PASSWORD, &submitted).await.map(|_| ())
        });
        // This pool has never committed before. Its first idle COMMIT proves the snapshot
        // released its SQL connection while the sole blocking worker remains occupied.
        // It does not prove that this login has already dispatched into Tokio's queue.
        // A separate source guard keeps the initial OTP check after password verification.
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let committed: bool = query("SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE datname = current_database() AND application_name = $1 AND state = 'idle' AND query = 'COMMIT') AS committed")
                    .bind(&application).fetch_one(&f.admin).await.unwrap().get("committed");
                if committed { break; }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }).await.expect("login did not release its first snapshot transaction");
        assert!(!pending.is_finished());
        f.expire(&code);
        drop(release);
        worker.await.unwrap();
        let result = pending.await.unwrap();
        let after = f.snapshot().await;
        pool.close().await;
        f.cleanup().await;
        assert_eq!(result, Err(DurableAuthError::InvalidCredentials));
        assert_eq!(after, before);
    });
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_freshness_does_not_claim_single_use_across_independent_sources() {
    let f = Fixture::ready().await;
    let other = f.independent_source(f.pool.clone());
    let code = f.code();
    let name = username();
    let (first, second) = tokio::join!(
        f.source.login(&name, PASSWORD, &code),
        other.login(&name, PASSWORD, &code),
    );
    let outcomes = [
        first.as_ref().map(|_| ()).map_err(|error| *error),
        second.as_ref().map(|_| ()).map_err(|error| *error),
    ];
    let winner = first.ok().or_else(|| second.ok());
    let count = f.count_sessions().await;
    let valid = if let Some(login) = winner.as_ref() {
        f.source.validate_session(&login.token).await.unwrap()
    } else {
        None
    };
    // The freshness helper itself still accepts this code; durable consumption is a separate
    // transaction boundary, now composed into login rather than claimed by the helper.
    let still_fresh =
        f.source.verify_current_totp(SECRET, &code) && other.verify_current_totp(SECRET, &code);
    f.cleanup().await;
    assert!(still_fresh);
    assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|result| **result == Err(DurableAuthError::InvalidCredentials))
            .count(),
        1
    );
    assert_eq!(count, 1);
    assert_eq!(valid, winner.map(|login| login.session));
}
