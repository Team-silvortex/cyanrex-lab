pub(super) use super::publication_fixture::{
    artifact_id, manifest, revision_id, snapshot, task_counts, task_id, token_hash, within,
    Fixture, Prepared,
};
pub(super) use super::publication_http::{
    call, error, headers, request, unread, wire as unbound_wire, PATH,
};
use axum::{
    body::Body,
    http::{HeaderValue, Request, StatusCode},
    Router,
};
use cyanrex_engine::{
    platform_http::{build_platform_workspace_router, PlatformWorkspaceHttpState},
    services::task_store::{TaskContentSnapshot, TaskContentStore},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

pub(super) async fn router(p: &Prepared<'_>, capacities: [usize; 8]) -> Router {
    let [login, lifecycle, index, content, create, replace, publish, task_publish] = capacities;
    build_platform_workspace_router(
        PlatformWorkspaceHttpState::with_task_text_publication_capacities(
            p.storage().await,
            login,
            lifecycle,
            index,
            content,
            create,
            replace,
            publish,
            task_publish,
        )
        .unwrap(),
    )
}
pub(super) fn path(old: &TaskContentSnapshot) -> String {
    format!(
        "/platform/v1/tasks/{}/text-artifacts",
        old.task.reference.task_id
    )
}
pub(super) fn payload(old: &TaskContentSnapshot, n: u128, text: &str) -> Value {
    json!({"schema_version":1,"expected":{"task":old.task,"manifest":old.manifest},
        "artifact_id":artifact_id(n),"revision_id":revision_id(n),"text":text})
}
pub(super) fn wire(old: &TaskContentSnapshot, n: u128, text: &str) -> Body {
    Body::from(payload(old, n, text).to_string())
}
pub(super) fn request_token(
    p: &Prepared<'_>,
    token: &str,
    method: &str,
    path: &str,
    body: Body,
) -> Request<Body> {
    let mut result = request(p, 0, method, path, body);
    let mut cookie = HeaderValue::from_str(&format!("cyanrex_platform_session={token}")).unwrap();
    cookie.set_sensitive(true);
    result.headers_mut().insert("cookie", cookie);
    result
}
pub(super) async fn current(p: &Prepared<'_>, number: u128, owner: usize) -> TaskContentSnapshot {
    super::schema::path(p.f, &p.f.task).await;
    let value = TaskContentStore::new(p.f.owner().clone(), p.f.config().scope())
        .get(
            cyanrex_engine::models::collaboration::TaskRef {
                workspace: p.f.config().scope(),
                task_id: task_id(number),
            },
            p.owners[owner],
        )
        .await
        .unwrap()
        .unwrap();
    super::schema::path(p.f, &p.f.source).await;
    value
}
pub(super) async fn unchanged(
    p: &Prepared<'_>,
    number: u128,
    owner: usize,
    expected: &TaskContentSnapshot,
) {
    assert_eq!(current(p, number, owner).await, *expected);
}
pub(super) async fn login(p: &Prepared<'_>, route: &Router) -> String {
    super::native_fixture::login_profile(p).await;
    let mut req = request(
        p,
        0,
        "POST",
        "/platform/v1/session/login",
        Body::from(super::native_fixture::credentials().to_string()),
    );
    req.headers_mut().remove("cookie");
    let response = within(route.clone().oneshot(req)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    for (key, expected) in [
        ("cache-control", "no-store"),
        ("pragma", "no-cache"),
        ("x-content-type-options", "nosniff"),
        ("content-type", "application/json"),
    ] {
        assert_eq!(response.headers()[key], expected);
    }
    assert_eq!(response.headers().get_all("set-cookie").iter().count(), 1);
    let (pair, attributes) = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split_once(';')
        .unwrap();
    assert_eq!(
        attributes,
        " Path=/platform/v1; HttpOnly; SameSite=Strict; Secure"
    );
    let token = pair
        .strip_prefix("cyanrex_platform_session=")
        .unwrap()
        .to_owned();
    let parsed = uuid::Uuid::parse_str(&token).unwrap();
    assert_eq!(parsed.to_string(), token);
    let value: Value = serde_json::from_slice(
        &within(response.into_body().collect())
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    assert_eq!(value, json!({"schema_version":1,"authenticated":true}));
    token
}
