use super::test_support::*;
use crate::platform_http::build_text_artifact_publish_router;
use axum::{
    body::{Body, Bytes, HttpBody},
    http::StatusCode,
};
use http_body_util::BodyExt;
use std::time::Duration;
use tower::ServiceExt;

#[tokio::test]
async fn text_publish_clones_hold_capacity_until_final_frame_or_drop_not_retained_json_bytes() {
    let backend = Backend::new(Outcome::Success);
    let shared = state(backend.clone(), ORIGIN, 1);
    let first = build_text_artifact_publish_router(shared.clone());
    let second = build_text_artifact_publish_router(shared);
    let response = first
        .clone()
        .oneshot(request("POST", CREATE, body()))
        .await
        .unwrap();
    error(
        &capture(second.clone(), request("POST", CREATE, untouched())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 1);
    let mut held = response.into_body();
    assert!(!held.is_end_stream());
    let retained = held.frame().await.unwrap().unwrap().into_data().unwrap();
    assert!(!retained.is_empty());
    assert!(held.is_end_stream());
    assert_eq!(
        capture(second.clone(), request("POST", CREATE, body()))
            .await
            .status,
        StatusCode::CREATED
    );
    let response = second
        .clone()
        .oneshot(request("POST", CREATE, body()))
        .await
        .unwrap();
    drop(response);
    assert_eq!(
        capture(first, request("POST", CREATE, body())).await.status,
        StatusCode::CREATED
    );
    assert!(!retained.is_empty());
    assert_eq!(backend.calls(), 4);
}

#[tokio::test]
async fn text_publish_independent_states_have_independent_budgets_and_configured_four_slots() {
    let backend = Backend::new(Outcome::Success);
    let first = app(backend.clone(), 4);
    let independent = app(backend.clone(), 1);
    let mut held = vec![];
    for _ in 0..4 {
        held.push(
            first
                .clone()
                .oneshot(request("POST", CREATE, body()))
                .await
                .unwrap(),
        );
    }
    error(
        &capture(first.clone(), request("POST", CREATE, untouched())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    assert_eq!(
        capture(independent, request("POST", CREATE, body()))
            .await
            .status,
        StatusCode::CREATED
    );
    assert_eq!(backend.calls(), 5);
    drop(held.pop());
    assert_eq!(
        capture(first, request("POST", CREATE, body())).await.status,
        StatusCode::CREATED
    );
    assert_eq!(backend.calls(), 6);
}

#[tokio::test]
async fn text_publish_dispatched_error_response_also_holds_admission_until_drop() {
    let backend = Backend::new(Outcome::Unavailable);
    let app = app(backend.clone(), 1);
    let response = app
        .clone()
        .oneshot(request("POST", CREATE, body()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    error(
        &capture(app.clone(), request("POST", CREATE, untouched())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 1);
    drop(response);
    error(
        &capture(app, request("POST", CREATE, body())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    );
    assert_eq!(backend.calls(), 2);
}

#[tokio::test]
async fn text_publish_body_timeout_and_cancelled_collection_release_their_lane_without_dispatch() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 1);
    let (pending, entered) = pending_body();
    let waiter = tokio::spawn(app.clone().oneshot(request("POST", CREATE, pending)));
    tokio::time::timeout(Duration::from_secs(1), entered.notified())
        .await
        .unwrap();
    error(
        &capture(app.clone(), request("POST", CREATE, untouched())).await,
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
    let waiter = tokio::spawn(app.clone().oneshot(request("POST", CREATE, pending)));
    tokio::time::timeout(Duration::from_secs(1), entered.notified())
        .await
        .unwrap();
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());
    assert_eq!(
        capture(app, request("POST", CREATE, body())).await.status,
        StatusCode::CREATED
    );
    assert_eq!(backend.calls(), 1);
}

#[tokio::test]
async fn text_publish_always_ready_empty_frames_cannot_starve_the_body_deadline() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 1);
    let started = std::time::Instant::now();
    let stream = futures_util::stream::poll_fn(move |_| {
        std::task::Poll::Ready(if started.elapsed() < Duration::from_millis(2200) {
            Some(Ok::<_, std::io::Error>(Bytes::new()))
        } else {
            None
        })
    });
    let response = tokio::time::timeout(
        Duration::from_secs(3),
        capture(
            app.clone(),
            request("POST", CREATE, Body::from_stream(stream)),
        ),
    )
    .await
    .unwrap();
    error(
        &response,
        StatusCode::REQUEST_TIMEOUT,
        "request_timeout",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 0);
    assert_eq!(
        capture(app, request("POST", CREATE, body())).await.status,
        StatusCode::CREATED
    );
}

#[tokio::test]
async fn text_publish_body_deadline_never_wraps_dispatched_mutation_or_retries_it() {
    let backend = Backend::blocking(Outcome::Success);
    let app = app(backend.clone(), 1);
    let mut waiter = tokio::spawn(app.clone().oneshot(request("POST", CREATE, body())));
    backend.entered().await;
    assert!(
        tokio::time::timeout(Duration::from_millis(2200), &mut waiter)
            .await
            .is_err()
    );
    assert_eq!(backend.calls(), 1);
    error(
        &capture(app.clone(), request("POST", CREATE, untouched())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    backend.release();
    let response = capture_response(waiter.await.unwrap().unwrap()).await;
    assert_eq!(response.status, StatusCode::CREATED);
    assert_eq!(backend.calls(), 1);
}

#[tokio::test]
async fn text_publish_cancelled_dispatched_waiter_releases_http_slot_not_a_rollback_receipt() {
    let backend = Backend::blocking(Outcome::Success);
    let app = app(backend.clone(), 1);
    let waiter = tokio::spawn(app.clone().oneshot(request("POST", CREATE, body())));
    backend.entered().await;
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());
    assert_eq!(backend.calls(), 1);
    // Prove new ingress can occupy the slot without retrying or dispatching another mutation.
    let (pending, entered) = pending_body();
    let probe = tokio::spawn(app.oneshot(request("POST", CREATE, pending)));
    tokio::time::timeout(Duration::from_secs(1), entered.notified())
        .await
        .unwrap();
    assert_eq!(backend.calls(), 1);
    probe.abort();
    assert!(probe.await.unwrap_err().is_cancelled());
}
