//! Closed Sources prove transport dispatch only, not successful retained Task content access.
use super::{Backend, TaskContentReadHttpState};
use crate::{
    models::collaboration::{AuthorityId, WorkspaceRef},
    platform_http::{build_task_content_read_router, TaskContentHttpConfigError},
    services::auth_service::durable_source::{DurableAuthSource, SessionTaskContentWorkspace},
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
use std::{os::unix::fs::DirBuilderExt, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::Notify;
use tower::ServiceExt;
use uuid::Uuid;

pub(in crate::platform_http) const ORIGIN: &str = "https://platform.example";
pub(in crate::platform_http) const TASK: &str =
    "/platform/v1/tasks/e3698554-8eb3-4b6d-884e-8074e1555136";
const TOKEN: &str = "ae0b35cd-c9b8-413c-a892-854b15959f41";
pub(super) const COOKIE: &str = "cyanrex_platform_session=ae0b35cd-c9b8-413c-a892-854b15959f41";

pub(super) struct TestRoot(PathBuf);
impl Drop for TestRoot {
    fn drop(&mut self) {
        assert_eq!(std::fs::read_dir(&self.0).unwrap().count(), 0);
        std::fs::remove_dir(&self.0).unwrap();
    }
}

pub(super) struct Fixture {
    source: DurableAuthSource,
    workspace: SessionTaskContentWorkspace,
    root: Arc<TestRoot>,
}

impl Fixture {
    pub(super) async fn new() -> Self {
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
        let authority_id = AuthorityId::try_from(Uuid::from_u128(4201)).unwrap();
        let scope = WorkspaceRef {
            authority_id,
            workspace_id: Uuid::from_u128(4202).try_into().unwrap(),
        };
        let source = DurableAuthSource::new(pool, authority_id);
        let root = Arc::new(TestRoot(
            std::env::temp_dir().join(format!("cyanrex-content-read-http-{}", Uuid::new_v4())),
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root.0)
            .unwrap();
        let artifacts = source
            .open_artifact_workspace("private_artifacts", scope, &root.0)
            .unwrap();
        let workspace =
            SessionTaskContentWorkspace::new("private_tasks", scope, artifacts).unwrap();
        Self {
            source,
            workspace,
            root,
        }
    }

    pub(super) fn state(
        &self,
        origin: &str,
        capacity: usize,
    ) -> Result<TaskContentReadHttpState, TaskContentHttpConfigError> {
        TaskContentReadHttpState::configured(
            Backend::Source {
                source: self.source.clone(),
                workspace: self.workspace.clone(),
                _root: Arc::clone(&self.root),
            },
            origin,
            capacity,
        )
    }
}

pub(in crate::platform_http) async fn state(capacity: usize) -> TaskContentReadHttpState {
    Fixture::new().await.state(ORIGIN, capacity).unwrap()
}

pub(super) async fn app(capacity: usize) -> Router {
    build_task_content_read_router(state(capacity).await)
}

pub(super) fn request(method: &str, path: &str, body: Body) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("x-cyanrex-platform-request", "1")
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, COOKIE)
        .body(body)
        .unwrap();
    request
        .headers_mut()
        .get_mut(header::COOKIE)
        .unwrap()
        .set_sensitive(true);
    request
}

pub(super) fn untouched() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<Bytes, std::io::Error>>> {
            panic!("content read rejection must not poll request body")
        },
    ))
}

pub(super) fn pending_body() -> (Body, Arc<Notify>) {
    let entered = Arc::new(Notify::new());
    let observed = entered.clone();
    let body = Body::from_stream(futures_util::stream::poll_fn(move |_| {
        observed.notify_one();
        std::task::Poll::<Option<Result<Bytes, std::io::Error>>>::Pending
    }));
    (body, entered)
}

pub(super) struct Captured {
    pub(super) status: StatusCode,
    pub(super) headers: HeaderMap,
    pub(super) json: Value,
}

pub(super) async fn capture(app: Router, request: Request<Body>) -> Captured {
    capture_response(
        tokio::time::timeout(Duration::from_secs(1), app.oneshot(request))
            .await
            .unwrap()
            .unwrap(),
    )
    .await
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
        "set-cookie",
        "location",
        "retry-after",
        "access-control-allow-origin",
        "access-control-allow-credentials",
    ] {
        assert!(!response.headers.contains_key(name));
    }
    assert!(!response.json.to_string().contains(TOKEN));
}

pub(super) fn error(response: &Captured, status: StatusCode, code: &str, outcome: &str) {
    assert_eq!(response.status, status);
    private(response);
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

pub(super) fn exhausted(response: &Captured) {
    error(
        response,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
}
