use super::fixture::*;
use crate::{
    services::auth_service::durable_source::{DurableAuthError, SessionStockLimits},
    sqlx_compat::{query, Row},
};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_stock_independent_repeatable_read_sources_compete_for_the_last_persisted_session(
) {
    let (first, second, count, first_counter, second_counter, pids, rr, validated) =
        Fixture::checked(|f| {
            Box::pin(async move {
                let other = "stock-concurrent-other".parse().unwrap();
                f.register_name(&other).await;
                let (first_source, first_pool) = f.independent("first_rr", true).await;
                let (second_source, second_pool) = f.independent("second_rr", true).await;
                let mut rr = true;
                for pool in [&first_pool, &second_pool] {
                    let actual: String =
                        within(query("SHOW default_transaction_isolation").fetch_one(pool))
                            .await
                            .unwrap()
                            .get(0);
                    rr &= actual == "repeatable read";
                }
                let mut holder = within(f.pool().begin()).await.unwrap();
                // Initial read transactions are compatible; only both post-password writers wait.
                within(
                    query("SELECT singleton FROM collaboration_auth_source_schema FOR SHARE")
                        .fetch_one(&mut *holder),
                )
                .await
                .unwrap();
                let blocker: i32 =
                    within(query("SELECT pg_backend_pid() AS pid").fetch_one(&mut *holder))
                        .await
                        .unwrap()
                        .get("pid");
                let first_name = f.first.clone();
                let second_name = other.clone();
                let first_code = f.code();
                let second_code = f.code();
                let limits = SessionStockLimits::new(1, 1).unwrap();
                let first = Pending::start(async move {
                    first_source
                        .login_with_session_limits(&first_name, PASSWORD, &first_code, limits)
                        .await
                });
                let second = Pending::start(async move {
                    second_source
                        .login_with_session_limits(&second_name, PASSWORD, &second_code, limits)
                        .await
                });
                let first_pid = f.blocked_writer("first_rr", blocker).await;
                let second_pid = f.blocked_writer("second_rr", blocker).await;
                within(holder.rollback()).await.unwrap();
                let first = first.finish().await;
                let second = second.finish().await;
                let mut validated = true;
                for result in [&first, &second] {
                    if let Ok(login) = result {
                        validated &= within(f.source().validate_session(&login.token))
                            .await
                            .unwrap()
                            .as_ref()
                            == Some(&login.session);
                    }
                }
                f.drain().await;
                let count = f.snapshot().await.sessions;
                let first_counter = f.counter(&f.first).await;
                let second_counter = f.counter(&other).await;
                (
                    first.map(|_| ()),
                    second.map(|_| ()),
                    count,
                    first_counter,
                    second_counter,
                    (first_pid, second_pid),
                    rr,
                    validated,
                )
            })
        })
        .await;
    assert!(
        rr && validated,
        "RR independent Sources or returned Session control were not real"
    );
    assert!(pids.0 > 0 && pids.1 > 0);
    assert_ne!(
        pids.0, pids.1,
        "competition used one backend instead of independent Sources"
    );
    let results = [first, second];
    assert_eq!(
        results.iter().filter(|result| result.is_ok()).count(),
        1,
        "independent writer results: {results:?}"
    );
    assert_eq!(
        results
            .iter()
            .filter(|result| **result == Err(DurableAuthError::RateLimited))
            .count(),
        1
    );
    assert_eq!(count, 1);
    for (result, counter) in results.iter().zip([first_counter, second_counter]) {
        if result.is_ok() {
            assert!(counter > -1);
        } else {
            assert_eq!(counter, -1, "a capacity loser consumed its independent OTP");
        }
    }
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_stock_actual_post_insert_cancellation_preserves_stock_and_otp_after_observed_rollback(
) {
    let (before, after, hits, waited, cancelled, recovery) = Fixture::checked(|f| {
        Box::pin(async move {
            let body = format!(
                "{} PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 74621); RETURN NEW;",
                f.hit_sql()
            );
            f.fault("sessions", "AFTER INSERT", &body, false).await;
            let before = f.snapshot().await;
            let mut holder = within(f.pool().begin()).await.unwrap();
            within(
                query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 74621)")
                    .execute(&mut *holder),
            )
            .await
            .unwrap();
            let blocker: i32 =
                within(query("SELECT pg_backend_pid() AS pid").fetch_one(&mut *holder))
                    .await
                    .unwrap()
                    .get("pid");
            let source = f.source().clone();
            let name = f.first.clone();
            let code = f.code();
            let submitted = code.clone();
            let limits = SessionStockLimits::new(4, 8).unwrap();
            let pending = Pending::start(async move {
                source
                    .login_with_session_limits(&name, PASSWORD, &submitted, limits)
                    .await
                    .map(|_| ())
            });
            let waited = f.blocked_trigger(blocker).await;
            let hits = f.calls().await;
            let cancelled = pending.cancel().await;
            within(holder.rollback()).await.unwrap();
            // Cancellation is not rollback proof: observe the actual writer has drained first.
            f.drain().await;
            let after = f.snapshot().await;
            f.sql("DROP TRIGGER stock_fault ON sessions").await;
            let recovery = within(
                f.source()
                    .login_with_session_limits(&f.first, PASSWORD, &code, limits),
            )
            .await
            .map(|_| ());
            (before, after, hits, waited, cancelled, recovery)
        })
    })
    .await;
    assert!(cancelled && waited > 0);
    assert_eq!(
        hits, 1,
        "cancelled call never reached its post-insert blocker"
    );
    assert!(
        unchanged(&before, &after),
        "the observed precommit cancellation left committed stock or OTP state"
    );
    assert_eq!((after.sessions, after.watermark), (0, -1));
    assert_eq!(recovery, Ok(()));
}
