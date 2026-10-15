use axum::{
    body::{Body, Bytes},
    http::{header, HeaderMap, Request, StatusCode},
    Router,
};
use cyanrex_engine::{
    models::collaboration::AuthorityId,
    platform_http::{build_session_lifecycle_router, SessionLifecycleHttpState},
    services::auth_service::durable_source::DurableAuthSource,
};
use http_body::Frame;
use http_body_util::{BodyExt, StreamBody};
use serde_json::{json, Value};
use sqlx_postgres::PgPoolOptions;
use std::time::Duration;
use tower::ServiceExt;
use uuid::Uuid;

const ORIGIN: &str = "https://platform.example";
const SESSION: &str = "/platform/v1/session";
const LOGOUT: &str = "/platform/v1/session/logout";
const TOKEN: &str = "ae0b35cd-c9b8-413c-a892-854b15959f41";

async fn source() -> DurableAuthSource {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    DurableAuthSource::new(pool, AuthorityId::try_from(Uuid::new_v4()).unwrap())
}
async fn app() -> Router {
    build_session_lifecycle_router(SessionLifecycleHttpState::new(source().await, ORIGIN).unwrap())
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
fn untouched_body() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<Bytes, std::io::Error>>> {
            panic!("rejected admission must not poll the body")
        },
    ))
}
fn private(response: &axum::response::Response) {
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()[header::PRAGMA], "no-cache");
    assert_eq!(
        response.headers()[header::X_CONTENT_TYPE_OPTIONS],
        "nosniff"
    );
    for name in [
        header::SET_COOKIE,
        header::RETRY_AFTER,
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
    ] {
        assert!(!response.headers().contains_key(name));
    }
}
async fn error(app: Router, req: Request<Body>, status: StatusCode, code: &str, outcome: &str) {
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), status);
    private(&response);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap(),
        json!({"schema_version":1,"error":{"code":code,"outcome":outcome}})
    );
}

#[tokio::test]
async fn session_http_config_is_canonical_and_capacity_is_explicitly_bounded() {
    let source = source().await;
    for origin in [
        ORIGIN,
        "https://platform.example:8443",
        "http://localhost:3000",
        "http://127.0.0.1:3000",
        "http://[::1]:3000",
    ] {
        assert!(SessionLifecycleHttpState::new(source.clone(), origin).is_ok());
    }
    for origin in [
        "",
        "null",
        "*",
        "http://192.168.1.1",
        "http://platform.example",
        "https://PLATFORM.example",
        "https://platform.example/",
        "https://platform.example:443",
        "https://platform.example/path",
        "https://user@platform.example",
        "https://platform.example?secret=a",
        "https://platform.example#fragment",
        " https://platform.example",
        "https://*.example",
    ] {
        assert!(SessionLifecycleHttpState::new(source.clone(), origin).is_err());
    }
    for capacity in [0, 17, usize::MAX] {
        assert!(
            SessionLifecycleHttpState::with_capacity(source.clone(), ORIGIN, capacity).is_err()
        );
    }
    for capacity in [1, 4, 16] {
        assert!(SessionLifecycleHttpState::with_capacity(source.clone(), ORIGIN, capacity).is_ok());
    }
}

