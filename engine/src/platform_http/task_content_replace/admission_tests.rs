use super::test_support::*;
use crate::platform_http::build_task_content_replace_router;
use axum::{body::HttpBody, http::StatusCode};
use http_body_util::BodyExt;
use std::time::Duration;
use tower::ServiceExt;

#[tokio::test]
async fn replace_capacity_is_shared_until_final_response_frame_or_drop() {
    let backend = Backend::new(Outcome::Success);
    let shared = state(backend.clone(), ORIGIN, 1);
    let first = build_task_content_replace_router(shared.clone());
    let second = build_task_content_replace_router(shared);
    let response = first
        .clone()
        .oneshot(request("PUT", REPLACE, body()))
        .await
        .unwrap();
    error(
        &capture(second.clone(), request("PUT", REPLACE, untouched())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 1);
    let mut held = response.into_body();
    assert!(!held.is_end_stream());
    assert!(held.frame().await.unwrap().unwrap().is_data());
    assert!(held.is_end_stream());
    assert_eq!(
        capture(second.clone(), request("PUT", REPLACE, body()))
            .await
            .status,
        StatusCode::OK
    );
    let response = second
        .clone()
        .oneshot(request("PUT", REPLACE, body()))
        .await
        .unwrap();
    drop(response);
    assert_eq!(
        capture(first, request("PUT", REPLACE, body())).await.status,
        StatusCode::OK
    );
    assert_eq!(backend.calls(), 4);
}

#[tokio::test]
async fn replace_body_deadline_and_cancelled_waiter_release_only_their_own_lane() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 1);
    let (pending, entered) = pending_body();
    let waiter = tokio::spawn(app.clone().oneshot(request("PUT", REPLACE, pending)));
    tokio::time::timeout(Duration::from_secs(1), entered.notified())
        .await
        .unwrap();
    error(
        &capture(app.clone(), request("PUT", REPLACE, untouched())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    let response = tokio::time::timeout(Duration::from_secs(3), waiter)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    error(
        &capture_response(response).await,
        StatusCode::REQUEST_TIMEOUT,
        "request_timeout",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 0);
    let (pending, entered) = pending_body();
    let waiter = tokio::spawn(app.clone().oneshot(request("PUT", REPLACE, pending)));
    tokio::time::timeout(Duration::from_secs(1), entered.notified())
        .await
        .unwrap();
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());
    assert_eq!(
        capture(app, request("PUT", REPLACE, body())).await.status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn replace_dispatched_mutation_outlives_body_deadline_and_is_never_retried() {
    let backend = Backend::blocking(Outcome::Success);
    let app = app(backend.clone(), 1);
    let mut waiter = tokio::spawn(app.clone().oneshot(request("PUT", REPLACE, body())));
    backend.entered().await;
    assert!(
        tokio::time::timeout(Duration::from_millis(2200), &mut waiter)
            .await
            .is_err()
    );
    assert_eq!(backend.calls(), 1);
    error(
        &capture(app.clone(), request("PUT", REPLACE, untouched())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    backend.release();
    let response = capture_response(waiter.await.unwrap().unwrap()).await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(backend.calls(), 1);
}
