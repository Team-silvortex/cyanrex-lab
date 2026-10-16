#![cfg(unix)]
//! Default transport/legacy-constructor preservation, not real platform storage authorization.

use axum::{
    body::{Body, Bytes},
    http::{header, Request, StatusCode},
    response::Response,
};
use cyanrex_engine::{
    models::collaboration::AuthorityId,
    platform_host::PlatformStorageConnector,
    platform_http::{
        build_session_lifecycle_router, SessionLifecycleHttpState, TaskContentHttpConfigError,
    },
    services::auth_service::durable_source::DurableAuthSource,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx_core::connection::ConnectOptions;
use sqlx_postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use tower::ServiceExt;
use uuid::Uuid;

const ORIGIN: &str = "https://platform.example:8443";
const TOKEN: &str = "ae0b35cd-c9b8-413c-a892-854b15959f41";
const SESSION: &str = "/platform/v1/session";
const LOGOUT: &str = "/platform/v1/session/logout";

async fn source() -> DurableAuthSource {
    let pool = PgPoolOptions::new().connect_lazy_with(
        PgConnectOptions::new_without_pgpass()
            .host("127.0.0.1")
            .port(1)
            .username("synthetic_closed_source")
            .database("synthetic_unopened_database")
            .password("")
            .ssl_mode(PgSslMode::Disable)
            .disable_statement_logging(),
    );
    pool.close().await;
    DurableAuthSource::new(pool, AuthorityId::try_from(Uuid::from_u128(4201)).unwrap())
}

fn request(method: &str, path: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("x-cyanrex-platform-request", "1")
        .header(header::ORIGIN, ORIGIN)
        .header(header::COOKIE, format!("cyanrex_platform_session={TOKEN}"))
        .body(body)
        .unwrap()
}

fn untouched() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<Bytes, std::io::Error>>> {
            panic!("transport rejection must not poll the request body")
        },
    ))
}

async fn failure(response: Response, status: StatusCode, code: &str, outcome: &str) {
    assert_eq!(response.status(), status);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()[header::PRAGMA], "no-cache");
    assert_eq!(
        response.headers()[header::X_CONTENT_TYPE_OPTIONS],
        "nosniff"
    );
    for name in [
        header::SET_COOKIE,
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        header::RETRY_AFTER,
    ] {
        assert!(!response.headers().contains_key(name));
    }
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap(),
        json!({"schema_version":1,"error":{"code":code,"outcome":outcome}})
    );
}

#[test]
fn platform_session_http_storage_entry_consumes_one_binding_without_a_separate_origin_or_source() {
    let _: fn(
        PlatformStorageConnector,
        usize,
    ) -> Result<SessionLifecycleHttpState, TaskContentHttpConfigError> =
        SessionLifecycleHttpState::with_platform_storage;
    fn state_traits<T: Clone + Send + Sync>() {}
    state_traits::<SessionLifecycleHttpState>();
}

#[tokio::test]
async fn platform_session_http_original_source_constructors_keep_origin_and_capacity_precedence() {
    let source = source().await;
    assert!(SessionLifecycleHttpState::new(source.clone(), ORIGIN).is_ok());
    for capacity in [1, 4, 16] {
        assert!(SessionLifecycleHttpState::with_capacity(source.clone(), ORIGIN, capacity).is_ok());
    }
    for capacity in [0, 17, usize::MAX] {
        assert!(matches!(
            SessionLifecycleHttpState::with_capacity(source.clone(), ORIGIN, capacity),
            Err(TaskContentHttpConfigError::InvalidCapacity)
        ));
        assert!(matches!(
            SessionLifecycleHttpState::with_capacity(
                source.clone(),
                "https://PLATFORM.example/",
                capacity
            ),
            Err(TaskContentHttpConfigError::InvalidOrigin)
        ));
    }
}

#[tokio::test]
async fn platform_session_http_source_backend_failure_remains_unconfirmed_and_preserves_cookie() {
    let state = SessionLifecycleHttpState::new(source().await, ORIGIN).unwrap();
    let app = build_session_lifecycle_router(state);
    for (method, path) in [("GET", SESSION), ("POST", LOGOUT)] {
        for _ in 0..2 {
            let response = app
                .clone()
                .oneshot(request(method, path, Body::empty()))
                .await
                .unwrap();
            failure(
                response,
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
                "unconfirmed",
            )
            .await;
        }
    }
}

#[tokio::test]
async fn platform_session_http_backend_state_clones_share_slots_but_separate_states_do_not() {
    let source = source().await;
    let state = SessionLifecycleHttpState::with_capacity(source.clone(), ORIGIN, 1).unwrap();
    let retained = build_session_lifecycle_router(state.clone())
        .oneshot(request("GET", SESSION, Body::empty()))
        .await
        .unwrap();
    assert_eq!(retained.status(), StatusCode::SERVICE_UNAVAILABLE);
    let exhausted = build_session_lifecycle_router(state.clone())
        .oneshot(request("POST", LOGOUT, untouched()))
        .await
        .unwrap();
    failure(
        exhausted,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    )
    .await;
    let independent = SessionLifecycleHttpState::with_capacity(source, ORIGIN, 1).unwrap();
    let independent = build_session_lifecycle_router(independent)
        .oneshot(request("POST", LOGOUT, Body::empty()))
        .await
        .unwrap();
    failure(
        independent,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
    drop(retained);
    let available = build_session_lifecycle_router(state)
        .oneshot(request("POST", LOGOUT, Body::empty()))
        .await
        .unwrap();
    failure(
        available,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
}

#[tokio::test]
async fn platform_session_http_credentials_and_route_admission_still_precede_backend_dispatch() {
    let app = build_session_lifecycle_router(
        SessionLifecycleHttpState::new(source().await, ORIGIN).unwrap(),
    );
    let mut wrong_origin = request("POST", LOGOUT, untouched());
    wrong_origin
        .headers_mut()
        .insert(header::ORIGIN, "https://other.example".parse().unwrap());
    failure(
        app.clone().oneshot(wrong_origin).await.unwrap(),
        StatusCode::FORBIDDEN,
        "request_forbidden",
        "not_attempted",
    )
    .await;
    let mut missing_cookie = request("GET", SESSION, untouched());
    missing_cookie.headers_mut().remove(header::COOKIE);
    failure(
        app.clone().oneshot(missing_cookie).await.unwrap(),
        StatusCode::UNAUTHORIZED,
        "invalid_session",
        "not_attempted",
    )
    .await;
    failure(
        app.oneshot(request("GET", "/platform/v1/session/login", untouched()))
            .await
            .unwrap(),
        StatusCode::NOT_FOUND,
        "not_found",
        "not_attempted",
    )
    .await;
}
