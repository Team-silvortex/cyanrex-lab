use super::{
    fixture::*,
    login_http_helpers::{release, source_holder, waiting_on, Pending},
};
use axum::{
    http::StatusCode,
    middleware::{self, Next},
    Router,
};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;
use tower::ServiceExt;

#[derive(Clone, Copy)]
enum Operation {
    Login,
    Get,
    Logout,
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_session_router_cancelled_observed_sql_wait_drains_without_manufacturing_success(
) {
    for operation in [Operation::Login, Operation::Get, Operation::Logout] {
        let (
            issued,
            waited,
            cancelled,
            released,
            drained,
            unchanged,
            recovered,
            no_backend,
            cleaned,
        ) = Fixture::checked(|f| {
            Box::pin(async move {
                let prepared = Prepared::new(f).await;
                let state = prepared.state(1, 1).await;
                let app =
                    cyanrex_engine::platform_http::build_platform_session_router(state.clone());
                let (issued, token) = if matches!(operation, Operation::Login) {
                    (true, uuid::Uuid::new_v4().to_string())
                } else {
                    let token = issued_token(&capture(app.clone(), valid_request()).await);
                    (
                        token.is_some(),
                        token.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                    )
                };
                let before = snapshot(f).await;
                let (holder, pid) = source_holder(f).await;
                let req = match operation {
                    Operation::Login => valid_request(),
                    Operation::Get => get(&token),
                    Operation::Logout => logout(&token),
                };
                let owned = app.clone();
                let pending = Pending::start(async move { owned.oneshot(req).await });
                let waited = waiting_on(f, pid).await;
                let cancelled = pending.cancel().await;
                let released = release(holder).await;
                let drained = no_connections(f).await;
                let unchanged = before == snapshot(f).await;
                let recovered = if matches!(operation, Operation::Login) {
                    issued_token(&capture(app.clone(), valid_request()).await).is_some()
                        && session_count(f).await == 1
                } else {
                    current(&capture(app.clone(), get(&token)).await) && session_count(f).await == 1
                };
                drop(app);
                drop(state);
                let no_backend = no_connections(f).await;
                let cleaned = prepared.finish();
                (
                    issued, waited, cancelled, released, drained, unchanged, recovered, no_backend,
                    cleaned,
                )
            })
        })
        .await;
        assert!(
            issued
                && waited
                && cancelled
                && released
                && drained
                && unchanged
                && recovered
                && no_backend
                && cleaned
        );
    }
}

fn delivery_gate(app: Router, login: bool) -> (Router, oneshot::Receiver<bool>) {
    let (ready, receiver) = oneshot::channel();
    let ready = Arc::new(Mutex::new(Some(ready)));
    let gated = app.layer(middleware::from_fn(move |request, next: Next| {
        let ready = ready.clone();
        async move {
            let response = next.run(request).await;
            let confirmed = if login { header_token(&response).is_some() }
            else {
                response.status()==StatusCode::OK && response.headers().get("set-cookie").is_some_and(|h|h==
                    "cyanrex_platform_session=; Path=/platform/v1; HttpOnly; SameSite=Strict; Max-Age=0; Secure")
            };
            if let Some(ready)=ready.lock().unwrap().take() { let _ = ready.send(confirmed); }
            // Test-only outer transport pause after the original confirmed command. No headers
            // or body leave this gate; this is not a simulated missing database COMMIT ACK.
            std::future::pending::<()>().await;
            response
        }
    }));
    (gated, receiver)
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_session_router_cancelled_confirmed_delivery_preserves_committed_login_or_logout(
) {
    for login_delivery in [true, false] {
        let (
            issued,
            confirmed,
            committed,
            full,
            cancelled,
            unchanged,
            recovered,
            no_backend,
            cleaned,
        ) = Fixture::checked(|f| {
            Box::pin(async move {
                let prepared = Prepared::new(f).await;
                let state = prepared.state(1, 1).await;
                let app =
                    cyanrex_engine::platform_http::build_platform_session_router(state.clone());
                let code = otp();
                let (issued, token) = if login_delivery {
                    (true, uuid::Uuid::new_v4().to_string())
                } else {
                    let response = capture(
                        app.clone(),
                        login_request(username().as_str(), password(), &code),
                    )
                    .await;
                    let token = issued_token(&response);
                    (
                        token.is_some(),
                        token.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                    )
                };
                let (gated, ready) = delivery_gate(app.clone(), login_delivery);
                let req = if login_delivery {
                    login_request(username().as_str(), password(), &code)
                } else {
                    logout(&token)
                };
                let pending = Pending::start(async move { gated.oneshot(req).await });
                let confirmed = within(ready).await.unwrap_or(false);
                let committed =
                    session_count(f).await == i64::from(login_delivery) && watermark(f).await >= 0;
                let after_commit = snapshot(f).await;
                let req = if login_delivery {
                    raw(untouched_body())
                } else {
                    lifecycle("GET", SESSION, &token, untouched_body())
                };
                let full = denied(
                    &capture(app.clone(), req).await,
                    StatusCode::SERVICE_UNAVAILABLE,
                    "capacity_exhausted",
                    "not_attempted",
                );
                let cancelled = pending.cancel().await;
                let drained = no_connections(f).await;
                let unchanged = drained && after_commit == snapshot(f).await;
                let response = if login_delivery {
                    capture(
                        app.clone(),
                        login_request(username().as_str(), password(), &code),
                    )
                    .await
                } else {
                    capture(app.clone(), get(&token)).await
                };
                let recovered = denied(
                    &response,
                    StatusCode::UNAUTHORIZED,
                    if login_delivery {
                        "invalid_credentials"
                    } else {
                        "invalid_session"
                    },
                    "unconfirmed",
                ) && after_commit == snapshot(f).await
                    && session_count(f).await == i64::from(login_delivery);
                drop(response);
                drop(app);
                drop(state);
                let no_backend = no_connections(f).await;
                let cleaned = prepared.finish();
                (
                    issued, confirmed, committed, full, cancelled, unchanged, recovered,
                    no_backend, cleaned,
                )
            })
        })
        .await;
        assert!(
            issued
                && confirmed
                && committed
                && full
                && cancelled
                && unchanged
                && recovered
                && no_backend
                && cleaned
        );
    }
}
