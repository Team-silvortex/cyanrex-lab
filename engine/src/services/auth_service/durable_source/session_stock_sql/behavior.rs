use super::fixture::*;
use crate::services::auth_service::durable_source::{DurableAuthError, SessionStockLimits};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_stock_full_account_preserves_expired_device_and_unused_otp_until_explicit_logout(
) {
    let (before, rejected_state, released, completed, rejected, accepted, validated, valid_code) =
        Fixture::checked(|f| {
            Box::pin(async move {
                let existing = f.seed_expired_session().await;
                let code = f.code();
                let valid_code = f.code_is_current(&code);
                let limits = SessionStockLimits::new(1, 2)
                    .unwrap_or_else(|_| panic!("valid trusted stock limits rejected"));
                let before = f.snapshot().await;
                let rejected = within(
                    f.source()
                        .login_with_session_limits(&f.first, PASSWORD, &code, limits),
                )
                .await
                .map(|_| ());
                let rejected_state = f.snapshot().await;
                // This explicit fixture operation, not login, is the only deletion allowed here.
                within(f.source().logout(&existing))
                    .await
                    .unwrap_or_else(|_| panic!("explicit original-device logout failed"));
                let released = f.snapshot().await;
                let accepted = within(
                    f.source()
                        .login_with_session_limits(&f.first, PASSWORD, &code, limits),
                )
                .await;
                let validated = if let Ok(login) = &accepted {
                    within(f.source().validate_session(&login.token))
                        .await
                        .unwrap_or_else(|_| panic!("bounded-login token verification failed"))
                        .as_ref()
                        == Some(&login.session)
                } else {
                    false
                };
                let accepted = accepted.map(|_| ());
                let completed = f.snapshot().await;
                (
                    before,
                    rejected_state,
                    released,
                    completed,
                    rejected,
                    accepted,
                    validated,
                    valid_code,
                )
            })
        })
        .await;

    // Scaffold behavior is asserted only after owned database cleanup.
    assert!(valid_code, "baseline used an already invalid synthetic OTP");
    assert_eq!(before.sessions, 1);
    assert_eq!(before.watermark, -1);
    assert_eq!(rejected, Err(DurableAuthError::RateLimited));
    assert!(
        unchanged(&before, &rejected_state),
        "stock refusal consumed OTP or altered stored devices"
    );
    assert_eq!(released.sessions, 0);
    assert_eq!(released.watermark, -1);
    assert_eq!(
        accepted,
        Ok(()),
        "the same unconsumed code must work after explicit capacity release"
    );
    assert!(validated);
    assert_eq!(completed.sessions, 1);
    assert!(completed.watermark > -1);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_stock_total_counts_other_accounts_without_consuming_the_requesting_accounts_otp(
) {
    let (before, refused, completed, rejected, accepted, other_before, other_after) =
        Fixture::checked(|f| {
            Box::pin(async move {
                let other = "stock-other-account".parse().unwrap();
                f.register_name(&other).await;
                let existing = f
                    .seed_session(&other, "clock_timestamp() - INTERVAL '1 hour'")
                    .await;
                let code = f.code();
                let limits = SessionStockLimits::new(1, 1).unwrap();
                let before = f.snapshot().await;
                let other_before = f.counter(&other).await;
                let rejected = within(
                    f.source()
                        .login_with_session_limits(&f.first, PASSWORD, &code, limits),
                )
                .await
                .map(|_| ());
                let refused = f.snapshot().await;
                within(f.source().logout(&existing)).await.unwrap();
                let accepted = within(
                    f.source()
                        .login_with_session_limits(&f.first, PASSWORD, &code, limits),
                )
                .await
                .map(|_| ());
                let completed = f.snapshot().await;
                let other_after = f.counter(&other).await;
                (
                    before,
                    refused,
                    completed,
                    rejected,
                    accepted,
                    other_before,
                    other_after,
                )
            })
        })
        .await;
    assert_eq!(rejected, Err(DurableAuthError::RateLimited));
    assert!(unchanged(&before, &refused));
    assert_eq!(before.sessions, 1);
    assert_eq!(before.watermark, -1);
    assert_eq!(accepted, Ok(()));
    assert_eq!(completed.sessions, 1);
    assert_eq!((other_before, other_after), (-1, -1));
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_stock_policy_construction_does_not_change_the_legacy_login_entry_point() {
    let (before, configured, legacy, validated, old_state, refused, bounded) =
        Fixture::checked(|f| {
            Box::pin(async move {
                f.seed_expired_session().await;
                let before = f.snapshot().await;
                let limits = SessionStockLimits::new(1, 1).unwrap();
                let configured = f.snapshot().await;
                let code = f.code();
                let legacy = within(f.source().login(&f.first, PASSWORD, &code)).await;
                let validated = if let Ok(login) = &legacy {
                    within(f.source().validate_session(&login.token))
                        .await
                        .unwrap()
                        .as_ref()
                        == Some(&login.session)
                } else {
                    false
                };
                let legacy = legacy.map(|_| ());
                let old_state = f.snapshot().await;
                let next = f.next_code();
                let bounded = within(
                    f.source()
                        .login_with_session_limits(&f.first, PASSWORD, &next, limits),
                )
                .await
                .map(|_| ());
                let refused = f.snapshot().await;
                (
                    before, configured, legacy, validated, old_state, refused, bounded,
                )
            })
        })
        .await;
    assert!(
        unchanged(&before, &configured),
        "constructing a policy installed a global quota"
    );
    assert_eq!(legacy, Ok(()));
    assert!(validated);
    assert_eq!(old_state.sessions, 2);
    assert_eq!(bounded, Err(DurableAuthError::RateLimited));
    assert!(
        unchanged(&old_state, &refused),
        "bounded refusal changed legacy devices or OTP state"
    );
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_stock_counts_expired_infinite_and_out_of_chrono_range_rows_without_decoding_or_cleanup(
) {
    for expires in [
        "clock_timestamp() - INTERVAL '1 hour'",
        "'-infinity'::timestamptz",
        "'infinity'::timestamptz",
        "'294276-12-31 00:00:00+00'::timestamptz",
    ] {
        let (before, after, result, valid) = Fixture::checked(|f| {
            Box::pin(async move {
                f.seed_session(&f.first, expires).await;
                let code = f.code();
                let valid = f.code_is_current(&code);
                let before = f.snapshot().await;
                let result = within(f.source().login_with_session_limits(
                    &f.first,
                    PASSWORD,
                    &code,
                    SessionStockLimits::new(1, 1).unwrap(),
                ))
                .await
                .map(|_| ());
                let after = f.snapshot().await;
                (before, after, result, valid)
            })
        })
        .await;
        assert!(valid);
        assert_eq!(result, Err(DurableAuthError::RateLimited));
        assert!(
            unchanged(&before, &after),
            "a stored device was decoded, filtered or cleaned"
        );
        assert_eq!((after.sessions, after.watermark), (1, -1));
    }
}