#[tokio::test]
async fn session_http_only_exact_routes_and_methods_are_admitted() {
    let app = app().await;
    for (path, allow) in [(SESSION, "GET"), (LOGOUT, "POST")] {
        for method in ["GET", "POST", "PUT", "DELETE", "OPTIONS"] {
            if method == allow {
                continue;
            }
            let response = app
                .clone()
                .oneshot(request(method, path, untouched_body()))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
            assert_eq!(response.headers()[header::ALLOW], allow);
            private(&response);
            assert_eq!(
                serde_json::from_slice::<Value>(
                    &response.into_body().collect().await.unwrap().to_bytes()
                )
                .unwrap(),
                json!({"schema_version":1,"error":{"code":"method_not_allowed","outcome":"not_attempted"}})
            );
        }
        let response = app
            .clone()
            .oneshot(request("HEAD", path, untouched_body()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response.headers()[header::ALLOW], allow);
        assert_eq!(response.headers()[header::CONTENT_LENGTH], "0");
        private(&response);
        assert!(response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .is_empty());
    }
    for path in [
        "/platform/v1/session/",
        "/platform/v1/session/login",
        "/auth/login",
        "/platform/v1/session/logout/",
        "/platform/v1/session/%6cout",
    ] {
        error(
            app.clone(),
            request("GET", path, untouched_body()),
            StatusCode::NOT_FOUND,
            "not_found",
            "not_attempted",
        )
        .await;
    }
    for (method, path) in [("GET", SESSION), ("POST", LOGOUT)] {
        error(
            app.clone(),
            request(method, &format!("{path}?token={TOKEN}"), untouched_body()),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        )
        .await;
    }
}

#[tokio::test]
async fn session_http_never_adopts_legacy_duplicate_noncanonical_or_authorization_credentials() {
    let app = app().await;
    for cookie in [
        String::new(),
        format!("cyanrex_session={TOKEN}"),
        "cyanrex_platform_session=invalid".into(),
        format!("cyanrex_platform_session={}", TOKEN.to_uppercase()),
        format!("cyanrex_platform_session={}", TOKEN.replace('-', "")),
        "cyanrex_platform_session=00000000-0000-0000-0000-000000000000".into(),
        format!("cyanrex_platform_session=\"{TOKEN}\""),
        format!("cyanrex_platform_session={TOKEN}; cyanrex_platform_session={TOKEN}"),
    ] {
        let mut req = request("POST", LOGOUT, untouched_body());
        req.headers_mut()
            .insert(header::COOKIE, cookie.parse().unwrap());
        error(
            app.clone(),
            req,
            StatusCode::UNAUTHORIZED,
            "invalid_session",
            "not_attempted",
        )
        .await;
    }
    let mut req = request("GET", SESSION, untouched_body());
    req.headers_mut().append(
        header::COOKIE,
        format!("cyanrex_platform_session={TOKEN}").parse().unwrap(),
    );
    error(
        app.clone(),
        req,
        StatusCode::UNAUTHORIZED,
        "invalid_session",
        "not_attempted",
    )
    .await;
    let mut req = request("GET", SESSION, untouched_body());
    req.headers_mut()
        .insert(header::AUTHORIZATION, "Bearer synthetic".parse().unwrap());
    error(
        app.clone(),
        req,
        StatusCode::UNAUTHORIZED,
        "invalid_session",
        "not_attempted",
    )
    .await;
    let mut req = request("GET", SESSION, untouched_body());
    req.headers_mut().append(
        header::COOKIE,
        format!("padding={}", "x".repeat(8192)).parse().unwrap(),
    );
    error(
        app,
        req,
        StatusCode::PAYLOAD_TOO_LARGE,
        "request_too_large",
        "not_attempted",
    )
    .await;
}

#[tokio::test]
async fn session_http_origin_and_request_marker_reject_before_body_or_source() {
    let app = app().await;
    for (method, path) in [("GET", SESSION), ("POST", LOGOUT)] {
        for marker in [None, Some(""), Some("true"), Some("01"), Some("1, 1")] {
            let mut req = request(method, path, untouched_body());
            req.headers_mut().remove("x-cyanrex-platform-request");
            if let Some(marker) = marker {
                req.headers_mut()
                    .insert("x-cyanrex-platform-request", marker.parse().unwrap());
            }
            error(
                app.clone(),
                req,
                StatusCode::FORBIDDEN,
                "request_forbidden",
                "not_attempted",
            )
            .await;
        }
        for origin in [
            "null",
            "https://evil.example",
            "https://platform.example/",
            "https://PLATFORM.example",
            "http://platform.example",
        ] {
            let mut req = request(method, path, untouched_body());
            req.headers_mut()
                .insert(header::ORIGIN, origin.parse().unwrap());
            error(
                app.clone(),
                req,
                StatusCode::FORBIDDEN,
                "request_forbidden",
                "not_attempted",
            )
            .await;
        }
        for name in ["origin", "x-cyanrex-platform-request"] {
            let mut req = request(method, path, untouched_body());
            let value = req.headers()[name].clone();
            req.headers_mut().append(name, value);
            error(
                app.clone(),
                req,
                StatusCode::FORBIDDEN,
                "request_forbidden",
                "not_attempted",
            )
            .await;
        }
    }
    let mut req = request("POST", LOGOUT, untouched_body());
    req.headers_mut().remove(header::ORIGIN);
    req.headers_mut()
        .insert(header::REFERER, format!("{ORIGIN}/safe").parse().unwrap());
    error(
        app.clone(),
        req,
        StatusCode::FORBIDDEN,
        "request_forbidden",
        "not_attempted",
    )
    .await;
    let mut req = request("GET", SESSION, Body::empty());
    req.headers_mut().remove(header::ORIGIN);
    error(
        app,
        req,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
}

#[tokio::test]
async fn session_http_empty_body_encoding_media_type_and_trailer_guards_precede_source() {
    let app = app().await;
    for (method, path) in [("GET", SESSION), ("POST", LOGOUT)] {
        error(
            app.clone(),
            request(method, path, Body::from("{}")),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        )
        .await;
        let mut req = request(method, path, untouched_body());
        req.headers_mut()
            .insert(header::CONTENT_ENCODING, "identity".parse().unwrap());
        error(
            app.clone(),
            req,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        )
        .await;
        let mut req = request(method, path, untouched_body());
        req.headers_mut()
            .insert(header::CONTENT_TYPE, "text/plain".parse().unwrap());
        error(
            app.clone(),
            req,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "not_attempted",
        )
        .await;
        let body = Body::new(StreamBody::new(futures_util::stream::iter([Ok::<
            _,
            std::io::Error,
        >(
            Frame::<Bytes>::trailers(HeaderMap::new()),
        )])));
        error(
            app.clone(),
            request(method, path, body),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        )
        .await;
        let body = Body::from_stream(futures_util::stream::iter([Err::<Bytes, _>(
            std::io::Error::other("synthetic private fault"),
        )]));
        error(
            app.clone(),
            request(method, path, body),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        )
        .await;
    }
}

#[tokio::test]
async fn session_http_stalled_empty_body_has_one_fixed_deadline() {
    let body = Body::from_stream(futures_util::stream::pending::<Result<Bytes, std::io::Error>>());
    tokio::time::timeout(
        Duration::from_secs(4),
        error(
            app().await,
            request("POST", LOGOUT, body),
            StatusCode::REQUEST_TIMEOUT,
            "request_timeout",
            "not_attempted",
        ),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn session_http_closed_source_never_clears_cookie_or_claims_authentication() {
    let app = app().await;
    for (method, path) in [("GET", SESSION), ("POST", LOGOUT)] {
        error(
            app.clone(),
            request(method, path, Body::empty()),
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "unconfirmed",
        )
        .await;
    }
}

#[tokio::test]
async fn session_http_response_lifetime_slots_are_shared_by_clones_and_release_on_body_completion_or_drop(
) {
    let state = SessionLifecycleHttpState::with_capacity(source().await, ORIGIN, 1).unwrap();
    let first = build_session_lifecycle_router(state.clone())
        .oneshot(request("GET", SESSION, Body::empty()))
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::SERVICE_UNAVAILABLE);
    private(&first);
    error(
        build_session_lifecycle_router(state.clone()),
        request("POST", LOGOUT, untouched_body()),
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    )
    .await;
    first.into_body().collect().await.unwrap();
    let next = build_session_lifecycle_router(state.clone())
        .oneshot(request("POST", LOGOUT, Body::empty()))
        .await
        .unwrap();
    assert_eq!(next.status(), StatusCode::SERVICE_UNAVAILABLE);
    private(&next);
    drop(next);
    error(
        build_session_lifecycle_router(state),
        request("GET", SESSION, Body::empty()),
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
}
