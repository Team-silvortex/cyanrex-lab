//! Stored-PHC acceptance regressions for the prepared source only, not live authentication.
//! Deliberately malformed/oversized numbers are never sent to an unguarded expensive KDF.
use argon2::password_hash::PasswordHash;
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::{AuthorityId, LegacyUsername, WorkspaceRef},
    services::auth_service::durable_source::{DurableAuthError, DurableAuthSource},
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use uuid::Uuid;

#[allow(dead_code)]
#[path = "durable_auth_source/fixture.rs"]
mod source_fixture;
use source_fixture::*;

const NEW_PASSWORD: &str = "synthetic-profile-replacement-password";

fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: authority(),
        workspace_id: "60a9845d-0687-4f19-a4f5-2ebf64bed4ba".parse().unwrap(),
    }
}

async fn bootstrap_guard() -> sqlx_core::transaction::Transaction<'static, sqlx_postgres::Postgres>
{
    // Cooperate with existing bootstrap tests that install database-global event hooks.
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&std::env::var("CYANREX_TEST_DATABASE_URL").expect("disposable test DB required"))
        .await
        .unwrap();
    let mut guard = pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext(current_database()), hashtext('cyanrex.test.bootstrap.event-hooks'))")
        .execute(&mut *guard).await.unwrap();
    guard
}

async fn stored_hash(f: &Fixture) -> String {
    query("SELECT password_hash FROM users WHERE username = $1")
        .bind(username().as_str())
        .fetch_one(&f.pool)
        .await
        .unwrap()
        .get("password_hash")
}

async fn replace_hash(f: &Fixture, hash: &str) {
    let changed = query("UPDATE users SET password_hash = $1 WHERE username = $2")
        .bind(hash)
        .bind(username().as_str())
        .execute(&f.pool)
        .await
        .unwrap();
    assert_eq!(changed.rows_affected(), 1);
}

async fn snapshot(f: &Fixture, registry: bool) -> Vec<String> {
    let mut tables = vec!["users", "sessions", "collaboration_auth_source_schema"];
    if registry {
        tables.extend([
            "collaboration_principals",
            "collaboration_legacy_identities",
            "collaboration_memberships",
            "collaboration_deployment_grants",
            "collaboration_identity_audit",
            "collaboration_policy_audit",
        ]);
    }
    let mut state = Vec::new();
    for table in tables {
        state.push(query(&format!("SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text), '[]'::jsonb)::text AS state FROM {table} t"))
            .fetch_one(&f.pool).await.unwrap().get("state"));
    }
    state
}

async fn assert_writer_profile(f: &Fixture) -> String {
    let row = query("SELECT password_hash, password_salt FROM users WHERE username = $1")
        .bind(username().as_str())
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let encoded: String = row.get("password_hash");
    let salt: String = row.get("password_salt");
    let parsed = PasswordHash::new(&encoded).unwrap();
    assert_eq!(parsed.algorithm.as_str(), "argon2id");
    assert_eq!(parsed.version, Some(19));
    assert_eq!(parsed.params.as_str(), "m=19456,t=2,p=1");
    assert_eq!(parsed.hash.unwrap().len(), 32);
    let mut decoded = [0u8; 48];
    assert_eq!(
        parsed.salt.unwrap().decode_b64(&mut decoded).unwrap(),
        salt.as_bytes()
    );
    assert_eq!(salt.len(), 36);
    assert_eq!(Uuid::parse_str(&salt).unwrap().to_string(), salt);
    encoded
}

async fn fault(f: &Fixture, table: &str, timing: &str, body: &str) {
    assert!(matches!(table, "users" | "sessions"));
    f.sql("CREATE SEQUENCE profile_fault_calls").await;
    f.sql(&format!(
        "CREATE FUNCTION profile_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.profile_fault_calls'); {body} END $$",
        f.schema
    ))
    .await;
    f.sql(&format!("CREATE TRIGGER profile_fault {timing} ON {table} FOR EACH ROW EXECUTE FUNCTION profile_fault()"))
        .await;
}

