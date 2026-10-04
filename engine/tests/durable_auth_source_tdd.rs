use chrono::{Duration, Utc};
use cyanrex_engine::{
    models::collaboration::{AuthorityId, LegacyUsername},
    services::auth_service::durable_source::{DurableAuthError, DurableAuthSource},
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use uuid::Uuid;

#[path = "durable_auth_source/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "durable_auth_source/faults.rs"]
mod faults;

#[tokio::test]
async fn closed_durable_auth_source_never_creates_or_authenticates_from_memory() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    pool.close().await;
    let source = DurableAuthSource::new(pool, authority());
    assert_eq!(
        source.install_empty_schema().await,
        Err(DurableAuthError::StorageUnavailable)
    );
    assert!(matches!(
        source.register(&username(), PASSWORD).await,
        Err(DurableAuthError::StorageUnavailable)
    ));
    assert!(matches!(
        source.login(&username(), PASSWORD, "000000").await,
        Err(DurableAuthError::StorageUnavailable)
    ));
    assert_eq!(
        source.validate_session(&Uuid::new_v4().to_string()).await,
        Err(DurableAuthError::StorageUnavailable)
    );
    assert_eq!(
        source.logout(&Uuid::new_v4().to_string()).await,
        Err(DurableAuthError::StorageUnavailable)
    );
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_source_install_is_explicit_empty_only_scoped_and_repeatable() {
    let f = Fixture::new().await;
    assert!(f
        .source
        .login(&username(), PASSWORD, "000000")
        .await
        .is_err());
    assert_eq!(f.table_count().await, 0);
    let other = f.reopen().await;
    let (a, b) = tokio::join!(
        f.source.install_empty_schema(),
        other.install_empty_schema()
    );
    a.unwrap();
    b.unwrap();
    assert_eq!(f.count("users").await, 0);
    f.source.register(&username(), PASSWORD).await.unwrap();
    other.install_empty_schema().await.unwrap();
    assert_eq!(f.count("users").await, 1);
    let wrong = DurableAuthSource::new(f.pool.clone(), id());
    assert_eq!(
        wrong.install_empty_schema().await,
        Err(DurableAuthError::SourceMismatch)
    );
    f.cleanup().await;

    let f = Fixture::new().await;
    for statement in include_str!("../migrations/0001_auth_users_sessions.sql")
        .split(';')
        .filter(|s| !s.trim().is_empty())
    {
        f.sql(statement).await;
    }
    f.sql("INSERT INTO users (username, password_salt, password_hash, totp_secret) VALUES ('legacy-user', 'synthetic', 'synthetic', 'synthetic')").await;
    f.sql("INSERT INTO sessions (token, username, expires_at) VALUES ('synthetic-old-token', 'legacy-user', NOW() + INTERVAL '1 hour')").await;
    assert_eq!(
        f.source.install_empty_schema().await,
        Err(DurableAuthError::LegacyDataPresent)
    );
    assert_eq!(f.count("users").await, 1);
    assert_eq!(f.count("sessions").await, 1);
    assert_eq!(f.table_count().await, 2);
    f.sql("DELETE FROM sessions").await;
    f.sql("DELETE FROM users").await;
    f.source.install_empty_schema().await.unwrap();
    assert_eq!(f.count("users").await, 0);
    f.cleanup().await;

    for alteration in [
        "ALTER TABLE sessions DROP CONSTRAINT sessions_pkey",
        "ALTER TABLE sessions DROP CONSTRAINT sessions_username_fkey",
        "ALTER TABLE users ALTER COLUMN password_hash DROP NOT NULL",
        "ALTER TABLE sessions ALTER COLUMN expires_at TYPE TIMESTAMP",
    ] {
        let f = Fixture::new().await;
        for statement in include_str!("../migrations/0001_auth_users_sessions.sql")
            .split(';')
            .filter(|s| !s.trim().is_empty())
        {
            f.sql(statement).await;
        }
        f.sql(alteration).await;
        assert!(
            f.source.install_empty_schema().await.is_err(),
            "incompatible empty auth schema was silently adopted: {alteration}"
        );
        assert_eq!(f.table_count().await, 2);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_source_registration_and_login_keep_stable_incarnation_after_reload() {
    let f = Fixture::ready().await;
    let created = f.source.register(&username(), PASSWORD).await.unwrap();
    assert_eq!(created.account.authority_id, authority());
    assert_eq!(created.account.username, username());
    let other = f.reopen().await;
    let login = other
        .login(&username(), PASSWORD, &otp(&created.bootstrap.secret))
        .await
        .unwrap();
    assert_eq!(login.session.account, created.account);
    let row = query("SELECT s.token, s.account_id, u.password_hash FROM sessions s JOIN users u USING (username, account_id)")
        .fetch_one(&f.pool).await.unwrap();
    assert_eq!(row.get::<String, _>("token"), token_hash(&login.token));
    assert_ne!(row.get::<String, _>("token"), login.token);
    assert!(row.get::<String, _>("password_hash").starts_with("$argon2"));
    assert_eq!(
        row.get::<Uuid, _>("account_id"),
        created.account.account_id.as_uuid()
    );
    assert_eq!(
        f.source.validate_session(&login.token).await.unwrap(),
        Some(login.session)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_source_duplicate_registration_commits_only_one_incarnation() {
    let f = Fixture::ready().await;
    let other = f.reopen().await;
    let user = username();
    let (a, b) = tokio::join!(
        f.source.register(&user, PASSWORD),
        other.register(&user, PASSWORD)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(matches!(
        if a.is_err() { a } else { b },
        Err(DurableAuthError::AccountExists)
    ));
    assert_eq!(f.count("users").await, 1);
    assert_eq!(f.count("sessions").await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_source_recreated_name_never_reuses_identity_or_old_session() {
    let f = Fixture::ready().await;
    let created = f.source.register(&username(), PASSWORD).await.unwrap();
    let old = f
        .source
        .login(&username(), PASSWORD, &otp(&created.bootstrap.secret))
        .await
        .unwrap();
    // Synthetic privileged maintenance only: no public account-deletion bridge exists yet.
    f.sql("DELETE FROM users").await;
    assert_eq!(f.source.validate_session(&old.token).await.unwrap(), None);
    let replacement = f.source.register(&username(), PASSWORD).await.unwrap();
    assert_ne!(replacement.account.account_id, created.account.account_id);
    let current = f
        .source
        .login(&username(), PASSWORD, &otp(&replacement.bootstrap.secret))
        .await
        .unwrap();
    assert_eq!(current.session.account, replacement.account);
    f.source.logout(&old.token).await.unwrap();
    assert_eq!(
        f.source.validate_session(&current.token).await.unwrap(),
        Some(current.session)
    );
    assert_eq!(f.source.validate_session(&old.token).await.unwrap(), None);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_source_credentials_and_bounded_throttling_gate_session_issuance() {
    let f = Fixture::ready().await;
    assert!(matches!(
        f.source.register(&username(), "short").await,
        Err(DurableAuthError::InvalidInput)
    ));
    let created = f.source.register(&username(), PASSWORD).await.unwrap();
    let code = otp(&created.bootstrap.secret);
    assert!(matches!(
        f.source.login(&username(), PASSWORD, "invalid-otp").await,
        Err(DurableAuthError::InvalidCredentials)
    ));
    for _ in 0..4 {
        assert!(matches!(
            f.source.login(&username(), "wrong-password", &code).await,
            Err(DurableAuthError::InvalidCredentials)
        ));
    }
    assert!(matches!(
        f.source.login(&username(), PASSWORD, &code).await,
        Err(DurableAuthError::RateLimited)
    ));
    assert_eq!(f.count("sessions").await, 0);
    let another: LegacyUsername = "another-learner".parse().unwrap();
    let created = f.source.register(&another, PASSWORD).await.unwrap();
    f.source
        .login(&another, PASSWORD, &otp(&created.bootstrap.secret))
        .await
        .unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_source_logout_and_expiration_never_resurrect_cached_sessions() {
    let f = Fixture::ready().await;
    let (created, login) = f.login().await;
    let other = f.reopen().await;
    assert!(other
        .validate_session(&login.token)
        .await
        .unwrap()
        .is_some());
    f.source.logout(&login.token).await.unwrap();
    f.source.logout(&login.token).await.unwrap();
    assert_eq!(other.validate_session(&login.token).await.unwrap(), None);
    let login = other
        .login(&username(), PASSWORD, &next_otp(&created.bootstrap.secret))
        .await
        .unwrap();
    query("UPDATE sessions SET expires_at = $1")
        .bind(Utc::now() - Duration::seconds(1))
        .execute(&f.pool)
        .await
        .unwrap();
    assert_eq!(other.validate_session(&login.token).await.unwrap(), None);
    assert_eq!(
        f.count("sessions").await,
        1,
        "validation must remain read-only"
    );
    f.source.logout(&login.token).await.unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_source_account_and_session_coordinates_cannot_be_reassigned() {
    let f = Fixture::ready().await;
    let (_, login) = f.login().await;
    for statement in [
        "UPDATE users SET account_id = gen_random_uuid()",
        "UPDATE users SET username = 'another-learner'",
        "UPDATE sessions SET account_id = gen_random_uuid()",
        "UPDATE sessions SET token = repeat('a', 64)",
        "INSERT INTO users (username, password_salt, password_hash, totp_secret) VALUES ('legacy-writer', 'synthetic', 'synthetic', 'synthetic')",
        "INSERT INTO sessions (token, username, expires_at) SELECT repeat('b', 64), username, NOW() + INTERVAL '1 hour' FROM users",
        "INSERT INTO sessions (token, username, account_id, expires_at) SELECT repeat('c', 64), username, gen_random_uuid(), NOW() + INTERVAL '1 hour' FROM users",
    ] {
        assert!(query(statement).execute(&f.pool).await.is_err(), "unfenced source mutation: {statement}");
    }
    assert!(f
        .source
        .validate_session(&login.token)
        .await
        .unwrap()
        .is_some());
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_source_wrong_authority_and_token_shapes_never_select_an_identity() {
    let f = Fixture::ready().await;
    let (_, login) = f.login().await;
    let wrong = DurableAuthSource::new(f.pool.clone(), id());
    assert_eq!(
        wrong.validate_session(&login.token).await,
        Err(DurableAuthError::SourceMismatch)
    );
    assert_eq!(
        wrong.logout(&login.token).await,
        Err(DurableAuthError::SourceMismatch)
    );
    assert!(matches!(
        wrong.register(&username(), PASSWORD).await,
        Err(DurableAuthError::SourceMismatch)
    ));
    assert!(matches!(
        wrong.login(&username(), PASSWORD, "000000").await,
        Err(DurableAuthError::SourceMismatch)
    ));
    for token in [
        String::new(),
        format!(" {} ", login.token),
        token_hash(&login.token),
        "x".repeat(4096),
    ] {
        assert_eq!(f.source.validate_session(&token).await.unwrap(), None);
    }
    assert!(f
        .source
        .validate_session(&login.token)
        .await
        .unwrap()
        .is_some());
    f.cleanup().await;
}
