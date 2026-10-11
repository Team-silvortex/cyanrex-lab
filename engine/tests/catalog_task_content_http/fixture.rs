use super::*;

pub use super::manual_http_fixture::{
    error, headers, replace_body, request, response, snapshot, COOKIE, MARKER, ORIGIN, TASK, TOKEN,
};
pub const COLLECTION: &str = "/platform/v1/catalog-tasks";

pub fn definition(
    package: &str,
    package_version: u64,
    version: u64,
    bound: bool,
) -> TaskDefinition {
    TaskDefinition {
        schema_version: CoreSchemaVersion,
        reference: TaskDefinitionRef {
            package: VersionedName {
                name: package.parse().unwrap(),
                version: package_version.try_into().unwrap(),
            },
            name: format!("{package}.prepare").parse().unwrap(),
            version: version.try_into().unwrap(),
        },
        title: format!("{package} task"),
        summary: format!("Frozen package {package_version}, definition {version}"),
        evidence_schema: bound.then(|| VersionedName {
            name: format!("{package}.text").parse().unwrap(),
            version: 1.try_into().unwrap(),
        }),
        assessment_policy: bound.then(|| VersionedName {
            name: format!("{package}.editorial").parse().unwrap(),
            version: 1.try_into().unwrap(),
        }),
    }
}
pub fn definitions(values: &[TaskDefinition]) -> TaskDefinitionCatalog {
    let catalogs: Vec<_> = values
        .iter()
        .map(|value| {
            TaskDefinitionCatalog::new(value.reference.package.clone(), vec![value.clone()])
                .unwrap()
        })
        .collect();
    TaskDefinitionCatalog::compose(&catalogs).unwrap()
}
pub fn values() -> Vec<TaskDefinition> {
    vec![
        definition("sample.document", 1, 1, true),
        definition("sample.document", 2, 2, false),
        definition("sample.memo", 1, 3, false),
    ]
}
pub struct Offline {
    pub app: Router,
    pub source: DurableAuthSource,
    pub workspace: SessionCatalogTaskContentWorkspace,
    root: PathBuf,
}
impl Offline {
    pub async fn new() -> Self {
        let source = input_fixture::unavailable_source().await;
        let root = input_fixture::private_directory();
        let artifact = source
            .open_artifact_workspace("private_artifacts", scope(), &root)
            .unwrap();
        let workspace = SessionCatalogTaskContentWorkspace::from_definitions(
            "private_tasks",
            scope(),
            artifact,
            &definitions(&values()),
        )
        .unwrap();
        let app = build_catalog_task_content_router(
            CatalogTaskContentHttpState::new(source.clone(), workspace.clone(), ORIGIN).unwrap(),
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
pub fn create_body(
    task: TaskId,
    reference: &TaskDefinitionRef,
    manifest: &TaskContentManifest,
) -> String {
    json!({"schema_version":1,"task_id":task,"definition_ref":reference,"manifest":manifest})
        .to_string()
}
pub fn task_path(task: TaskId) -> String {
    format!("{COLLECTION}/{task}")
}
pub async fn offline_error(f: &Offline, req: Request<Body>, expected: StatusCode, code: &str) {
    error(f.app.clone(), req, expected, code, "not_attempted").await;
}

pub fn workspace(f: &Fixture, values: &[TaskDefinition]) -> SessionCatalogTaskContentWorkspace {
    SessionCatalogTaskContentWorkspace::from_definitions(
        &f.task_schema,
        scope(),
        f.artifact_workspace.clone(),
        &definitions(values),
    )
    .unwrap()
}
pub fn app(f: &Fixture, workspace: &SessionCatalogTaskContentWorkspace) -> Router {
    build_catalog_task_content_router(
        CatalogTaskContentHttpState::new(f.auth.source.clone(), workspace.clone(), ORIGIN).unwrap(),
    )
}
pub fn bounded_app(f: &Fixture, workspace: &SessionCatalogTaskContentWorkspace) -> Router {
    build_catalog_task_content_router(
        CatalogTaskContentHttpState::with_capacity(
            f.auth.source.clone(),
            workspace.clone(),
            ORIGIN,
            1,
        )
        .unwrap(),
    )
}
pub fn status_body(task: &TaskContentSnapshot, status: &str) -> String {
    json!({"schema_version":1,"expected_revision":task.task.revision,"status":status}).to_string()
}
pub async fn create(
    f: &Fixture,
    route: &Router,
    definition: &TaskDefinition,
    value: &TaskContentManifest,
) -> TaskContentSnapshot {
    snapshot(
        &response(
            route.clone(),
            request(
                "POST",
                COLLECTION,
                &f.auth.learner_token,
                create_body(id(), &definition.reference, value),
            ),
            StatusCode::CREATED,
        )
        .await,
    )
}
pub async fn read(f: &Fixture, route: &Router, token: &str, task: &TaskContentSnapshot) -> Value {
    let value = response(
        route.clone(),
        request(
            "GET",
            &task_path(task.task.reference.task_id),
            token,
            Body::empty(),
        ),
        StatusCode::OK,
    )
    .await;
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value.as_object().unwrap().len(), 4);
    assert_eq!(value["task"], json!(task.task));
    assert_eq!(value["manifest"], json!(task.manifest));
    assert_eq!(task.task.reference.workspace, scope());
    let _ = f;
    value
}
pub async fn pair() -> (
    Fixture,
    SessionCatalogTaskContentWorkspace,
    Router,
    TaskContentSnapshot,
    ArtifactRevision,
    ArtifactRevision,
) {
    let f = Fixture::ready().await;
    let workspace = workspace(&f, &values());
    let route = app(&f, &workspace);
    let old = f
        .create_artifact(&f.auth.learner_token, b"Original input")
        .await;
    let new = f
        .create_artifact(&f.auth.learner_token, b"Replacement input")
        .await;
    let task = create(
        &f,
        &route,
        &values()[0],
        &manifest("Private catalogue content", vec![old.reference.clone()]),
    )
    .await;
    (f, workspace, route, task, old, new)
}

#[derive(Debug, PartialEq, Eq)]
pub struct Observed {
    rows: Vec<(String, Vec<String>)>,
    files: Vec<(String, Vec<u8>)>,
}
pub async fn observed(f: &Fixture) -> Observed {
    let mut rows = Vec::new();
    for (pool, tables) in [
        (
            &f.task_pool,
            vec![
                "collaboration_task_schema",
                "collaboration_tasks",
                "collaboration_task_outbox",
            ],
        ),
        (
            &f.artifact_pool,
            vec![
                "collaboration_artifact_schema",
                "collaboration_artifacts",
                "collaboration_artifact_revisions",
                "collaboration_artifact_outbox",
            ],
        ),
    ] {
        for table in tables {
            let mut values: Vec<String> = query(&format!(
                "SELECT row_to_json(t)::text AS value FROM {table} t"
            ))
            .fetch_all(pool)
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.get("value"))
            .collect();
            values.sort();
            rows.push((table.into(), values));
        }
    }
    let mut files: Vec<_> = fs::read_dir(&f.directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().to_string_lossy().into_owned(),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect();
    files.sort();
    Observed { rows, files }
}
pub async fn denied(
    f: &Fixture,
    route: &Router,
    req: Request<Body>,
    status: StatusCode,
    code: &str,
) {
    let before = observed(f).await;
    error(route.clone(), req, status, code, "unconfirmed").await;
    assert_eq!(
        observed(f).await,
        before,
        "every service rejection preserves all seven business tables and file bytes"
    );
}
pub fn mutations(
    task: &TaskContentSnapshot,
    token: &str,
    value: &TaskContentManifest,
    definition: &TaskDefinition,
) -> Vec<Request<Body>> {
    let path = task_path(task.task.reference.task_id);
    vec![
        request(
            "POST",
            COLLECTION,
            token,
            create_body(id(), &definition.reference, value),
        ),
        request(
            "PUT",
            &format!("{path}/content"),
            token,
            replace_body(task, value),
        ),
        request(
            "POST",
            &format!("{path}/status"),
            token,
            status_body(task, "ready"),
        ),
    ]
}
pub fn saved_requests(task: &TaskContentSnapshot, token: &str) -> Vec<Request<Body>> {
    let path = task_path(task.task.reference.task_id);
    vec![
        request("GET", &path, token, Body::empty()),
        request(
            "PUT",
            &format!("{path}/content"),
            token,
            replace_body(
                task,
                &manifest("Cannot erase invalid saved content", vec![]),
            ),
        ),
        request(
            "POST",
            &format!("{path}/status"),
            token,
            status_body(task, "ready"),
        ),
    ]
}

pub type Tx<'a> = sqlx_core::transaction::Transaction<'a, sqlx_postgres::Postgres>;
pub async fn backend(tx: &mut Tx<'_>) -> i32 {
    query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
        .get("pid")
}
pub async fn blocked_by(f: &Fixture, blocker: i32) {
    tokio::time::timeout(Duration::from_secs(3),async {
        loop {
            let count: i64 = query("SELECT count(*) AS n FROM pg_stat_activity WHERE datname = current_database() AND $1 = ANY(pg_blocking_pids(pid))").bind(blocker).fetch_one(&f.auth.base.admin).await.unwrap().get("n");
            if count == 1 { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("HTTP operation did not reach its exact SQL wait barrier");
}
pub struct Pending(
    pub tokio::task::JoinHandle<Result<axum::response::Response, std::convert::Infallible>>,
);
impl Drop for Pending {
    fn drop(&mut self) {
        self.0.abort();
    }
}
pub async fn finish(pending: &mut Pending, status: StatusCode, code: &str) {
    let response = tokio::time::timeout(Duration::from_secs(3), &mut pending.0)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    headers(&response);
    assert_eq!(response.status(), status);
    let value: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        value,
        json!({"schema_version":1,"error":{"code":code,"outcome":"unconfirmed"}})
    );
}
