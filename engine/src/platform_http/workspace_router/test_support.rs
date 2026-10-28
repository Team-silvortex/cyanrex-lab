//! Transport-only fixture: closed sources establish dispatch, never successful resource access
//! or shared original storage. Separate SQL tests exercise the real single-binding constructor.
use super::{build_platform_workspace_router, PlatformWorkspaceHttpState};
pub(super) use crate::platform_http::session_login::test_support::{
    Backend as LoginBackend, Outcome as LoginOutcome,
};
pub(super) use crate::platform_http::task_text_artifact_publish::test_support::{
    Backend as TaskPublicationBackend, Outcome as TaskPublicationOutcome, CREATE as TASK_PUBLISH,
};
pub(super) use crate::platform_http::text_artifact_publish::test_support::{
    Backend as PublicationBackend, Outcome as PublicationOutcome,
};
use crate::{
    models::collaboration::{AuthorityId, WorkspaceRef},
    platform_http::{
        session_login::test_support as login, SessionLifecycleHttpState, TaskContentIndexHttpState,
    },
    services::auth_service::durable_source::{DurableAuthSource, SessionTaskContentIndexWorkspace},
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
pub(super) const INDEX: &str = "/platform/v1/task-index";
pub(super) const CREATE: &str = "/platform/v1/tasks";
pub(super) const PUBLISH: &str = "/platform/v1/text-artifacts";
pub(super) const REPLACE: &str = "/platform/v1/tasks/e3698554-8eb3-4b6d-884e-8074e1555136/content";
pub(super) const CONTENT: &str = "/platform/v1/tasks/e3698554-8eb3-4b6d-884e-8074e1555136";
pub(super) const TOKEN: &str = "ae0b35cd-c9b8-413c-a892-854b15959f41";

async fn closed_source() -> DurableAuthSource {
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
    capacities: [usize; 6],
) -> PlatformWorkspaceHttpState {
    let source = closed_source().await;
    let workspace = SessionTaskContentIndexWorkspace::new(
        "private_tasks",
        WorkspaceRef {
            authority_id: Uuid::from_u128(4201).try_into().unwrap(),
            workspace_id: Uuid::from_u128(4202).try_into().unwrap(),
        },
    )
    .unwrap();
    PlatformWorkspaceHttpState {
        login: login::state(backend, ORIGIN, capacities[0]),
        lifecycle: SessionLifecycleHttpState::with_capacity(source.clone(), ORIGIN, capacities[1])
            .unwrap(),
        index: TaskContentIndexHttpState::with_capacity(source, workspace, ORIGIN, capacities[2])
            .unwrap(),
        content: crate::platform_http::task_content_read::test_support::state(capacities[3]).await,
        create: crate::platform_http::task_create::test_support::state(
            crate::platform_http::task_create::test_support::Backend::new(
                crate::platform_http::task_create::test_support::Outcome::Unavailable,
            ),
            ORIGIN,
            capacities[4],
        ),
        replace: crate::platform_http::task_content_replace::test_support::state(
            crate::platform_http::task_content_replace::test_support::Backend::new(
                crate::platform_http::task_content_replace::test_support::Outcome::Unavailable,
            ),
            ORIGIN,
            capacities[5],
        ),
        text: None,
        task_text: None,
    }
}

/// Synthetic explicit opt-in only; real tests separately establish one original retained binding.
pub(super) async fn publication_state(
    login: Arc<LoginBackend>,
    publisher: Arc<PublicationBackend>,
    capacities: [usize; 7],
) -> PlatformWorkspaceHttpState {
    let mut state = state(
        login,
        [
            capacities[0],
            capacities[1],
            capacities[2],
            capacities[3],
            capacities[4],
            capacities[5],
        ],
    )
    .await;
    state.text = Some(
        crate::platform_http::text_artifact_publish::test_support::state(
            publisher,
            ORIGIN,
            capacities[6],
        ),
    );
    state
}

pub(super) fn publication_body() -> Body {
    crate::platform_http::text_artifact_publish::test_support::body()
}

pub(super) async fn task_publication_state(
    login: Arc<LoginBackend>,
    publisher: Arc<PublicationBackend>,
    task_publisher: Arc<TaskPublicationBackend>,
    capacities: [usize; 8],
) -> PlatformWorkspaceHttpState {
    let mut state = publication_state(
        login,
        publisher,
        [
            capacities[0],
            capacities[1],
            capacities[2],
            capacities[3],
            capacities[4],
            capacities[5],
            capacities[6],
        ],
    )
    .await;
    state.task_text = Some(
        crate::platform_http::task_text_artifact_publish::test_support::state(
            task_publisher,
            ORIGIN,
            capacities[7],
        ),
    );
    state
}

pub(super) fn task_publication_body() -> Body {
    crate::platform_http::task_text_artifact_publish::test_support::body()
}

pub(super) async fn app(backend: Arc<LoginBackend>) -> Router {
    build_platform_workspace_router(state(backend, [4, 4, 4, 4, 4, 4]).await)
}

pub(super) fn create_body() -> Body {
    crate::platform_http::task_create::test_support::body()
}

pub(super) fn replace_body() -> Body {
    crate::platform_http::task_content_replace::test_support::body()
}

pub(super) fn login_body() -> Body {
    Body::from(
        json!({"schema_version":1,"username":"synthetic-user",
        "password":"public-synthetic-password-only","otp":"123456"})
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
    if [
        SESSION,
        LOGOUT,
        INDEX,
        CONTENT,
        CREATE,
        REPLACE,
        PUBLISH,
        TASK_PUBLISH,
    ]
    .contains(&path.split('?').next().unwrap())
    {
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
            panic!("workspace rejection must not poll request body")
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
    for secret in [TOKEN, "public-synthetic-password-only", "123456"] {
        assert!(!response.json.to_string().contains(secret));
    }
}

pub(super) fn error(response: &Captured, status: StatusCode, code: &str, outcome: &str) {
    assert_eq!(response.status, status);
    private(response);
    assert!(!response.headers.contains_key(header::SET_COOKIE));
    assert_eq!(response.headers[header::CONTENT_TYPE], "application/json");
    assert_eq!(
        response.json,
        json!({"schema_version":1,"error":{"code":code,"outcome":outcome}})
    );
}

pub(super) fn dispatched(response: &Captured) {
    error(
        response,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    );
}

pub(super) fn issued(response: &Captured) {
    assert_eq!(response.status, StatusCode::OK);
    private(response);
    assert_eq!(
        response.json,
        json!({"schema_version":1,"authenticated":true})
    );
    assert_eq!(
        response.headers.get_all(header::SET_COOKIE).iter().count(),
        1
    );
    let cookie = &response.headers[header::SET_COOKIE];
    assert!(cookie.is_sensitive());
    assert_eq!(cookie.to_str().unwrap(), format!("cyanrex_platform_session={TOKEN}; Path=/platform/v1; HttpOnly; SameSite=Strict; Secure"));
}
