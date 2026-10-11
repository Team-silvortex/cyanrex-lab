use super::*;

pub const ORIGIN: &str = "https://platform.example";
pub const COOKIE: &str = "cyanrex_platform_session";
pub const MARKER: &str = "x-cyanrex-platform-request";
pub const TOKEN: &str = "ae0b35cd-c9b8-413c-a892-854b15959f41";
pub const COLLECTION: &str = "/platform/v1/task-index";
pub const TASK: &str = "e3698554-8eb3-4b6d-884e-8074e1555136";
pub use index_fixture::{catalog, configured, content, entry, index, values};

pub struct Offline {
    pub app: Router,
    pub source: DurableAuthSource,
    pub workspace: SessionTaskContentIndexWorkspace,
}
impl Offline {
    pub async fn new() -> Self {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://synthetic@127.0.0.1/unused")
            .unwrap();
        pool.close().await;
        let source = DurableAuthSource::new(pool, authority());
        let workspace = SessionTaskContentIndexWorkspace::new("private_tasks", scope()).unwrap();
        let app = build_task_content_index_router(
            TaskContentIndexHttpState::new(source.clone(), workspace.clone(), ORIGIN).unwrap(),
        );
        Self {
            app,
            source,
            workspace,
        }
    }
    pub fn state(&self, capacity: usize) -> TaskContentIndexHttpState {
        TaskContentIndexHttpState::with_capacity(
            self.source.clone(),
            self.workspace.clone(),
            ORIGIN,
            capacity,
        )
        .unwrap()
    }
}
pub fn request(method: &str, path: &str, body: impl Into<Body>) -> Request<Body> {
    request_with_token(method, path, TOKEN, body)
}
pub fn request_with_token(
    method: &str,
    path: &str,
    token: &str,
    body: impl Into<Body>,
) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header(MARKER, "1")
        .header("origin", ORIGIN)
        .header("cookie", format!("{COOKIE}={token}"))
        .header("content-type", "application/json")
        .body(body.into())
        .unwrap()
}
pub fn headers(response: &axum::response::Response) {
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    for header in ["set-cookie", "retry-after", "access-control-allow-origin"] {
        assert!(!response.headers().contains_key(header));
    }
}
pub async fn error(app: Router, req: Request<Body>, status: StatusCode, code: &str, outcome: &str) {
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), status);
    headers(&response);
    assert_eq!(response.headers()["content-type"], "application/json");
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap(),
        json!({"schema_version":1,"error":{"code":code,"outcome":outcome}})
    );
}
pub async fn rejected(f: &Offline, req: Request<Body>, status: StatusCode, code: &str) {
    error(f.app.clone(), req, status, code, "not_attempted").await;
}
pub async fn available(app: Router) {
    error(
        app,
        request("GET", COLLECTION, Body::empty()),
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
}
pub fn unread_body() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<axum::body::Bytes, std::io::Error>>> {
            panic!("rejected route/query/header/admission must not poll the body")
        },
    ))
}
pub fn app(f: &Fixture, workspace: &SessionTaskContentIndexWorkspace) -> Router {
    build_task_content_index_router(
        TaskContentIndexHttpState::new(f.auth.source.clone(), workspace.clone(), ORIGIN).unwrap(),
    )
}
pub fn bounded_app(f: &Fixture, workspace: &SessionTaskContentIndexWorkspace) -> Router {
    build_task_content_index_router(
        TaskContentIndexHttpState::with_capacity(
            f.auth.source.clone(),
            workspace.clone(),
            ORIGIN,
            1,
        )
        .unwrap(),
    )
}
pub async fn response_value(response: axum::response::Response, status: StatusCode) -> Value {
    assert_eq!(response.status(), status);
    headers(&response);
    assert_eq!(response.headers()["content-type"], "application/json");
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}
pub async fn read(f: &Fixture, route: &Router, token: &str, path: &str) -> Value {
    let before = index_sql_fixture::snapshot(f).await;
    let response = route
        .clone()
        .oneshot(request_with_token("GET", path, token, Body::empty()))
        .await
        .unwrap();
    let value = response_value(response, StatusCode::OK).await;
    assert_eq!(index_sql_fixture::snapshot(f).await, before);
    assert_eq!(value.as_object().unwrap().len(), 3);
    assert_eq!(value["schema_version"], 1);
    assert!(value["entries"].is_array());
    value
}
pub async fn denied(
    f: &Fixture,
    route: &Router,
    req: Request<Body>,
    status: StatusCode,
    code: &str,
    outcome: &str,
) {
    let before = index_sql_fixture::snapshot(f).await;
    error(route.clone(), req, status, code, outcome).await;
    assert_eq!(index_sql_fixture::snapshot(f).await, before);
}
pub fn projected(task: &TaskContentSnapshot) -> Value {
    let item = entry(task);
    json!({"reference":item.reference,"revision":item.revision,"title":item.title,
        "status":item.status,"definition_ref":item.definition_ref,"updated_at":item.updated_at,
        "payload_items":item.payload_items})
}
pub fn assert_page(value: &Value, tasks: &[TaskContentSnapshot], next: Option<TaskRef>) {
    assert_eq!(
        value,
        &json!({"schema_version":1,
            "entries":tasks.iter().map(projected).collect::<Vec<_>>(),
            "next_after":next})
    );
    for entry in value["entries"].as_array().unwrap() {
        assert_eq!(entry.as_object().unwrap().len(), 7);
    }
}
pub struct Pending(
    pub tokio::task::JoinHandle<Result<axum::response::Response, std::convert::Infallible>>,
);
impl Drop for Pending {
    fn drop(&mut self) {
        // A failed assertion must not detach an authorized SQL reader from its disposable fixture.
        self.0.abort();
    }
}
