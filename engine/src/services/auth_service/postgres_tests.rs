use super::*;

pub(super) async fn disposable_database() -> (PgPool, PgPool, String) {
    let url =
        std::env::var("CYANREX_TEST_DATABASE_URL").expect("disposable test database URL required");
    let admin = PgPoolOptions::new().connect(&url).await.unwrap();
    let schema = format!("auth_registration_{}", Uuid::new_v4().simple());
    // Test-generated identifiers only. No public tables or another instance's data are changed.
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let connection_schema = schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .after_connect(move |connection, _| {
            let schema = connection_schema.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path', $1, false), set_config('application_name', $1, false)")
                    .bind(schema)
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    (admin, pool, schema)
}

#[tokio::test]
#[ignore = "requires CYANREX_TEST_DATABASE_URL pointing to a disposable PostgreSQL database"]
async fn postgres_registration_persists_accounts_and_serializes_duplicates() {
    let (admin, pool, schema) = disposable_database().await;
    let mut auth = AuthService::new_with_default_teacher();
    auth.db_pool = Some(pool.clone());
    assert!(
        auth.active_pool().is_some(),
        "CYANREX_DB_FALLBACK must be enabled"
    );
    let registered = auth
        .register("persisted-student", "synthetic-password")
        .await
        .unwrap();
    assert!(!auth.db_disabled.load(Ordering::Relaxed));
    let stored = sqlx::query("SELECT password_hash, totp_secret FROM users WHERE username = $1")
        .bind("persisted-student")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(stored
        .get::<String, _>("password_hash")
        .starts_with("$argon2"));
    assert_eq!(stored.get::<String, _>("totp_secret"), registered.secret);

    let mut reloaded = AuthService::new_with_default_teacher();
    reloaded.db_pool = Some(pool.clone());
    assert!(!reloaded
        .users
        .read()
        .unwrap()
        .contains_key("persisted-student"));
    let parsed_uri = reqwest::Url::parse(&registered.otpauth_uri).unwrap();
    let uri_secret = parsed_uri
        .query_pairs()
        .find(|(name, _)| name == "secret")
        .unwrap()
        .1;
    let otp = compute_current_totp_code(&uri_secret);
    let login = reloaded
        .login("persisted-student", "synthetic-password", &otp)
        .await
        .unwrap();
    assert_eq!(
        reloaded
            .validate_session(&login.token)
            .await
            .unwrap()
            .username,
        "persisted-student"
    );
    assert_eq!(
        reloaded.role_for_username("persisted-student"),
        AuthRole::Student
    );
    let stored_token: String = sqlx::query("SELECT token FROM sessions WHERE username = $1")
        .bind("persisted-student")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("token");
    assert_eq!(stored_token, hash_session_token(&login.token));
    assert_ne!(stored_token, login.token);

    let (first, second) = tokio::join!(
        auth.register("concurrent-student", "first-synthetic-password"),
        reloaded.register(" CONCURRENT-STUDENT ", "second-synthetic-password")
    );
    let (created, duplicate, password) = if first.is_ok() {
        (first, second, "first-synthetic-password")
    } else {
        (second, first, "second-synthetic-password")
    };
    let created = created.unwrap();
    assert!(matches!(duplicate, Err(AuthError::UserAlreadyExists)));
    let count: i64 = sqlx::query("SELECT count(*) AS count FROM users WHERE username = $1")
        .bind("concurrent-student")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("count");
    assert_eq!(count, 1);
    let otp = compute_current_totp_code(&created.secret);
    assert!(reloaded
        .login("concurrent-student", password, &otp)
        .await
        .is_ok());
    assert!(!auth.db_disabled.load(Ordering::Relaxed));
    assert!(!reloaded.db_disabled.load(Ordering::Relaxed));

    // An insert failure after successful initialization must use the same real memory publication.
    pool.close().await;
    let offline = auth
        .register("later-offline-student", "offline-synthetic-password")
        .await
        .unwrap();
    assert!(auth.db_disabled.load(Ordering::Relaxed));
    let otp = compute_current_totp_code(&offline.secret);
    assert!(auth
        .login("later-offline-student", "offline-synthetic-password", &otp)
        .await
        .is_ok());
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}

#[derive(Clone, Copy)]
enum Invalidation {
    Logout,
    Expiry,
    DeleteAccount,
}

async fn assert_invalidation_survives_fallback(invalidation: Invalidation) {
    let (admin, pool, schema) = disposable_database().await;
    let mut auth = AuthService::new_with_default_teacher();
    auth.db_pool = Some(pool.clone());
    assert!(auth.active_pool().is_some());
    let registered = auth
        .register("cached-student", "synthetic-password")
        .await
        .unwrap();
    let otp = compute_current_totp_code(&registered.secret);
    let session = auth
        .login("cached-student", "synthetic-password", &otp)
        .await
        .unwrap();
    let other = auth
        .register("other-student", "other-synthetic-password")
        .await
        .unwrap();
    let other_session = auth
        .login(
            "other-student",
            "other-synthetic-password",
            &compute_current_totp_code(&other.secret),
        )
        .await
        .unwrap();
    let mut peer = AuthService::new_with_default_teacher();
    peer.db_pool = Some(pool.clone());

    match invalidation {
        Invalidation::Logout => {
            peer.logout(&session.token).await.unwrap();
            peer.logout(&session.token).await.unwrap();
        }
        Invalidation::Expiry => {
            sqlx::query(
                "UPDATE sessions SET expires_at = NOW() - INTERVAL '1 second' WHERE token = $1",
            )
            .bind(hash_session_token(&session.token))
            .execute(&pool)
            .await
            .unwrap();
        }
        Invalidation::DeleteAccount => {
            peer.delete_account("cached-student", "synthetic-password", &otp)
                .await
                .unwrap();
        }
    }
    if matches!(invalidation, Invalidation::DeleteAccount) {
        // Observe the authoritative absence through account lookup, not session validation.
        assert!(auth.get_user(" CACHED-STUDENT ").await.is_none());
    } else {
        assert!(auth.validate_session(&session.token).await.is_none());
    }
    assert!(!auth.db_disabled.load(Ordering::Relaxed));
    pool.close().await;
    let revived_session = auth.validate_session(&session.token).await.is_some();
    let revived_account = matches!(invalidation, Invalidation::DeleteAccount)
        && auth
            .login("cached-student", "synthetic-password", &otp)
            .await
            .is_ok();
    let unrelated_session_valid = auth.validate_session(&other_session.token).await.is_some();
    // Clean this test's generated schema even when the behavior assertions below fail.
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    assert!(
        !revived_session,
        "an observed invalidation must not revive from the memory fallback"
    );
    assert!(
        !revived_account,
        "an observed deleted account must not authenticate from stale memory"
    );
    assert!(
        unrelated_session_valid,
        "invalidation must remain scoped to its token/account"
    );
}

#[tokio::test]
#[ignore = "requires CYANREX_TEST_DATABASE_URL pointing to a disposable PostgreSQL database"]
async fn postgres_revoked_session_cannot_revive_in_fallback() {
    assert_invalidation_survives_fallback(Invalidation::Logout).await;
}

#[tokio::test]
#[ignore = "requires CYANREX_TEST_DATABASE_URL pointing to a disposable PostgreSQL database"]
async fn postgres_expired_session_cannot_revive_in_fallback() {
    assert_invalidation_survives_fallback(Invalidation::Expiry).await;
}

#[tokio::test]
#[ignore = "requires CYANREX_TEST_DATABASE_URL pointing to a disposable PostgreSQL database"]
async fn postgres_deleted_account_cannot_revive_in_fallback() {
    assert_invalidation_survives_fallback(Invalidation::DeleteAccount).await;
}
