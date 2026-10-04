use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::{AuthorityId, LegacyUsername},
    services::auth_service::durable_source::{DurableAuthError, DurableAuthSource},
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use uuid::Uuid;

#[allow(dead_code)]
#[path = "durable_auth_source/fixture.rs"]
mod source_fixture;
use source_fixture::*;
#[path = "durable_password_change/concurrency.rs"]
mod concurrency;
#[path = "durable_password_change/faults.rs"]
mod faults;
#[path = "durable_password_change/registry.rs"]
mod registry;

const NEW_PASSWORD: &str = "synthetic-replacement-password";

async fn state(f: &Fixture) -> (String, String) {
    let users = query("SELECT COALESCE(jsonb_agg(to_jsonb(u) ORDER BY username), '[]'::jsonb)::text AS state FROM users u")
        .fetch_one(&f.pool).await.unwrap().get("state");
    let sessions = query("SELECT COALESCE(jsonb_agg(to_jsonb(s) ORDER BY token), '[]'::jsonb)::text AS state FROM sessions s")
        .fetch_one(&f.pool).await.unwrap().get("state");
    (users, sessions)
}

#[tokio::test]
async fn closed_password_source_never_changes_credentials_from_memory() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    pool.close().await;
    let source = DurableAuthSource::new(pool, authority());
    assert_eq!(
        source
            .change_session_password(
                &Uuid::new_v4().to_string(),
                PASSWORD,
                NEW_PASSWORD,
                "000000"
            )
            .await,
        Err(DurableAuthError::StorageUnavailable)
    );
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_change_revokes_all_sessions_and_preserves_identity() {
    let f = Fixture::ready().await;
    let (account, first) = f.seed_account_session().await;
    // Device inventory is explicit fixture state; the rotation below is the real OTP consumer.
    let second = f.seed_session(&account.account).await;
    let expired = Uuid::new_v4().to_string();
    query("INSERT INTO sessions (token, username, account_id, expires_at) VALUES ($1, $2, $3, clock_timestamp() - INTERVAL '1 second')")
        .bind(token_hash(&expired)).bind(username().as_str()).bind(account.account.account_id.as_uuid())
        .execute(&f.pool).await.unwrap();
    let other_name: LegacyUsername = "untouched-account".parse().unwrap();
    let other = f.source.register(&other_name, PASSWORD).await.unwrap();
    let other_login = f
        .source
        .login(&other_name, PASSWORD, &otp(&other.bootstrap.secret))
        .await
        .unwrap();
    f.source
        .change_session_password(
            &first.token,
            PASSWORD,
            NEW_PASSWORD,
            &otp(&account.bootstrap.secret),
        )
        .await
        .unwrap();
    let reopened = f.reopen().await;
    for token in [&first.token, &second.token, &expired] {
        assert_eq!(reopened.validate_session(token).await.unwrap(), None);
    }
    assert_eq!(f.count("users").await, 2);
    assert_eq!(f.count("sessions").await, 1);
    assert_eq!(
        reopened.validate_session(&other_login.token).await.unwrap(),
        Some(other_login.session)
    );
    assert!(matches!(
        reopened
            .login(&username(), PASSWORD, &next_otp(&account.bootstrap.secret))
            .await,
        Err(DurableAuthError::InvalidCredentials)
    ));
    let new_login = reopened
        .login(
            &username(),
            NEW_PASSWORD,
            &next_otp(&account.bootstrap.secret),
        )
        .await
        .unwrap();
    assert_eq!(new_login.session.account, account.account);
    assert_eq!(
        f.table_count().await,
        3,
        "password change must not create registry state"
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_change_requires_reauthentication_and_bounded_inputs() {
    let f = Fixture::ready().await;
    let (account, login) = f.seed_account_session().await;
    let before = state(&f).await;
    let code = otp(&account.bootstrap.secret);
    for (current, new, otp, error) in [
        (
            "wrong".to_string(),
            NEW_PASSWORD.to_string(),
            code.clone(),
            DurableAuthError::InvalidCredentials,
        ),
        (
            PASSWORD.to_string(),
            NEW_PASSWORD.to_string(),
            "invalid".to_string(),
            DurableAuthError::InvalidCredentials,
        ),
        (
            "x".repeat(4097),
            NEW_PASSWORD.to_string(),
            code.clone(),
            DurableAuthError::InvalidInput,
        ),
        (
            PASSWORD.to_string(),
            "short".to_string(),
            code.clone(),
            DurableAuthError::InvalidInput,
        ),
        (
            PASSWORD.to_string(),
            "x".repeat(4097),
            code.clone(),
            DurableAuthError::InvalidInput,
        ),
        (
            PASSWORD.to_string(),
            NEW_PASSWORD.to_string(),
            "0".repeat(65),
            DurableAuthError::InvalidInput,
        ),
    ] {
        assert_eq!(
            f.source
                .change_session_password(&login.token, &current, &new, &otp)
                .await,
            Err(error)
        );
        assert_eq!(state(&f).await, before);
    }
    // A deliberate same-password rotation still creates a fresh salt and revokes all devices.
    f.source
        .change_session_password(&login.token, PASSWORD, PASSWORD, &code)
        .await
        .unwrap();
    assert_eq!(f.count("sessions").await, 0);
    assert_ne!(state(&f).await.0, before.0);
    f.source
        .login(&username(), PASSWORD, &next_otp(&account.bootstrap.secret))
        .await
        .unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_change_rejects_absent_revoked_expired_and_recreated_sessions() {
    for failure in [
        "shape",
        "digest",
        "unknown",
        "revoked",
        "expired",
        "recreated",
    ] {
        let f = Fixture::ready().await;
        let (account, login) = f.seed_account_session().await;
        let token = match failure {
            "shape" => "not-a-token".to_string(),
            "digest" => token_hash(&login.token),
            "unknown" => Uuid::new_v4().to_string(),
            "revoked" => {
                f.source.logout(&login.token).await.unwrap();
                login.token
            }
            "expired" => {
                f.sql("UPDATE sessions SET expires_at = clock_timestamp() - INTERVAL '1 second'")
                    .await;
                login.token
            }
            "recreated" => {
                f.sql("DELETE FROM users").await;
                let replacement = f.source.register(&username(), PASSWORD).await.unwrap();
                assert_ne!(replacement.account.account_id, account.account.account_id);
                login.token
            }
            _ => unreachable!(),
        };
        let before = state(&f).await;
        assert_eq!(
            f.source
                .change_session_password(
                    &token,
                    PASSWORD,
                    NEW_PASSWORD,
                    &otp(&account.bootstrap.secret)
                )
                .await,
            Err(DurableAuthError::InvalidCredentials)
        );
        assert_eq!(state(&f).await, before);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_change_shares_bounded_admission_with_login_and_clones() {
    let f = Fixture::ready().await;
    let (account, login) = f.seed_account_session().await;
    let before = state(&f).await;
    let clone = f.source.clone();
    for _ in 0..5 {
        assert_eq!(
            clone
                .change_session_password(
                    &login.token,
                    "wrong",
                    NEW_PASSWORD,
                    &otp(&account.bootstrap.secret)
                )
                .await,
            Err(DurableAuthError::InvalidCredentials)
        );
    }
    assert_eq!(
        f.source
            .change_session_password(
                &login.token,
                PASSWORD,
                NEW_PASSWORD,
                &otp(&account.bootstrap.secret)
            )
            .await,
        Err(DurableAuthError::RateLimited)
    );
    assert!(matches!(
        f.source
            .login(&username(), PASSWORD, &otp(&account.bootstrap.secret))
            .await,
        Err(DurableAuthError::RateLimited)
    ));
    assert_eq!(state(&f).await, before);
    // The admission budget is deliberately process/instance-local, not a durable global throttle.
    f.reopen()
        .await
        .change_session_password(
            &login.token,
            PASSWORD,
            NEW_PASSWORD,
            &otp(&account.bootstrap.secret),
        )
        .await
        .unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_change_requires_valid_source_without_install_or_fallback() {
    let f = Fixture::new().await;
    assert_eq!(
        f.source
            .change_session_password(
                &Uuid::new_v4().to_string(),
                PASSWORD,
                NEW_PASSWORD,
                "000000"
            )
            .await,
        Err(DurableAuthError::StorageUnavailable)
    );
    assert_eq!(f.table_count().await, 0);
    f.cleanup().await;
    for (damage, error) in [
        (
            "UPDATE collaboration_auth_source_schema SET version = 999",
            DurableAuthError::UnsupportedSchema,
        ),
        (
            "UPDATE collaboration_auth_source_schema SET authority_id = gen_random_uuid()",
            DurableAuthError::SourceMismatch,
        ),
        (
            "ALTER TABLE sessions RENAME TO offline_sessions",
            DurableAuthError::StorageUnavailable,
        ),
        (
            "UPDATE users SET password_hash = 'corrupt'",
            DurableAuthError::InvalidRecord,
        ),
    ] {
        let f = Fixture::ready().await;
        let (account, login) = f.seed_account_session().await;
        let original = state(&f).await;
        f.sql(damage).await;
        assert_eq!(
            f.source
                .change_session_password(
                    &login.token,
                    PASSWORD,
                    NEW_PASSWORD,
                    &otp(&account.bootstrap.secret)
                )
                .await,
            Err(error)
        );
        if damage.contains("RENAME") {
            f.sql("ALTER TABLE offline_sessions RENAME TO sessions")
                .await;
        }
        let after = state(&f).await;
        assert_eq!(after.1, original.1);
        if error != DurableAuthError::InvalidRecord {
            assert_eq!(after.0, original.0);
        }
        f.cleanup().await;
    }
}
