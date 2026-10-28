//! Synthetic composition evidence only; original-storage and actual host checks are opt-in SQL.
use super::{build_platform_workspace_router, test_support::*};
use crate::platform_http::{config, TaskContentHttpConfigError};
use axum::{
    body::{Body, HttpBody},
    http::{header, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[test]
fn task_publication_configuration_checks_all_eight_capacities_after_origin() {
    for slot in 0..8 {
        for capacity in [1, 4, 16] {
            let mut capacities = [4; 8];
            capacities[slot] = capacity;
            assert_eq!(config::validate(ORIGIN, &capacities), Ok(()));
        }
        for invalid in [0, 17, usize::MAX] {
            let mut capacities = [4; 8];
            capacities[slot] = invalid;
            assert_eq!(
                config::validate(ORIGIN, &capacities),
                Err(TaskContentHttpConfigError::InvalidCapacity)
            );
            assert_eq!(
                config::validate("http://platform.example", &capacities),
                Err(TaskContentHttpConfigError::InvalidOrigin)
            );
        }
    }
}

#[tokio::test]
async fn earlier_workspace_states_keep_task_bound_publication_absent_without_polling_body() {
    for capacity in [1, 4, 16] {
        let login = LoginBackend::new(LoginOutcome::Success);
        let publisher = PublicationBackend::new(PublicationOutcome::Success);
        let legacy = state(login.clone(), [capacity; 6]).await;
        let unbound = publication_state(login.clone(), publisher.clone(), [capacity; 7]).await;
        assert!(legacy.task_text.is_none());
        assert!(unbound.task_text.is_none());
        for state in [legacy, unbound] {
            error(
                &capture(
                    build_platform_workspace_router(state),
                    request("POST", TASK_PUBLISH, untouched()),
                )
                .await,
                StatusCode::NOT_FOUND,
                "not_found",
                "not_attempted",
            );
        }
        assert_eq!(publisher.calls(), 0);
        assert_eq!(login.calls(), 0);
    }
}

#[tokio::test]
async fn task_publication_workspace_adds_one_post_and_keeps_all_eight_original_routes() {
    let login = LoginBackend::new(LoginOutcome::Success);
    let publisher = PublicationBackend::new(PublicationOutcome::Success);
    let task_publisher = TaskPublicationBackend::new(TaskPublicationOutcome::Success);
    let app = build_platform_workspace_router(
        task_publication_state(
            login.clone(),
            publisher.clone(),
            task_publisher.clone(),
            [4; 8],
        )
        .await,
    );
    let result = capture(
        app.clone(),
        request("POST", TASK_PUBLISH, task_publication_body()),
    )
    .await;
    assert_eq!(result.status, StatusCode::CREATED);
    private(&result);
    assert_eq!(result.json.as_object().unwrap().len(), 2);
    assert_eq!(result.json["artifact"]["kind"], "cyanrex.task-text");
    assert!(!result.headers.contains_key(header::SET_COOKIE));
    assert!(result.json.get("text").is_none() && result.json.get("task").is_none());
    assert_eq!(task_publisher.calls(), 1);
    assert_eq!(
        capture(app.clone(), request("POST", PUBLISH, publication_body()))
            .await
            .status,
        StatusCode::CREATED
    );
    issued(&capture(app.clone(), request("POST", LOGIN, login_body())).await);
    for (method, path, body) in [
        ("GET", SESSION, Body::empty()),
        ("POST", LOGOUT, Body::empty()),
        ("GET", INDEX, Body::empty()),
        ("GET", CONTENT, Body::empty()),
        ("POST", CREATE, create_body()),
        ("PUT", REPLACE, replace_body()),
    ] {
        dispatched(&capture(app.clone(), request(method, path, body)).await);
    }
    assert_eq!(publisher.calls(), 1);
    assert_eq!(login.calls(), 1);
    assert_eq!(task_publisher.calls(), 1);
    for path in [
        format!("{TASK_PUBLISH}/"),
        format!("{TASK_PUBLISH}/extra"),
        "/ebpf/run".into(),
        "/platform/v1/catalog-tasks".into(),
        "/platform/v1/file-chunks".into(),
        format!("{CONTENT}/status"),
    ] {
        error(
            &capture(app.clone(), request("POST", &path, untouched())).await,
            StatusCode::NOT_FOUND,
            "not_found",
            "not_attempted",
        );
    }
    for method in ["GET", "PUT", "DELETE", "OPTIONS"] {
        let response = capture(app.clone(), request(method, TASK_PUBLISH, untouched())).await;
        error(
            &response,
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "not_attempted",
        );
        assert_eq!(response.headers[header::ALLOW], "POST");
    }
    assert_eq!(task_publisher.calls(), 1);
}

#[tokio::test]
async fn task_publication_workspace_eight_lanes_share_clones_but_not_each_other() {
    let login = LoginBackend::new(LoginOutcome::Success);
    let publisher = PublicationBackend::new(PublicationOutcome::Success);
    let task_publisher = TaskPublicationBackend::new(TaskPublicationOutcome::Success);
    let state = task_publication_state(
        login.clone(),
        publisher.clone(),
        task_publisher.clone(),
        [1; 8],
    )
    .await;
    let first = build_platform_workspace_router(state.clone());
    let second = build_platform_workspace_router(state);
    let published = first
        .clone()
        .oneshot(request("POST", TASK_PUBLISH, task_publication_body()))
        .await
        .unwrap();
    assert_eq!(published.status(), StatusCode::CREATED);
    let mut held = vec![];
    for (method, path, body, status) in [
        ("POST", PUBLISH, publication_body(), StatusCode::CREATED),
        ("POST", LOGIN, login_body(), StatusCode::OK),
        (
            "GET",
            SESSION,
            Body::empty(),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        ("GET", INDEX, Body::empty(), StatusCode::SERVICE_UNAVAILABLE),
        (
            "GET",
            CONTENT,
            Body::empty(),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        (
            "POST",
            CREATE,
            create_body(),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        (
            "PUT",
            REPLACE,
            replace_body(),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
    ] {
        let response = second
            .clone()
            .oneshot(request(method, path, body))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            status,
            "Task publication must not block {path}"
        );
        held.push(response);
    }
    for (method, path) in [
        ("POST", TASK_PUBLISH),
        ("POST", PUBLISH),
        ("POST", LOGIN),
        ("GET", SESSION),
        ("POST", LOGOUT),
        ("GET", INDEX),
        ("GET", CONTENT),
        ("POST", CREATE),
        ("PUT", REPLACE),
    ] {
        error(
            &capture(first.clone(), request(method, path, untouched())).await,
            StatusCode::SERVICE_UNAVAILABLE,
            "capacity_exhausted",
            "not_attempted",
        );
    }
    assert_eq!(task_publisher.calls(), 1);
    assert_eq!(publisher.calls(), 1);
    assert_eq!(login.calls(), 1);
    let mut body = published.into_body();
    assert!(!body.is_end_stream());
    let retained = body.frame().await.unwrap().unwrap().into_data().unwrap();
    assert!(body.is_end_stream());
    assert!(!retained.is_empty());
    assert_eq!(
        capture(
            second.clone(),
            request("POST", TASK_PUBLISH, task_publication_body())
        )
        .await
        .status,
        StatusCode::CREATED
    );
    error(
        &capture(first.clone(), request("POST", PUBLISH, untouched())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    let published = first
        .clone()
        .oneshot(request("POST", TASK_PUBLISH, task_publication_body()))
        .await
        .unwrap();
    drop(published);
    assert_eq!(
        capture(
            second.clone(),
            request("POST", TASK_PUBLISH, task_publication_body())
        )
        .await
        .status,
        StatusCode::CREATED
    );
    drop(held.remove(0));
    assert_eq!(
        capture(second, request("POST", PUBLISH, publication_body()))
            .await
            .status,
        StatusCode::CREATED
    );
    assert_eq!(task_publisher.calls(), 4);
    assert_eq!(publisher.calls(), 2);
    assert!(!retained.is_empty());
}

#[tokio::test]
async fn task_publication_workspace_errors_also_keep_their_own_lane_until_drop() {
    let task_publisher = TaskPublicationBackend::new(TaskPublicationOutcome::ArtifactConflict);
    let app = build_platform_workspace_router(
        task_publication_state(
            LoginBackend::new(LoginOutcome::Success),
            PublicationBackend::new(PublicationOutcome::Success),
            task_publisher.clone(),
            [1; 8],
        )
        .await,
    );
    let held = app
        .clone()
        .oneshot(request("POST", TASK_PUBLISH, task_publication_body()))
        .await
        .unwrap();
    assert_eq!(held.status(), StatusCode::CONFLICT);
    error(
        &capture(app.clone(), request("POST", TASK_PUBLISH, untouched())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    assert_eq!(
        capture(app.clone(), request("POST", PUBLISH, publication_body()))
            .await
            .status,
        StatusCode::CREATED
    );
    drop(held);
    error(
        &capture(app, request("POST", TASK_PUBLISH, task_publication_body())).await,
        StatusCode::CONFLICT,
        "conflict",
        "unconfirmed",
    );
    assert_eq!(task_publisher.calls(), 2);
}
