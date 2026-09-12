use super::*;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    response::Response,
    Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

#[derive(Clone, Copy)]
enum Mutation {
    Logout,
    Password,
    Delete,
}

impl Mutation {
    fn path(self) -> &'static str {
        match self {
            Self::Logout => "/auth/logout",
            Self::Password => "/auth/password/change",
            Self::Delete => "/auth/delete",
        }
    }
    fn body(self, otp: &str) -> Value {
        match self {
            Self::Logout => json!({}),
            Self::Password => {
                json!({"current_password": "synthetic-password", "new_password": "changed-synthetic-password", "otp": otp})
            }
            Self::Delete => json!({"password": "synthetic-password", "otp": otp}),
        }
    }
}

fn router_for(auth: &AuthService) -> Router {
    // Other services have no DATABASE_URL in this test process and no work is dispatched to them.
    let mut state = (*crate::build_state()).clone();
    state.auth_service = auth.clone();
    crate::build_router(Arc::new(state))
}

async fn mutate(app: &Router, mutation: Mutation, token: &str, otp: &str) -> Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(mutation.path())
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::COOKIE, format!("cyanrex_session={token}"))
                .body(Body::from(mutation.body(otp).to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn assert_write_failure(mutation: Mutation, skipped_row: bool) {
    let (admin, pool, schema) = super::postgres_tests::disposable_database().await;
    let mut auth = AuthService::new_with_default_teacher();
    auth.db_pool = Some(pool.clone());
    let registered = auth
        .register("mutation-student", "synthetic-password")
        .await
        .unwrap();
    let otp = compute_current_totp_code(&registered.secret);
    let login = auth
        .login("mutation-student", "synthetic-password", &otp)
        .await
        .unwrap();
    let original_hash = auth.users.read().unwrap()["mutation-student"]
        .password_hash
        .clone();
    let app = router_for(&auth);
    let (table, event) = match mutation {
        Mutation::Logout => ("sessions", "DELETE"),
        Mutation::Password => ("users", "UPDATE"),
        Mutation::Delete => ("users", "DELETE"),
    };
    // Fixed synthetic triggers in a generated, disposable schema only.
    let action = if skipped_row {
        "RETURN NULL;"
    } else {
        "RAISE EXCEPTION 'synthetic auth write failure';"
    };
    sqlx::query(&format!("CREATE FUNCTION reject_mutation() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN {action} END; $$"))
        .execute(&pool).await.unwrap();
    sqlx::query(&format!("CREATE TRIGGER reject_mutation BEFORE {event} ON {table} FOR EACH ROW EXECUTE FUNCTION reject_mutation()"))
        .execute(&pool).await.unwrap();

    let response = mutate(&app, mutation, &login.token, &otp).await;
    let status = response.status();
    let cleared_cookie = response.headers().contains_key(header::SET_COOKIE);
    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let saved: String =
        sqlx::query("SELECT password_hash FROM users WHERE username = 'mutation-student'")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get("password_hash");
    let stored_session: bool =
        sqlx::query("SELECT EXISTS (SELECT 1 FROM sessions WHERE token = $1) AS present")
            .bind(hash_session_token(&login.token))
            .fetch_one(&pool)
            .await
            .unwrap()
            .get("present");
    let cached_hash = auth
        .users
        .read()
        .unwrap()
        .get("mutation-student")
        .map(|user| user.password_hash.clone());
    let cached_session = auth.sessions.read().unwrap().contains_key(&login.token);

    // Once storage is healthy, an explicit retry must affect the real database, not just memory.
    sqlx::query(&format!("DROP TRIGGER reject_mutation ON {table}"))
        .execute(&pool)
        .await
        .unwrap();
    let retry = mutate(&app, mutation, &login.token, &otp).await;
    let retry_status = retry.status();
    let persisted = match mutation {
        Mutation::Password => {
            let row = sqlx::query("SELECT password_salt, password_hash FROM users WHERE username = 'mutation-student'")
                .fetch_one(&pool).await.unwrap();
            verify_password(
                "changed-synthetic-password",
                &row.get::<String, _>("password_salt"),
                &row.get::<String, _>("password_hash"),
            )
        }
        Mutation::Logout | Mutation::Delete => {
            let remaining: i64 =
                sqlx::query("SELECT count(*) AS count FROM sessions WHERE token = $1")
                    .bind(hash_session_token(&login.token))
                    .fetch_one(&pool)
                    .await
                    .unwrap()
                    .get("count");
            let user_count: i64 = sqlx::query(
                "SELECT count(*) AS count FROM users WHERE username = 'mutation-student'",
            )
            .fetch_one(&pool)
            .await
            .unwrap()
            .get("count");
            remaining == 0 && (matches!(mutation, Mutation::Logout) || user_count == 0)
        }
    };
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    assert_eq!(
        status,
        if skipped_row && !matches!(mutation, Mutation::Logout) {
            StatusCode::UNAUTHORIZED
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        }
    );
    assert_eq!(body["ok"], false);
    assert!(!body.to_string().contains("synthetic auth write failure"));
    assert!(
        !cleared_cookie,
        "failed mutations must not clear the cookie needed for an explicit retry"
    );
    assert_eq!(saved, original_hash);
    assert_eq!(cached_hash.as_deref(), Some(original_hash.as_str()));
    assert!(
        stored_session && cached_session,
        "failed account deletion must roll back session deletion too"
    );
    assert_eq!(retry_status, StatusCode::OK);
    assert!(
        persisted,
        "successful retries must survive an Engine restart"
    );
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_logout_write_failure_is_not_success() {
    assert_write_failure(Mutation::Logout, false).await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_logout_zero_row_deletion_is_not_success() {
    assert_write_failure(Mutation::Logout, true).await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_write_failure_is_not_success() {
    assert_write_failure(Mutation::Password, false).await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_account_deletion_failure_is_atomic() {
    assert_write_failure(Mutation::Delete, false).await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_zero_row_update_is_not_success() {
    assert_write_failure(Mutation::Password, true).await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_account_zero_row_deletion_is_not_success() {
    assert_write_failure(Mutation::Delete, true).await;
}

#[tokio::test]
async fn configured_auth_storage_outage_cannot_turn_mutations_into_memory_success() {
    for disabled in [false, true] {
        let mut auth = AuthService::new_with_default_teacher();
        auth.db_pool = None;
        let registered = auth
            .register("offline-mutation", "synthetic-password")
            .await
            .unwrap();
        let otp = compute_current_totp_code(&registered.secret);
        let login = auth
            .login("offline-mutation", "synthetic-password", &otp)
            .await
            .unwrap();
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://synthetic:synthetic@127.0.0.1:1/unreachable")
            .unwrap();
        pool.close().await;
        auth.db_pool = Some(pool);
        auth.db_disabled.store(disabled, Ordering::Relaxed);
        let app = router_for(&auth);
        for mutation in [Mutation::Password, Mutation::Logout, Mutation::Delete] {
            let response = mutate(&app, mutation, &login.token, &otp).await;
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert!(!response.headers().contains_key(header::SET_COOKIE));
            assert!(auth.sessions.read().unwrap().contains_key(&login.token));
            assert!(auth.users.read().unwrap().contains_key("offline-mutation"));
        }
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_account_mutations_normalize_identity_and_persist() {
    let (admin, pool, schema) = super::postgres_tests::disposable_database().await;
    let mut auth = AuthService::new_with_default_teacher();
    auth.db_pool = Some(pool.clone());
    let registered = auth
        .register("mixed-student", "synthetic-password")
        .await
        .unwrap();
    let otp = compute_current_totp_code(&registered.secret);
    auth.change_password(
        " MIXED-STUDENT ",
        "synthetic-password",
        "changed-synthetic-password",
        &otp,
    )
    .await
    .unwrap();
    let mut reloaded = AuthService::new_with_default_teacher();
    reloaded.db_pool = Some(pool.clone());
    let login = reloaded
        .login("mixed-student", "changed-synthetic-password", &otp)
        .await;
    let deletion = auth
        .delete_account(" MIXED-STUDENT ", "changed-synthetic-password", &otp)
        .await;
    let remaining: i64 =
        sqlx::query("SELECT count(*) AS count FROM users WHERE username = 'mixed-student'")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get("count");
    let session_removed = if let Ok(login) = &login {
        reloaded.validate_session(&login.token).await.is_none()
    } else {
        false
    };
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    assert!(
        login.is_ok(),
        "normalized password change must persist, not merely report success"
    );
    assert!(deletion.is_ok());
    assert_eq!(remaining, 0);
    assert!(session_removed);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_concurrent_password_changes_cannot_overwrite_newer_credentials() {
    let (admin, pool, schema) = super::postgres_tests::disposable_database().await;
    let mut auth = AuthService::new_with_default_teacher();
    auth.db_pool = Some(pool.clone());
    let registered = auth
        .register("concurrent-change", "synthetic-password")
        .await
        .unwrap();
    let otp = compute_current_totp_code(&registered.secret);
    let mut peer = AuthService::new_with_default_teacher();
    peer.db_pool = Some(pool.clone());
    peer.ensure_schema_and_seed().await.unwrap();
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("SELECT username FROM users WHERE username = 'concurrent-change' FOR UPDATE")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let first_auth = auth.clone();
    let first_otp = otp.clone();
    let first = tokio::spawn(async move {
        first_auth
            .change_password(
                "concurrent-change",
                "synthetic-password",
                "first-synthetic-change",
                &first_otp,
            )
            .await
    });
    let second_otp = otp.clone();
    let second = tokio::spawn(async move {
        peer.change_password(
            "concurrent-change",
            "synthetic-password",
            "second-synthetic-change",
            &second_otp,
        )
        .await
    });
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(8);
    let both_waiting = loop {
        let count: i64 = sqlx::query("SELECT count(*) AS count FROM pg_catalog.pg_stat_activity WHERE application_name = $1 AND wait_event_type = 'Lock'")
            .bind(&schema).fetch_one(&admin).await.unwrap().get("count");
        if count >= 2 {
            break true;
        }
        if tokio::time::Instant::now() >= deadline {
            break false;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    };
    blocker.commit().await.unwrap();
    let first = first.await.unwrap();
    let second = second.await.unwrap();
    let winner_password = if first.is_ok() {
        "first-synthetic-change"
    } else {
        "second-synthetic-change"
    };
    let winner_logs_in = auth
        .login("concurrent-change", winner_password, &otp)
        .await
        .is_ok();
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    assert!(
        both_waiting,
        "both requests must verify old credentials before either write completes"
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert!(
        matches!(first, Err(AuthError::InvalidCredentials))
            || matches!(second, Err(AuthError::InvalidCredentials))
    );
    assert!(winner_logs_in);
}
