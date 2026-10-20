use super::fixture::*;
use axum::{body::Body, http::StatusCode};
use cyanrex_engine::platform_http::build_platform_session_router;
use tower::ServiceExt;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_session_router_clones_share_login_rate_without_blocking_lifecycle() {
    let (
        issued,
        attempts_shared,
        global_shared,
        unchanged,
        authenticated,
        logged_out,
        revoked,
        no_backend,
        cleaned,
    ) = Fixture::checked(|f| {
        Box::pin(async move {
            let prepared = Prepared::new(f).await;
            let state = prepared.state(4, 4).await;
            let first = build_platform_session_router(state.clone());
            let second = build_platform_session_router(state.clone());
            let response = capture(first.clone(), valid_request()).await;
            let token = issued_token(&response);
            let issued = token.is_some();
            let token = token.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let before = snapshot(f).await;
            let mut attempts_shared = true;
            // One connector and its account-attempt map must be shared even by separately built routers.
            for index in 0..6 {
                let response = capture(
                    if index % 2 == 0 {
                        first.clone()
                    } else {
                        second.clone()
                    },
                    login_request(username().as_str(), "synthetic-wrong-password", &otp()),
                )
                .await;
                attempts_shared &= if index < 5 {
                    denied(
                        &response,
                        StatusCode::UNAUTHORIZED,
                        "invalid_credentials",
                        "unconfirmed",
                    )
                } else {
                    denied(
                        &response,
                        StatusCode::TOO_MANY_REQUESTS,
                        "rate_limited",
                        "unconfirmed",
                    )
                };
            }
            // 1 successful + 6 dispatched attempts + 57 transport-invalid bodies exhaust exactly64.
            let mut global_shared = true;
            for index in 0..57 {
                let response = capture(
                    if index % 2 == 0 {
                        first.clone()
                    } else {
                        second.clone()
                    },
                    raw(Body::from("{}")),
                )
                .await;
                global_shared &= denied(
                    &response,
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    "not_attempted",
                );
            }
            let response = capture(second.clone(), raw(untouched_body())).await;
            global_shared &= denied(
                &response,
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "not_attempted",
            );
            let unchanged = before == snapshot(f).await;
            let authenticated = current(&capture(second.clone(), get(&token)).await);
            let logged_out = cleared(&capture(first.clone(), logout(&token)).await);
            let response = capture(second.clone(), get(&token)).await;
            let revoked = denied(
                &response,
                StatusCode::UNAUTHORIZED,
                "invalid_session",
                "unconfirmed",
            ) && session_count(f).await == 0;
            drop(first);
            drop(second);
            drop(state);
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish();
            (
                issued,
                attempts_shared,
                global_shared,
                unchanged,
                authenticated,
                logged_out,
                revoked,
                no_backend,
                cleaned,
            )
        })
    })
    .await;
    assert!(
        issued
            && attempts_shared
            && global_shared
            && unchanged
            && authenticated
            && logged_out
            && revoked
            && no_backend
            && cleaned
    );
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_session_router_response_capacity_is_shared_per_lane_and_released_independently(
) {
    let (
        issued,
        login_full,
        life_full,
        get_completed,
        logout_reachable,
        login_completed,
        reverse_independent,
        released,
        count,
        no_backend,
        cleaned,
    ) = Fixture::checked(|f| {
        Box::pin(async move {
            let prepared = Prepared::new(f).await;
            let state = prepared.state(1, 1).await;
            let first = build_platform_session_router(state.clone());
            let second = build_platform_session_router(state.clone());
            let held = within(first.clone().oneshot(valid_request()))
                .await
                .unwrap();
            let token = header_token(&held);
            let issued = token.is_some();
            let token = token.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let response = capture(second.clone(), raw(untouched_body())).await;
            let login_full = denied(
                &response,
                StatusCode::SERVICE_UNAVAILABLE,
                "capacity_exhausted",
                "not_attempted",
            );
            let held_get = within(second.clone().oneshot(get(&token))).await.unwrap();
            let response = capture(
                first.clone(),
                lifecycle("GET", SESSION, &token, untouched_body()),
            )
            .await;
            let life_full = denied(
                &response,
                StatusCode::SERVICE_UNAVAILABLE,
                "capacity_exhausted",
                "not_attempted",
            );
            let get_completed = current(&capture_response(held_get).await);
            // Login's successful body is still unpolled, but its new token can already be logged out.
            let logout_reachable = cleared(&capture(second.clone(), logout(&token)).await);
            let login_completed = issued_token(&capture_response(held).await).is_some();
            let held_error = within(first.clone().oneshot(get(&token))).await.unwrap();
            let response = capture(
                second.clone(),
                lifecycle("POST", LOGOUT, &token, untouched_body()),
            )
            .await;
            let mut reverse_independent = denied(
                &response,
                StatusCode::SERVICE_UNAVAILABLE,
                "capacity_exhausted",
                "not_attempted",
            );
            let response = capture(
                second.clone(),
                login_request(username().as_str(), "synthetic-wrong-password", &otp()),
            )
            .await;
            reverse_independent &= denied(
                &response,
                StatusCode::UNAUTHORIZED,
                "invalid_credentials",
                "unconfirmed",
            );
            drop(held_error);
            let response = capture(first.clone(), get(&token)).await;
            let released = denied(
                &response,
                StatusCode::UNAUTHORIZED,
                "invalid_session",
                "unconfirmed",
            );
            let count = session_count(f).await;
            drop(first);
            drop(second);
            drop(state);
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish();
            (
                issued,
                login_full,
                life_full,
                get_completed,
                logout_reachable,
                login_completed,
                reverse_independent,
                released,
                count,
                no_backend,
                cleaned,
            )
        })
    })
    .await;
    assert!(
        issued
            && login_full
            && life_full
            && get_completed
            && logout_reachable
            && login_completed
            && reverse_independent
            && released
            && no_backend
            && cleaned
    );
    assert_eq!(count, 0);
}
