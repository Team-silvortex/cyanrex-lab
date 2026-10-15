use super::fixture::*;
use crate::services::auth_service::durable_source::{DurableAuthError, SessionStockLimits};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_stock_immediate_and_deferred_extra_rows_roll_back_even_below_configured_limits(
) {
    for (table, timing, deferred, per_account, total) in [
        ("users", "AFTER UPDATE OF otp_last_counter", false, 4, 8),
        ("sessions", "AFTER INSERT", false, 1, 1),
        ("sessions", "AFTER INSERT", true, 4, 8),
    ] {
        let (before, after, result, hits, recovery) = Fixture::checked(|f| Box::pin(async move {
            let digest = "a".repeat(64);
            // A token guard, not trigger depth, also prevents deferred child invocations recurring.
            let guard = if table == "sessions" { format!("IF NEW.token <> '{digest}' THEN") }
                else { "IF NEW.otp_last_counter <> OLD.otp_last_counter THEN".to_owned() };
            let body = format!("{guard} {}
                INSERT INTO sessions (token, username, account_id, expires_at)
                VALUES ('{digest}', NEW.username, NEW.account_id, clock_timestamp() - INTERVAL '1 hour');
                END IF; RETURN NEW;", f.hit_sql());
            f.fault(table, timing, &body, deferred).await;
            let before = f.snapshot().await;
            let code = f.code();
            let limits = SessionStockLimits::new(per_account, total).unwrap();
            let result = within(f.source().login_with_session_limits(&f.first, PASSWORD, &code, limits))
                .await.map(|_| ());
            f.drain().await;
            let hits = f.calls().await;
            let after = f.snapshot().await;
            f.sql(&format!("DROP TRIGGER stock_fault ON {table}")).await;
            // An independent rollback observation precedes this explicit synthetic healthy control.
            let recovery = within(f.source().login_with_session_limits(&f.first, PASSWORD, &code, limits))
                .await.map(|_| ());
            (before, after, result, hits, recovery)
        })).await;
        assert_eq!(
            hits, 1,
            "the original write did not execute its exact synthetic fault"
        );
        assert_eq!(result, Err(DurableAuthError::StorageUnavailable));
        assert!(
            unchanged(&before, &after),
            "extra rows or OTP consumption committed"
        );
        assert_eq!((after.sessions, after.watermark), (0, -1));
        assert_eq!(recovery, Ok(()));
    }
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_stock_original_source_drift_and_deferred_constraint_failure_never_publish_a_session(
) {
    for drift in [true, false] {
        let (before, after, result, hits, recovery) = Fixture::checked(|f| {
            Box::pin(async move {
                let table;
                if drift {
                    let shadow = f.shadow().await;
                    let body = format!(
                        "{} PERFORM set_config('search_path', '{shadow}', true); RETURN NEW;",
                        f.hit_sql()
                    );
                    f.fault("sessions", "AFTER INSERT", &body, false).await;
                    table = "sessions";
                } else {
                    let body = format!(
                        "{} RAISE EXCEPTION 'synthetic deferred stock failure'; RETURN NEW;",
                        f.hit_sql()
                    );
                    f.fault("users", "AFTER UPDATE", &body, true).await;
                    table = "users";
                }
                let before = f.snapshot().await;
                let code = f.code();
                let limits = SessionStockLimits::new(4, 8).unwrap();
                let result = within(
                    f.source()
                        .login_with_session_limits(&f.first, PASSWORD, &code, limits),
                )
                .await
                .map(|_| ());
                f.drain().await;
                let hits = f.calls().await;
                let after = f.snapshot().await;
                f.sql(&format!("DROP TRIGGER stock_fault ON {table}")).await;
                let recovery = within(
                    f.source()
                        .login_with_session_limits(&f.first, PASSWORD, &code, limits),
                )
                .await
                .map(|_| ());
                (before, after, result, hits, recovery)
            })
        })
        .await;
        assert_eq!(hits, 1);
        assert_eq!(
            result,
            Err(if drift {
                DurableAuthError::SourceMismatch
            } else {
                DurableAuthError::StorageUnavailable
            })
        );
        assert!(
            unchanged(&before, &after),
            "failed finalization changed original Source rows or relation identity"
        );
        assert_eq!(recovery, Ok(()));
    }
}
