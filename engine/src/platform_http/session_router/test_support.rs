//! Transport-only fixture. Closed lifecycle storage proves dispatch failure, not authentication,
//! committed logout or shared original storage; genuine SQL tests cover those separate properties.
use super::super::{build_platform_session_router, PlatformSessionHttpState};
pub(super) use crate::platform_http::session_login::test_support::{
    Backend as LoginBackend, Outcome as LoginOutcome,
};
use crate::{
    models::collaboration::AuthorityId,
    platform_http::{session_login::test_support as login, SessionLifecycleHttpState},
    services::auth_service::durable_source::DurableAuthSource,
};
use axum::{
    body::{Body, Bytes},
    http::{header, HeaderMap, Request, StatusCode},
    response::Response,
    Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx_core::connection::ConnectOptions;
use sqlx_postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

pub(super) const ORIGIN: &str = "https://platform.example";
pub(super) const LOGIN: &str = "/platform/v1/session/login";
pub(super) const SESSION: &str = "/platform/v1/session";
pub(super) const LOGOUT: &str = "/platform/v1/session/logout";
pub(super) const TOKEN: &str = "ae0b35cd-c9b8-413c-a892-854b15959f41";
pub(super) const PASSWORD: &str = "public-synthetic-password-only";

pub(super) async fn closed_source() -> DurableAuthSource {
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
pub(super) async fn state(
    backend: Arc<LoginBackend>,
    origin: &str,
    login_capacity: usize,
    lifecycle_capacity: usize,
) -> PlatformSessionHttpState {
    PlatformSessionHttpState {
        login: login::state(backend, origin, login_capacity),
        lifecycle: SessionLifecycleHttpState::with_capacity(
            closed_source().await,
            origin,
            lifecycle_capacity,
        )
        .unwrap(),
    }
}
pub(super) async fn app(backend: Arc<LoginBackend>) -> Router {
    build_platform_session_router(state(backend, ORIGIN, 4, 4).await)
}
pub(super) fn login_body() -> Body {
    Body::from(
        json!({"schema_version":1,"username":"synthetic-user","password":PASSWORD,"otp":"123456"})
            .to_string(),
    )
}
pub(super) fn request(method: &str, path: &str, body: Body) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("x-cyanrex-platform-request", "1")
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json")
        .body(body)
        .unwrap();
    if path == SESSION || path == LOGOUT {
        let mut cookie = format!("cyanrex_platform_session={TOKEN}")
            .parse::<axum::http::HeaderValue>()
            .unwrap();
        cookie.set_sensitive(true);
        request.headers_mut().insert(header::COOKIE, cookie);
    }
    request
}
pub(super) fn untouched() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<Bytes, std::io::Error>>> {
            panic!("combination route/header/capacity rejection must not poll the body")
        },
    ))
}
pub(super) struct Captured {
    pub(super) status: StatusCode,
    pub(super) headers: HeaderMap,
    pub(super) json: Value,
}
pub(super) async fn capture(app: Router, request: Request<Body>) -> Captured {
    capture_response(app.oneshot(request).await.unwrap()).await
}
pub(super) async fn capture_response(response: Response) -> Captured {
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    Captured {
        status,
        headers,
        json,
    }
}
pub(super) fn private(response: &Captured) {
    assert_eq!(response.headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers[header::PRAGMA], "no-cache");
    assert_eq!(response.headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    for name in [
        "location",
        "retry-after",
        "access-control-allow-origin",
        "access-control-allow-credentials",
    ] {
        assert!(!response.headers.contains_key(name));
    }
    for secret in [TOKEN, PASSWORD, "123456"] {
        assert!(
            !response.json.to_string().contains(secret),
            "transport projected synthetic secrets"
        );
    }
}
pub(super) fn error(response: &Captured, status: StatusCode, code: &str, outcome: &str) {
    assert_eq!(response.status, status);
    private(response);
    assert!(!response.headers.contains_key(header::SET_COOKIE));
    assert_eq!(response.headers[header::CONTENT_TYPE], "application/json");
    assert!(
        response.json == json!({"schema_version":1,"error":{"code":code,"outcome":outcome}}),
        "private error projection diverged"
    );
}
pub(super) fn issued(response: &Captured, secure: bool) {
    assert_eq!(response.status, StatusCode::OK);
    private(response);
    assert!(response.json == json!({"schema_version":1,"authenticated":true}));
    assert_eq!(
        response.headers.get_all(header::SET_COOKIE).iter().count(),
        1
    );
    let cookie = response.headers.get(header::SET_COOKIE).unwrap();
    assert!(cookie.is_sensitive());
    let expected = format!(
        "cyanrex_platform_session={TOKEN}; Path=/platform/v1; HttpOnly; SameSite=Strict{}",
        if secure { "; Secure" } else { "" }
    );
    assert!(
        cookie.as_bytes() == expected.as_bytes(),
        "composed route changed issuance Cookie policy"
    );
}
pub(super) fn lifecycle_dispatched(response: &Captured) {
    error(
        response,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    );
}
