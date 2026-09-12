//! Opt-in authentication race, cancellation and durable-write regression tests.
//! Use only a disposable CYANREX_TEST_DATABASE_URL; no production hooks or credentials are used.
use super::*;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

const USER: &str = "boundary-student";
const PASSWORD: &str = "synthetic-boundary-password";

async fn paused_login_is_not_published(cancel: bool) {
    let f = Fixture::new().await;
    let key = (Uuid::new_v4().as_u128() & i64::MAX as u128) as i64;
    let mut blocker = f.pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(key)
        .execute(&mut *blocker)
        .await
        .unwrap();
    f.sql(&format!("CREATE FUNCTION pause_session() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock({key}); RETURN NEW; END; $$")).await;
    f.sql("CREATE TRIGGER pause_session BEFORE INSERT ON sessions FOR EACH ROW EXECUTE FUNCTION pause_session()").await;
    let (auth, otp) = (f.auth.clone(), f.otp.clone());
    let login = tokio::spawn(async move { auth.login(USER, PASSWORD, &otp).await });
    let blocked = f.wait_blocked("INSERT INTO sessions").await;
    let cached_before_commit = f.auth.sessions.read().unwrap().len();
    let changed = if cancel {
        login.abort();
        true
    } else {
        matches!(
            tokio::time::timeout(
                std::time::Duration::from_secs(8),
                f.auth
                    .change_password(USER, PASSWORD, "changed-synthetic-password", &f.otp)
            )
            .await,
            Ok(Ok(()))
        )
    };
    blocker.commit().await.unwrap();
    let rejected = match login.await {
        Err(error) => cancel && error.is_cancelled(),
        Ok(Err(AuthError::InvalidCredentials)) => !cancel,
        _ => false,
    };
    f.drain_pool().await;
    let snapshot = f.snapshot().await;
    let disabled = f.auth.db_disabled.load(Ordering::Relaxed);
    let cached_after = f.auth.sessions.read().unwrap().len();
    f.cleanup().await;
    assert!(blocked && changed && rejected);
    assert_eq!(
        cached_before_commit, 1,
        "pending login must never publish a cache token"
    );
    assert_eq!(cached_after, 1);
    assert_eq!(snapshot, (true, 1, true, true));
    assert!(!disabled);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_cancelled_login_rolls_back_without_publishing_a_session() {
    paused_login_is_not_published(true).await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_login_cannot_publish_after_concurrent_password_change() {
    paused_login_is_not_published(false).await;
}

struct Fixture {
    admin: PgPool,
    pool: PgPool,
    schema: String,
    auth: AuthService,
    otp: String,
    token: String,
}

impl Fixture {
    async fn new() -> Self {
        let (admin, pool, schema) = super::postgres_tests::disposable_database().await;
        let mut auth = AuthService::new_with_default_teacher();
        auth.db_pool = Some(pool.clone());
        assert!(auth.active_pool().is_some());
        let registered = auth.register(USER, PASSWORD).await.unwrap();
        let otp = compute_current_totp_code(&registered.secret);
        let token = auth.login(USER, PASSWORD, &otp).await.unwrap().token;
        Self {
            admin,
            pool,
            schema,
            auth,
            otp,
            token,
        }
    }

    async fn sql(&self, statement: &str) {
        sqlx::query(statement).execute(&self.pool).await.unwrap();
    }

    async fn snapshot(&self) -> (bool, i64, bool, bool) {
        let row = sqlx::query("SELECT EXISTS (SELECT 1 FROM users WHERE username = $1) AS present, (SELECT count(*) FROM sessions WHERE username = $1) AS sessions")
            .bind(USER).fetch_one(&self.pool).await.unwrap();
        (
            row.get("present"),
            row.get("sessions"),
            self.auth.users.read().unwrap().contains_key(USER),
            self.auth.sessions.read().unwrap().contains_key(&self.token),
        )
    }

    async fn wait_blocked(&self, prefix: &str) -> bool {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(8);
        loop {
            let count: i64 = sqlx::query("SELECT count(*) AS count FROM pg_catalog.pg_stat_activity WHERE application_name = $1 AND wait_event_type = 'Lock' AND query LIKE $2")
                .bind(&self.schema).bind(format!("{prefix}%"))
                .fetch_one(&self.admin).await.unwrap().get("count");
            if count > 0 {
                return true;
            }
            if tokio::time::Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    async fn drain_pool(&self) {
        // Hold every connection once: a cancelled query's deferred rollback/drain must finish.
        tokio::time::timeout(std::time::Duration::from_secs(8), async {
            let mut connections = Vec::new();
            for _ in 0..4 {
                let mut connection = self.pool.acquire().await.unwrap();
                sqlx::query("SELECT 1")
                    .execute(&mut *connection)
                    .await
                    .unwrap();
                connections.push(connection);
            }
        })
        .await
        .expect("cancelled database work must release its connection");
    }

    async fn cleanup(self) {
        self.pool.close().await;
        sqlx::query(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
    }
}

async fn delete_fault(trigger: &str, action: &str) {
    let f = Fixture::new().await;
    f.sql(&format!("CREATE FUNCTION boundary_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN {action} END; $$")).await;
    f.sql(trigger).await;
    let outcome = f.auth.delete_account(USER, PASSWORD, &f.otp).await;
    let snapshot = f.snapshot().await;
    let session_valid = f.auth.validate_session(&f.token).await.is_some();
    let cached_hash_unchanged = verify_password(
        PASSWORD,
        &f.auth
            .users
            .read()
            .unwrap()
            .get(USER)
            .map(|u| u.password_salt.clone())
            .unwrap_or_default(),
        &f.auth
            .users
            .read()
            .unwrap()
            .get(USER)
            .map(|u| u.password_hash.clone())
            .unwrap_or_default(),
    );
    // Cleanup before the behavioral assertions, including known-failing cases.
    f.cleanup().await;
    assert!(
        matches!(outcome, Err(AuthError::StorageUnavailable)),
        "a partial/failed deletion must not be acknowledged: {outcome:?}; snapshot={snapshot:?}; session_valid={session_valid}"
    );
    assert_eq!(snapshot, (true, 1, true, true));
    assert!(cached_hash_unchanged);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_delete_session_step_failure_preserves_account_and_session() {
    delete_fault("CREATE TRIGGER boundary_fault BEFORE DELETE ON sessions FOR EACH ROW EXECUTE FUNCTION boundary_fault()",
        "RAISE EXCEPTION 'synthetic session delete failure';").await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_delete_deferred_commit_failure_rolls_back_account_and_sessions() {
    delete_fault("CREATE CONSTRAINT TRIGGER boundary_fault AFTER DELETE ON users DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION boundary_fault()",
        "RAISE EXCEPTION 'synthetic deferred commit failure';").await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL; probes suppressed cascade deletion"]
async fn postgres_delete_suppressed_session_cascade_cannot_report_success() {
    delete_fault("CREATE TRIGGER boundary_fault BEFORE DELETE ON sessions FOR EACH ROW EXECUTE FUNCTION boundary_fault()",
        "RETURN NULL;").await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_cancelled_account_deletion_rolls_back_the_earlier_session_delete() {
    let f = Fixture::new().await;
    let mut blocker = f.pool.begin().await.unwrap();
    sqlx::query("SELECT username FROM users WHERE username = $1 FOR UPDATE")
        .bind(USER)
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let auth = f.auth.clone();
    let otp = f.otp.clone();
    let deletion = tokio::spawn(async move { auth.delete_account(USER, PASSWORD, &otp).await });
    let blocked = f.wait_blocked("DELETE FROM users").await;
    deletion.abort();
    let cancelled = deletion.await.unwrap_err().is_cancelled();
    blocker.commit().await.unwrap();
    f.drain_pool().await;
    let snapshot = f.snapshot().await;
    f.cleanup().await;
    assert!(
        blocked,
        "cancellation must occur after session deletion but before user deletion"
    );
    assert!(cancelled);
    assert_eq!(snapshot, (true, 1, true, true));
}

async fn login_races_account_deletion(recreate: bool) {
    let f = Fixture::new().await;
    let key = (Uuid::new_v4().as_u128() & i64::MAX as u128) as i64;
    let mut blocker = f.pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(key)
        .execute(&mut *blocker)
        .await
        .unwrap();
    f.sql(&format!("CREATE FUNCTION pause_session() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock({key}); RETURN NEW; END; $$")).await;
    f.sql("CREATE TRIGGER pause_session BEFORE INSERT ON sessions FOR EACH ROW EXECUTE FUNCTION pause_session()").await;
    let mut state = (*crate::build_state()).clone();
    state.auth_service = f.auth.clone();
    let app = crate::build_router(Arc::new(state));
    let otp = f.otp.clone();
    let login_app = app.clone();
    let login = tokio::spawn(async move {
        login_app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({"username": USER, "password": PASSWORD, "otp": otp}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap()
    });
    let blocked = f.wait_blocked("INSERT INTO sessions").await;
    let deletion = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        f.auth.delete_account(USER, PASSWORD, &f.otp),
    )
    .await;
    let deleted = matches!(deletion, Ok(Ok(())));
    if recreate && deleted {
        f.auth
            .register(USER, "replacement-synthetic-password")
            .await
            .unwrap();
    }
    blocker.commit().await.unwrap();
    let response = tokio::time::timeout(std::time::Duration::from_secs(8), login)
        .await
        .unwrap()
        .unwrap();
    let status = response.status();
    let cookie = response.headers().get(header::SET_COOKIE).cloned();
    let me = if let Some(cookie) = &cookie {
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/auth/me")
                    .header(header::COOKIE, cookie.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice::<Value>(&bytes).unwrap()["authenticated"] == true
    } else {
        false
    };
    let snapshot = f.snapshot().await;
    let disabled = f.auth.db_disabled.load(Ordering::Relaxed);
    f.cleanup().await;
    assert!(
        blocked && deleted,
        "must delete the account after login verifies its old credentials"
    );
    assert!(!status.is_success() && !me && cookie.is_none(),
        "stale login: status={status}, authenticated={me}, cookie={}, db_disabled={disabled}, recreated={recreate}, snapshot={snapshot:?}", cookie.is_some());
    assert!(
        !disabled,
        "account deletion is not a reason to disable persistence for every user"
    );
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL; probes login/deletion race"]
async fn postgres_login_cannot_report_success_after_its_account_was_deleted() {
    login_races_account_deletion(false).await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL; probes account-generation isolation"]
async fn postgres_old_login_cannot_authenticate_as_a_recreated_account() {
    login_races_account_deletion(true).await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL; probes zero-row session insertion"]
async fn postgres_login_zero_row_session_insert_cannot_report_success() {
    let f = Fixture::new().await;
    f.sql("CREATE FUNCTION skip_session() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NULL; END; $$").await;
    f.sql("CREATE TRIGGER skip_session BEFORE INSERT ON sessions FOR EACH ROW EXECUTE FUNCTION skip_session()").await;
    let login = f.auth.login(USER, PASSWORD, &f.otp).await;
    let valid = if let Ok(login) = &login {
        f.auth.validate_session(&login.token).await.is_some()
    } else {
        false
    };
    let succeeded = login.is_ok();
    let original_valid = f.auth.validate_session(&f.token).await.is_some();
    f.cleanup().await;
    assert!(original_valid);
    assert!(
        !succeeded,
        "zero-row session insertion: login_success={succeeded}, new_session_valid={valid}"
    );
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_cancelled_classroom_join_never_reopens_its_consumed_invitation() {
    use crate::services::classroom::ClassroomService;
    let f = Fixture::new().await;
    let classroom_id = "23d40d83-19de-43d3-85fe-506d86592a70";
    let classroom =
        ClassroomService::configured(classroom_id, "Synthetic Lab", "http://localhost:3000")
            .unwrap();
    let invitation = classroom.issue("joining-student").unwrap();
    let token = invitation
        .join_url
        .split("#invite=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap();
    let body = json!({"classroom_id": classroom_id, "protocol_version": 1, "client_version": "0.3.7",
        "required_capabilities": ["student-invite-v1"], "invite_token": token, "username": "joining-student", "password": PASSWORD});
    let key = (Uuid::new_v4().as_u128() & i64::MAX as u128) as i64;
    let mut blocker = f.pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(key)
        .execute(&mut *blocker)
        .await
        .unwrap();
    f.sql(&format!("CREATE FUNCTION pause_join() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock({key}); RETURN NEW; END; $$")).await;
    f.sql("CREATE TRIGGER pause_join BEFORE INSERT ON users FOR EACH ROW WHEN (NEW.username = 'joining-student') EXECUTE FUNCTION pause_join()").await;
    let mut state = (*crate::build_state()).clone();
    state.auth_service = f.auth.clone();
    state.classroom = classroom.clone();
    let app = crate::build_router(Arc::new(state));
    let join_request = || {
        Request::builder()
            .method("POST")
            .uri("/classroom/join")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ORIGIN, "http://localhost:3000")
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let request = join_request();
    let joining_app = app.clone();
    let joining = tokio::spawn(async move { joining_app.oneshot(request).await.unwrap() });
    let blocked = f.wait_blocked("INSERT INTO users").await;
    joining.abort();
    let cancelled = joining.await.unwrap_err().is_cancelled();
    blocker.commit().await.unwrap();
    f.drain_pool().await;
    let inventory_empty = classroom.inventory().unwrap().is_empty();
    let replay = app.oneshot(join_request()).await.unwrap();
    let replay_status = replay.status();
    let cached = f.auth.users.read().unwrap().contains_key("joining-student");
    let row = sqlx::query(
        "SELECT EXISTS (SELECT 1 FROM users WHERE username = 'joining-student') AS present",
    )
    .fetch_one(&f.pool)
    .await
    .unwrap();
    let persisted: bool = row.get("present");
    let unaffected = f.snapshot().await;
    f.cleanup().await;
    assert!(blocked && cancelled);
    assert!(inventory_empty && !cached);
    assert_eq!(replay_status, StatusCode::FORBIDDEN);
    assert_eq!(unaffected, (true, 1, true, true));
    // Cancellation is not a rollback guarantee for an already-dispatched autocommit INSERT.
    eprintln!(
        "cancelled enrollment: durable_account_present={persisted}, invitation_reusable=false"
    );
}
