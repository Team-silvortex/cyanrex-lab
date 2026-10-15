use super::fixture::*;
use axum::http::{HeaderValue, StatusCode};
use cyanrex_engine::platform_http::{build_session_lifecycle_router, SessionLifecycleHttpState};
use serde_json::json;
use sqlx_core::query::query;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_http_reads_current_session_and_confirms_exact_logout_without_touching_other_sessions(
) {
    let (
        before,
        read_only,
        after,
        first,
        logout,
        rejected,
        other,
        missing_logout,
        other_valid,
        current_absent,
    ) = Fixture::checked(|f| {
        Box::pin(async move {
            let before = f.snapshot().await;
            let mut selected = request("GET", SESSION, &f.token);
            let mut cookie = HeaderValue::from_str(&format!(
                "{COOKIE}={}; cyanrex_session={}",
                f.token, f.other_token
            ))
            .unwrap();
            cookie.set_sensitive(true);
            selected.headers_mut().insert("cookie", cookie);
            let first = capture(f.app(), selected).await;
            let read_only = f.snapshot().await;
            let logout = capture(f.app(), request("POST", LOGOUT, &f.token)).await;
            let rejected = capture(f.app(), request("GET", SESSION, &f.token)).await;
            let other = capture(f.app(), request("GET", SESSION, &f.other_token)).await;
            let missing_logout = capture(
                f.app(),
                request("POST", LOGOUT, &Uuid::new_v4().to_string()),
            )
            .await;
            let after = f.snapshot().await;
            let other_valid = within(f.source().validate_session(&f.other_token))
                .await
                .unwrap_or_else(|_| panic!("other Session validation failed"))
                .is_some();
            let current_absent = within(f.source().validate_session(&f.token))
                .await
                .unwrap_or_else(|_| panic!("revoked Session validation failed"))
                .is_none();
            (
                before,
                read_only,
                after,
                first,
                logout,
                rejected,
                other,
                missing_logout,
                other_valid,
                current_absent,
            )
        })
    })
    .await;

    // Behavior expectations follow checked namespace/pool cleanup, including an inert scaffold Red.
    successful(
        &first,
        json!({"schema_version":1,"authenticated":true}),
        false,
    );
    successful(&logout, json!({"schema_version":1,"ok":true}), true);
    denied(
        &rejected,
        StatusCode::UNAUTHORIZED,
        "invalid_session",
        "unconfirmed",
    );
    successful(
        &other,
        json!({"schema_version":1,"authenticated":true}),
        false,
    );
    successful(&missing_logout, json!({"schema_version":1,"ok":true}), true);
    assert!(
        before.users == read_only.users
            && before.source == read_only.source
            && before.sessions == read_only.sessions,
        "GET mutated the Source snapshot"
    );
    assert!(
        before.users == after.users && before.source == after.source,
        "logout changed an account or Source authority"
    );
    assert_eq!(before.session_count, 2);
    assert_eq!(after.session_count, 1);
    assert!(
        before.registry_absent && after.registry_absent,
        "source-only HTTP implicitly installed a collaboration registry"
    );
    assert!(
        other_valid && current_absent,
        "logout did not revoke exactly the selected Session"
    );
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_http_expired_revoked_and_recreated_accounts_never_authenticate_old_cookies(
) {
    for mode in ["expired", "revoked", "recreated"] {
        let (reply, before, after, incarnation_changed, invalid) = Fixture::checked(|f| Box::pin(async move {
            let mut incarnation_changed = true;
            match mode {
                "expired" => {
                    within(query("UPDATE sessions SET expires_at = clock_timestamp() - INTERVAL '1 second' WHERE token = $1")
                        .bind(token_hash(&f.token)).execute(f.pool())).await
                        .unwrap_or_else(|_| panic!("synthetic expiration setup failed"));
                }
                "revoked" => {
                    within(query("DELETE FROM sessions WHERE token = $1")
                        .bind(token_hash(&f.token)).execute(f.pool())).await
                        .unwrap_or_else(|_| panic!("synthetic revocation setup failed"));
                }
                "recreated" => {
                    within(query("DELETE FROM users WHERE username = $1")
                        .bind(f.username.as_str()).execute(f.pool())).await
                        .unwrap_or_else(|_| panic!("synthetic account deletion failed"));
                    let registration = within(f.source().register(&f.username, PASSWORD)).await
                        .unwrap_or_else(|_| panic!("same-name synthetic registration failed"));
                    incarnation_changed = registration.account.account_id.as_uuid() != f.account_id;
                }
                _ => unreachable!(),
            }
            let before = f.snapshot().await;
            let reply = capture(f.app(), request("GET", SESSION, &f.token)).await;
            let invalid = within(f.source().validate_session(&f.token)).await
                .unwrap_or_else(|_| panic!("invalid Session control failed")).is_none();
            let after = f.snapshot().await;
            (reply, before, after, incarnation_changed, invalid)
        })).await;
        denied(
            &reply,
            StatusCode::UNAUTHORIZED,
            "invalid_session",
            "unconfirmed",
        );
        assert!(
            incarnation_changed && invalid,
            "an old cookie acquired a recreated account"
        );
        assert!(
            unchanged(&before, &after),
            "GET repaired or deleted invalid Session state"
        );
        assert!(after.registry_absent);
    }
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_http_loopback_logout_clears_only_the_host_platform_cookie_without_secure()
{
    let (first, logout, before, after, current_absent, other_valid) = Fixture::checked(|f| {
        Box::pin(async move {
            let origin = "http://127.0.0.1:34123";
            let app = build_session_lifecycle_router(
                SessionLifecycleHttpState::new(f.source().clone(), origin)
                    .unwrap_or_else(|_| panic!("explicit loopback origin rejected")),
            );
            let before = f.snapshot().await;
            let mut read = request("GET", SESSION, &f.token);
            read.headers_mut().remove("origin");
            let first = capture(app.clone(), read).await;
            let mut post = request("POST", LOGOUT, &f.token);
            post.headers_mut()
                .insert("origin", HeaderValue::from_static(origin));
            let logout = capture(app, post).await;
            let after = f.snapshot().await;
            let current_absent = within(f.source().validate_session(&f.token))
                .await
                .unwrap_or_else(|_| panic!("loopback logout control failed"))
                .is_none();
            let other_valid = within(f.source().validate_session(&f.other_token))
                .await
                .unwrap_or_else(|_| panic!("second device control failed"))
                .is_some();
            (first, logout, before, after, current_absent, other_valid)
        })
    })
    .await;
    successful(
        &first,
        json!({"schema_version":1,"authenticated":true}),
        false,
    );
    assert_eq!(logout.status, StatusCode::OK);
    common_headers(&logout);
    assert!(logout.value == json!({"schema_version":1,"ok":true}));
    assert_eq!(logout.headers.get_all("set-cookie").iter().count(), 1);
    assert_eq!(
        logout.headers.get("set-cookie").unwrap(),
        "cyanrex_platform_session=; Path=/platform/v1; HttpOnly; SameSite=Strict; Max-Age=0"
    );
    assert!(before.users == after.users && before.source == after.source);
    assert_eq!(before.session_count, 2);
    assert_eq!(after.session_count, 1);
    assert!(current_absent && other_valid && after.registry_absent);
}
