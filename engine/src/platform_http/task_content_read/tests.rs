//! Transport-only tests. Closed Sources prove dispatch, never successful retained resource access.
use super::test_support::*;
use super::*;
use axum::{
    body::{Body, HttpBody},
    http::{header, HeaderValue, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn content_read_configuration_validates_origin_then_capacity_without_dialing() {
    let fixture = Fixture::new().await;
    for capacity in [1, 4, 16] {
        assert!(fixture.state(ORIGIN, capacity).is_ok());
    }
    for capacity in [0, 17, usize::MAX] {
        assert!(matches!(
            fixture.state(ORIGIN, capacity),
            Err(TaskContentHttpConfigError::InvalidCapacity)
        ));
        assert!(matches!(
            fixture.state("http://platform.example", capacity),
            Err(TaskContentHttpConfigError::InvalidOrigin)
        ));
    }
}

#[tokio::test]
async fn content_read_exposes_only_one_raw_canonical_get_route() {
    let app = app(4).await;
    for path in [
        "/",
        "/platform/v1/tasks",
        "/platform/v1/tasks/",
        "/platform/v1/text-artifacts",
        "/platform/v1/catalog-tasks",
        "/platform/v1/tasks/e3698554-8eb3-4b6d-884e-8074e1555136/content",
        "/platform/v1/tasks/e3698554-8eb3-4b6d-884e-8074e1555136/status",
        "/platform/v1/tasks/e3698554-8eb3-4b6d-884e-8074e1555136/extra",
    ] {
        error(
            &capture(app.clone(), request("POST", path, untouched())).await,
            StatusCode::NOT_FOUND,
            "not_found",
            "not_attempted",
        );
    }
    for id in [
        "not-a-uuid",
        "00000000-0000-0000-0000-000000000000",
        "E3698554-8EB3-4B6D-884E-8074E1555136",
        "e36985548eb34b6d884e8074e1555136",
        "%653698554-8eb3-4b6d-884e-8074e1555136",
    ] {
        error(
            &capture(
                app.clone(),
                request("GET", &format!("/platform/v1/tasks/{id}"), untouched()),
            )
            .await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    for method in ["POST", "PUT", "DELETE", "PATCH", "OPTIONS"] {
        let response = capture(app.clone(), request(method, TASK, untouched())).await;
        error(
            &response,
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "not_attempted",
        );
        assert_eq!(response.headers[header::ALLOW], "GET");
    }
    let head = capture(app.clone(), request("HEAD", TASK, untouched())).await;
    assert_eq!(head.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(head.headers[header::ALLOW], "GET");
    assert_eq!(head.headers[header::CONTENT_LENGTH], "0");
    assert!(head.json.is_null());
    private(&head);
    dispatched(&capture(app, request("GET", TASK, Body::empty())).await);
}

#[tokio::test]
async fn content_read_rejects_query_and_credentials_before_body_or_dispatch() {
    let app = app(4).await;
    for query in [
        "?",
        "?token=secret",
        "?owner=other",
        "?limit=1",
        "?scope=other",
    ] {
        error(
            &capture(
                app.clone(),
                request("GET", &format!("{TASK}{query}"), untouched()),
            )
            .await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    for name in ["x-cyanrex-platform-request", "origin"] {
        let mut req = request("GET", TASK, untouched());
        req.headers_mut()
            .append(name, HeaderValue::from_static("1"));
        error(
            &capture(app.clone(), req).await,
            StatusCode::FORBIDDEN,
            "request_forbidden",
            "not_attempted",
        );
    }
    for mode in ["missing", "duplicate", "legacy", "bearer"] {
        let mut req = request("GET", TASK, untouched());
        match mode {
            "missing" => {
                req.headers_mut().remove(header::COOKIE);
            }
            "duplicate" => {
                req.headers_mut()
                    .append(header::COOKIE, HeaderValue::from_static(COOKIE));
            }
            "legacy" => {
                req.headers_mut().insert(
                    header::COOKIE,
                    HeaderValue::from_static("cyanrex_session=old"),
                );
            }
            _ => {
                req.headers_mut().insert(
                    header::AUTHORIZATION,
                    HeaderValue::from_static("Bearer synthetic"),
                );
            }
        }
        error(
            &capture(app.clone(), req).await,
            StatusCode::UNAUTHORIZED,
            "invalid_session",
            "not_attempted",
        );
    }
    let mut req = request("GET", TASK, Body::empty());
    req.headers_mut().remove(header::ORIGIN);
    dispatched(&capture(app, req).await);
}

#[tokio::test]
async fn content_read_accepts_only_empty_body_and_optional_exact_json_content_type() {
    let app = app(4).await;
    for body in [Body::from("x"), Body::from("{}"), Body::from("\n")] {
        error(
            &capture(app.clone(), request("GET", TASK, body)).await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    let mut req = request("GET", TASK, untouched());
    req.headers_mut().insert(
        header::CONTENT_ENCODING,
        HeaderValue::from_static("identity"),
    );
    error(
        &capture(app.clone(), req).await,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "not_attempted",
    );
    let mut req = request("GET", TASK, untouched());
    req.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"));
    error(
        &capture(app.clone(), req).await,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_media_type",
        "not_attempted",
    );
    for content_type in [
        None,
        Some("application/json"),
        Some("application/json; charset=utf-8"),
    ] {
        let mut req = request("GET", TASK, Body::empty());
        req.headers_mut().remove(header::CONTENT_TYPE);
        if let Some(value) = content_type {
            req.headers_mut()
                .insert(header::CONTENT_TYPE, value.parse().unwrap());
        }
        dispatched(&capture(app.clone(), req).await);
    }
}

#[tokio::test]
async fn content_read_four_response_leases_reject_fifth_without_polling() {
    let app = app(4).await;
    let mut held = Vec::new();
    for _ in 0..4 {
        let response = app
            .clone()
            .oneshot(request("GET", TASK, Body::empty()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        held.push(response);
    }
    exhausted(&capture(app.clone(), request("GET", TASK, untouched())).await);
    drop(held);
    dispatched(&capture(app, request("GET", TASK, Body::empty())).await);
}

#[tokio::test]
async fn content_read_clones_share_response_slots_until_last_frame_or_drop() {
    let fixture = Fixture::new().await;
    let state = fixture.state(ORIGIN, 1).unwrap();
    let first = build_task_content_read_router(state.clone());
    let second = build_task_content_read_router(state);
    let independent = build_task_content_read_router(fixture.state(ORIGIN, 1).unwrap());
    let response = first
        .clone()
        .oneshot(request("GET", TASK, Body::empty()))
        .await
        .unwrap();
    exhausted(&capture(second.clone(), request("GET", TASK, untouched())).await);
    dispatched(&capture(independent, request("GET", TASK, Body::empty())).await);
    let mut body = response.into_body();
    assert!(body.frame().await.unwrap().unwrap().into_data().is_ok());
    assert!(body.is_end_stream());
    let held = second
        .clone()
        .oneshot(request("GET", TASK, Body::empty()))
        .await
        .unwrap();
    exhausted(&capture(first.clone(), request("GET", TASK, untouched())).await);
    drop(held);
    dispatched(&capture(first, request("GET", TASK, Body::empty())).await);
    drop(body);
}

#[tokio::test]
async fn content_read_cancelled_body_releases_only_its_response_admission() {
    let app = app(1).await;
    let (body, entered) = pending_body();
    let pending = tokio::spawn(app.clone().oneshot(request("GET", TASK, body)));
    tokio::time::timeout(std::time::Duration::from_secs(1), entered.notified())
        .await
        .expect("admitted read must reach the body barrier");
    exhausted(&capture(app.clone(), request("GET", TASK, untouched())).await);
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    dispatched(&capture(app, request("GET", TASK, Body::empty())).await);
}

#[tokio::test]
async fn content_read_body_timeout_remains_pre_dispatch_and_releases_on_response_completion() {
    let app = app(1).await;
    let (body, entered) = pending_body();
    let pending = tokio::spawn(app.clone().oneshot(request("GET", TASK, body)));
    tokio::time::timeout(std::time::Duration::from_secs(1), entered.notified())
        .await
        .expect("admitted read must reach the body barrier");
    let response = tokio::time::timeout(std::time::Duration::from_secs(3), pending)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    exhausted(&capture(app.clone(), request("GET", TASK, untouched())).await);
    error(
        &capture_response(response).await,
        StatusCode::REQUEST_TIMEOUT,
        "request_timeout",
        "not_attempted",
    );
    dispatched(&capture(app, request("GET", TASK, Body::empty())).await);
}
