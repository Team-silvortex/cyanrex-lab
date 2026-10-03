use super::*;

pub const ORIGIN: &str = "https://platform.example";
pub const COOKIE: &str = "cyanrex_platform_session";
pub const MARKER: &str = "x-cyanrex-platform-request";
pub const TASK: &str = "e3698554-8eb3-4b6d-884e-8074e1555136";
pub const TOKEN: &str = "ae0b35cd-c9b8-413c-a892-854b15959f41";
pub const COLLECTION: &str = "/platform/v1/tasks";

pub struct Offline {
    pub app: Router,
    pub source: DurableAuthSource,
    pub workspace: SessionTaskContentWorkspace,
    root: PathBuf,
}
impl Offline {
    pub async fn new() -> Self {
        let source = input_fixture::unavailable_source().await;
        let root = input_fixture::private_directory();
        let artifact = source
            .open_artifact_workspace("private_artifacts", scope(), &root)
            .unwrap();
        let workspace =
            SessionTaskContentWorkspace::new("private_tasks", scope(), artifact).unwrap();
        let app = build_task_content_router(
            TaskContentHttpState::new(source.clone(), workspace.clone(), ORIGIN).unwrap(),
        );
        Self {
            app,
            source,
            workspace,
            root,
        }
    }
}
impl Drop for Offline {
    fn drop(&mut self) {
        assert_eq!(fs::read_dir(&self.root).unwrap().count(), 0);
        fs::remove_dir(&self.root).unwrap();
    }
}
pub fn app(f: &Fixture) -> Router {
    build_task_content_router(
        TaskContentHttpState::new(f.auth.source.clone(), f.workspace.clone(), ORIGIN).unwrap(),
    )
}
pub fn request(method: &str, path: &str, token: &str, body: impl Into<Body>) -> Request<Body> {
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
pub fn create_body(task: TaskId, manifest: &TaskContentManifest) -> String {
    json!({"schema_version":1,"task_id":task,"manifest":manifest}).to_string()
}
pub fn replace_body(task: &TaskContentSnapshot, manifest: &TaskContentManifest) -> String {
    json!({"schema_version":1,"expected_revision":task.task.revision,"manifest":manifest})
        .to_string()
}
pub fn task_path(task: TaskId) -> String {
    format!("{COLLECTION}/{task}")
}
pub fn headers(response: &axum::response::Response) {
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert!(!response.headers().contains_key("set-cookie"));
    assert!(!response.headers().contains_key("retry-after"));
    assert!(!response
        .headers()
        .contains_key("access-control-allow-origin"));
}
pub async fn response(app: Router, req: Request<Body>, expected: StatusCode) -> Value {
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), expected);
    headers(&response);
    assert_eq!(response.headers()["content-type"], "application/json");
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}
pub async fn error(
    app: Router,
    req: Request<Body>,
    expected: StatusCode,
    code: &str,
    outcome: &str,
) {
    let value = response(app, req, expected).await;
    assert_eq!(
        value,
        json!({"schema_version":1,"error":{"code":code,"outcome":outcome}})
    );
}
pub async fn offline_error(f: &Offline, req: Request<Body>, expected: StatusCode, code: &str) {
    error(f.app.clone(), req, expected, code, "not_attempted").await;
}
pub fn snapshot(value: &Value) -> TaskContentSnapshot {
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value.as_object().unwrap().len(), 3);
    TaskContentSnapshot {
        task: serde_json::from_value(value["task"].clone()).unwrap(),
        manifest: serde_json::from_value(value["manifest"].clone()).unwrap(),
    }
}
pub async fn create_http(f: &Fixture, value: &TaskContentManifest) -> TaskContentSnapshot {
    let result = response(
        app(f),
        request(
            "POST",
            COLLECTION,
            &f.auth.learner_token,
            create_body(id(), value),
        ),
        StatusCode::CREATED,
    )
    .await;
    snapshot(&result)
}
