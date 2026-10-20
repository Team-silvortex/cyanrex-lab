use super::fixture::*;
use axum::{
    body::{Body, Bytes},
    http::StatusCode,
    middleware::{self, Next},
};
use cyanrex_engine::platform_http::build_session_login_router;
use std::{
    sync::{Arc, Mutex},
    task::Poll,
};
use tokio::sync::oneshot;
use tower::ServiceExt;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_login_http_clones_hold_capacity_through_body_and_success_response_lifetimes(
) {
    let (
        body_seen,
        body_full,
        cancelled,
        body_recovered,
        response_full,
        issued,
        error_full,
        drop_recovered,
        retained,
        no_backend,
        cleaned,
    ) = Fixture::checked(|f| {
        Box::pin(async move {
            let prepared = Prepared::new(f).await;
            let shared = state(&prepared, 1).await;
            let first = build_session_login_router(shared.clone());
            let second = build_session_login_router(shared.clone());
            let (started, seen) = oneshot::channel();
            let mut started = Some(started);
            let body = Body::from_stream(futures_util::stream::poll_fn(
                move |_| -> Poll<Option<Result<Bytes, std::io::Error>>> {
                    if let Some(started) = started.take() {
                        let _ = started.send(());
                    }
                    Poll::Pending
                },
            ));
            let owned = first.clone();
            let pending = Pending::start(async move { owned.oneshot(raw(body)).await });
            let body_seen = within(seen).await.is_ok();
            let response = capture(second.clone(), raw(untouched_body())).await;
            let body_full = denied(
                &response,
                StatusCode::SERVICE_UNAVAILABLE,
                "capacity_exhausted",
                "not_attempted",
            );
            let cancelled = pending.cancel().await;
            let response = capture(
                first.clone(),
                request(username().as_str(), "synthetic-wrong-password", &otp()),
            )
            .await;
            let body_recovered = denied(
                &response,
                StatusCode::UNAUTHORIZED,
                "invalid_credentials",
                "unconfirmed",
            );

            let held = within(first.clone().oneshot(valid_request()))
                .await
                .unwrap();
            let committed = snapshot(f).await;
            let response = capture(second.clone(), raw(untouched_body())).await;
            let response_full = denied(
                &response,
                StatusCode::SERVICE_UNAVAILABLE,
                "capacity_exhausted",
                "not_attempted",
            );
            let response = capture_response(held).await;
            let issued = issued_token(&response).is_some();
            drop(response);

            // Completion released the successful body's slot, even though the Router state persists.
            let held_error = within(second.clone().oneshot(request(
                username().as_str(),
                "synthetic-wrong-password",
                &otp(),
            )))
            .await
            .unwrap();
            let backend_error = held_error.status() == StatusCode::UNAUTHORIZED;
            let response = capture(first.clone(), raw(untouched_body())).await;
            let error_full = backend_error
                && denied(
                    &response,
                    StatusCode::SERVICE_UNAVAILABLE,
                    "capacity_exhausted",
                    "not_attempted",
                );
            drop(held_error);
            let response = capture(
                first.clone(),
                request(username().as_str(), "synthetic-wrong-password", &otp()),
            )
            .await;
            let drop_recovered = denied(
                &response,
                StatusCode::UNAUTHORIZED,
                "invalid_credentials",
                "unconfirmed",
            );
            let retained = committed == snapshot(f).await
                && session_count(f).await == 1
                && watermark(f).await >= 0;
            drop(response);
            drop(first);
            drop(second);
            drop(shared);
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish();
            (
                body_seen,
                body_full,
                cancelled,
                body_recovered,
                response_full,
                issued,
                error_full,
                drop_recovered,
                retained,
                no_backend,
                cleaned,
            )
        })
    })
    .await;
    assert!(body_seen && body_full && cancelled && body_recovered && response_full && issued);
    assert!(error_full && drop_recovered && retained && no_backend && cleaned);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_login_http_cancelled_observed_source_wait_closes_without_manufacturing_success(
) {
    let (waited, full, cancelled, released, drained, unchanged, recovered, no_backend, cleaned) =
        Fixture::checked(|f| {
            Box::pin(async move {
                let prepared = Prepared::new(f).await;
                let app = build_session_login_router(state(&prepared, 1).await);
                let before = snapshot(f).await;
                let (holder, pid) = source_holder(f).await;
                let owned = app.clone();
                let pending = Pending::start(async move { owned.oneshot(valid_request()).await });
                let waited = waiting_on(f, pid).await;
                let response = capture(app.clone(), raw(untouched_body())).await;
                let full = denied(
                    &response,
                    StatusCode::SERVICE_UNAVAILABLE,
                    "capacity_exhausted",
                    "not_attempted",
                );
                let cancelled = pending.cancel().await;
                let released = release(holder).await;
                // The real runtime backend is gone before treating the snapshot as rollback evidence.
                let drained = no_connections(f).await;
                let unchanged = before == snapshot(f).await
                    && session_count(f).await == 0
                    && watermark(f).await == -1;
                let response = capture(app.clone(), valid_request()).await;
                let recovered = issued_token(&response).is_some() && session_count(f).await == 1;
                drop(response);
                drop(app);
                let no_backend = no_connections(f).await;
                let cleaned = prepared.finish();
                (
                    waited, full, cancelled, released, drained, unchanged, recovered, no_backend,
                    cleaned,
                )
            })
        })
        .await;
    assert!(
        waited
            && full
            && cancelled
            && released
            && drained
            && unchanged
            && recovered
            && no_backend
            && cleaned
    );
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_login_http_cancelled_confirmed_but_undelivered_response_keeps_session_and_otp(
) {
    let (confirmed, persisted, full, cancelled, unchanged, replay, recovered, no_backend, cleaned) =
        Fixture::checked(|f| {
            Box::pin(async move {
                let prepared = Prepared::new(f).await;
                let app = build_session_login_router(state(&prepared, 1).await);
                let (ready, confirmed) = oneshot::channel();
                let ready = Arc::new(Mutex::new(Some(ready)));
                // Test-only outer transport gate: a real confirmed response exists, but the caller has
                // received neither headers nor body. It does not simulate a missing database COMMIT ACK.
                let gated = app
                    .clone()
                    .layer(middleware::from_fn(move |request, next: Next| {
                        let ready = ready.clone();
                        async move {
                            let response = next.run(request).await;
                            let projected = response.status() == StatusCode::OK
                                && response
                                    .headers()
                                    .get("set-cookie")
                                    .is_some_and(|h| h.is_sensitive());
                            if let Some(ready) = ready.lock().unwrap().take() {
                                let _ = ready.send(projected);
                            }
                            std::future::pending::<()>().await;
                            response
                        }
                    }));
                let code = otp();
                let req = request(username().as_str(), password(), &code);
                let pending = Pending::start(async move { gated.oneshot(req).await });
                let confirmed = within(confirmed).await.unwrap_or(false);
                let persisted = session_count(f).await == 1 && watermark(f).await >= 0;
                let committed = snapshot(f).await;
                let response = capture(app.clone(), raw(untouched_body())).await;
                let full = denied(
                    &response,
                    StatusCode::SERVICE_UNAVAILABLE,
                    "capacity_exhausted",
                    "not_attempted",
                );
                let cancelled = pending.cancel().await;
                let no_backend = no_connections(f).await;
                let unchanged = committed == snapshot(f).await && session_count(f).await == 1;
                let response =
                    capture(app.clone(), request(username().as_str(), password(), &code)).await;
                let replay = denied(
                    &response,
                    StatusCode::UNAUTHORIZED,
                    "invalid_credentials",
                    "unconfirmed",
                ) && committed == snapshot(f).await;
                // A fresh invalid-password request passes transport admission; no implicit retry,
                // token adoption, Session deletion or OTP reset follows cancellation of delivery.
                let response = capture(
                    app.clone(),
                    request(username().as_str(), "synthetic-wrong-password", &code),
                )
                .await;
                let recovered = denied(
                    &response,
                    StatusCode::UNAUTHORIZED,
                    "invalid_credentials",
                    "unconfirmed",
                ) && committed == snapshot(f).await
                    && session_count(f).await == 1;
                drop(response);
                drop(app);
                let no_backend = no_backend && no_connections(f).await;
                let cleaned = prepared.finish();
                (
                    confirmed, persisted, full, cancelled, unchanged, replay, recovered,
                    no_backend, cleaned,
                )
            })
        })
        .await;
    assert!(
        confirmed
            && persisted
            && full
            && cancelled
            && unchanged
            && replay
            && recovered
            && no_backend
            && cleaned
    );
}