async fn assert_fault_fired(f: &Fixture) {
    let row = query("SELECT last_value, is_called FROM profile_fault_calls")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert!(
        row.get::<bool, _>("is_called"),
        "write fault was not reached"
    );
    assert_eq!(row.get::<i64, _>("last_value"), 1);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_profile_login_rejects_unsupported_records_without_issuing_sessions() {
    let f = Fixture::ready().await;
    let account = f.source.register(&username(), PASSWORD).await.unwrap();
    let original = assert_writer_profile(&f).await;
    let variants = [
        original.replace("m=19456", "m=8"),
        original.replace("t=2", "t=1"),
        original.replace("m=19456", "m=19456,m=8"),
        original.replace("m=19456", "m=8,m=19456"),
        original.replace("p=1", "p=1,p=1"),
        original.replace("m=19456", &format!("m=19456,m={}", "9".repeat(200))),
        original.replace("p=1", "p=4294967296"),
        original.replace("t=2,", ""),
        original.replace("$v=19", ""),
        original.replace("$v=19", "$v=16"),
        original.replace("argon2id", "argon2i"),
        original.replace("p=1", "p=1,data=YQ"),
        "$argon2-invalid".to_owned(),
    ];
    for damaged in variants {
        replace_hash(&f, &damaged).await;
        let before = snapshot(&f, false).await;
        // Independent callers avoid the separate five-attempt username budget masking a case.
        let source = DurableAuthSource::new(f.pool.clone(), authority());
        assert!(matches!(
            source
                .login(&username(), PASSWORD, &otp(&account.bootstrap.secret))
                .await,
            Err(DurableAuthError::InvalidRecord)
        ));
        assert_eq!(snapshot(&f, false).await, before);
        assert_eq!(f.count("sessions").await, 0);
    }
    replace_hash(&f, &original).await;
    let login = f
        .source
        .login(&username(), PASSWORD, &otp(&account.bootstrap.secret))
        .await
        .unwrap();
    assert_eq!(login.session.account, account.account);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_profile_existing_session_is_not_implicitly_revoked_and_can_logout() {
    let f = Fixture::ready().await;
    let (_, login) = f.login().await;
    let damaged = stored_hash(&f).await.replace("m=19456", "m=8");
    replace_hash(&f, &damaged).await;
    let before = snapshot(&f, false).await;
    assert_eq!(
        f.source.validate_session(&login.token).await.unwrap(),
        Some(login.session)
    );
    assert_eq!(snapshot(&f, false).await, before);
    f.source.logout(&login.token).await.unwrap();
    f.source.logout(&login.token).await.unwrap();
    assert_eq!(f.source.validate_session(&login.token).await.unwrap(), None);
    assert_eq!(stored_hash(&f).await, damaged);
    assert_eq!(f.count("users").await, 1);
    assert_eq!(f.count("sessions").await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_profile_rotation_rejects_bad_record_without_changing_session_or_registry(
) {
    let _guard = bootstrap_guard().await;
    let f = Fixture::new().await;
    let boot = f
        .source
        .bootstrap_empty_authority(scope(), &username(), PASSWORD)
        .await
        .unwrap();
    let login = f
        .source
        .login(
            &username(),
            PASSWORD,
            &otp(&boot.registration.bootstrap.secret),
        )
        .await
        .unwrap();
    replace_hash(&f, &stored_hash(&f).await.replace("t=2", "t=1")).await;
    let before = snapshot(&f, true).await;
    assert_eq!(
        f.source
            .change_session_password(
                &login.token,
                PASSWORD,
                NEW_PASSWORD,
                &otp(&boot.registration.bootstrap.secret)
            )
            .await,
        Err(DurableAuthError::InvalidRecord)
    );
    assert_eq!(snapshot(&f, true).await, before);
    assert_eq!(
        f.source.validate_session(&login.token).await.unwrap(),
        Some(login.session)
    );
    assert_eq!(f.count("sessions").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_profile_official_registration_rotation_and_bootstrap_remain_usable() {
    let f = Fixture::ready().await;
    let (account, login) = f.seed_account_session().await;
    let first = assert_writer_profile(&f).await;
    f.source
        .change_session_password(
            &login.token,
            PASSWORD,
            NEW_PASSWORD,
            &otp(&account.bootstrap.secret),
        )
        .await
        .unwrap();
    let rotated = assert_writer_profile(&f).await;
    assert_ne!(rotated, first);
    assert_eq!(f.source.validate_session(&login.token).await.unwrap(), None);
    assert!(matches!(
        f.source
            .login(&username(), PASSWORD, &next_otp(&account.bootstrap.secret))
            .await,
        Err(DurableAuthError::InvalidCredentials)
    ));
    let fresh = f
        .source
        .login(
            &username(),
            NEW_PASSWORD,
            &next_otp(&account.bootstrap.secret),
        )
        .await
        .unwrap();
    assert_eq!(fresh.session.account, account.account);
    f.cleanup().await;

    let _guard = bootstrap_guard().await;
    let f = Fixture::new().await;
    let boot = f
        .source
        .bootstrap_empty_authority(scope(), &username(), PASSWORD)
        .await
        .unwrap();
    assert_writer_profile(&f).await;
    assert_eq!(
        f.count("sessions").await,
        0,
        "bootstrap must not issue a Session"
    );
    let login = f
        .source
        .login(
            &username(),
            PASSWORD,
            &otp(&boot.registration.bootstrap.secret),
        )
        .await
        .unwrap();
    assert_eq!(login.session.account, boot.registration.account);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_profile_registration_rechecks_trigger_changed_profile_before_commit() {
    let f = Fixture::ready().await;
    fault(
        &f,
        "users",
        "BEFORE INSERT",
        "NEW.password_hash = replace(NEW.password_hash, 'm=19456', 'm=8'); RETURN NEW;",
    )
    .await;
    let before = snapshot(&f, false).await;
    assert!(matches!(
        f.source.register(&username(), PASSWORD).await,
        Err(DurableAuthError::InvalidRecord)
    ));
    assert_fault_fired(&f).await;
    assert_eq!(snapshot(&f, false).await, before);
    assert_eq!(f.count("users").await, 0);
    assert_eq!(f.count("sessions").await, 0);
    f.sql("DROP TRIGGER profile_fault ON users").await;
    f.source.register(&username(), PASSWORD).await.unwrap();
    assert_writer_profile(&f).await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_profile_rotation_rechecks_post_revoke_profile_and_rolls_back() {
    let f = Fixture::ready().await;
    let (account, login) = f.seed_account_session().await;
    fault(
        &f,
        "sessions",
        "AFTER DELETE",
        "UPDATE users SET password_hash = replace(password_hash, 'm=19456', 'm=8'); RETURN OLD;",
    )
    .await;
    let before = snapshot(&f, false).await;
    assert_eq!(
        f.source
            .change_session_password(
                &login.token,
                PASSWORD,
                NEW_PASSWORD,
                &otp(&account.bootstrap.secret)
            )
            .await,
        Err(DurableAuthError::InvalidRecord)
    );
    assert_fault_fired(&f).await;
    assert_eq!(snapshot(&f, false).await, before);
    assert_eq!(
        f.source.validate_session(&login.token).await.unwrap(),
        Some(login.session)
    );
    assert_eq!(f.count("sessions").await, 1);
    assert_writer_profile(&f).await;
    f.cleanup().await;
}
