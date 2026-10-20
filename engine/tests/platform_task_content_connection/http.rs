use super::fixture::*;
use axum::{
    body::{Body, HttpBody},
    http::{HeaderValue, Request, StatusCode},
    response::Response,
    Router,
};
use cyanrex_engine::{
    platform_http::{
        build_platform_session_router, build_platform_workspace_router,
        build_task_content_read_router, PlatformSessionHttpState, PlatformWorkspaceHttpState,
        TaskContentHttpConfigError, TaskContentReadHttpState,
    },
    services::auth_service::durable_source::SessionTaskContent,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

pub(super) fn path(task: &cyanrex_engine::services::task_store::TaskContentSnapshot) -> String {
    format!("/platform/v1/tasks/{}", task.task.reference.task_id)
}
pub(super) fn projected(content: &SessionTaskContent) -> Value {
    json!({"schema_version":1,"task":content.snapshot.task,"manifest":content.snapshot.manifest,
        "contents":content.contents.iter().map(|value| json!({"artifact":value.revision.reference,
            "text":String::from_utf8(value.bytes.clone()).unwrap()})).collect::<Vec<_>>()})
}
fn request(p: &Prepared<'_>, method: &str, path: &str, owner: usize, body: Body) -> Request<Body> {
    let mut cookie =
        HeaderValue::from_str(&format!("cyanrex_platform_session={}", p.tokens[owner])).unwrap();
    cookie.set_sensitive(true);
    Request::builder()
        .method(method)
        .uri(path)
        .header("origin", p.f.config().origin())
        .header("x-cyanrex-platform-request", "1")
        .header("cookie", cookie)
        .body(body)
        .unwrap()
}
fn unread_body() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<axum::body::Bytes, std::io::Error>>> {
            panic!("rejected route, query, headers or capacity must not poll body")
        },
    ))
}
pub(super) fn headers(response: &Response) {
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["pragma"], "no-cache");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(response.headers()["content-type"], "application/json");
    for key in [
        "set-cookie",
        "retry-after",
        "location",
        "access-control-allow-origin",
    ] {
        assert!(
            !response.headers().contains_key(key),
            "unexpected {key} response header"
        );
    }
}
async fn value(response: Response, status: StatusCode) -> Value {
    assert_eq!(response.status(), status);
    headers(&response);
    serde_json::from_slice(
        &within(response.into_body().collect())
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap()
}
async fn rejected(
    route: &Router,
    request: Request<Body>,
    status: StatusCode,
    code: &str,
    outcome: &str,
) {
    let response = within(route.clone().oneshot(request)).await.unwrap();
    if status == StatusCode::METHOD_NOT_ALLOWED {
        assert_eq!(response.headers()["allow"], "GET");
    }
    assert_eq!(
        value(response, status).await,
        json!({"schema_version":1,"error":{"code":code,"outcome":outcome}})
    );
}
struct Pending(tokio::task::JoinHandle<Result<Response, std::convert::Infallible>>);
impl Drop for Pending {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_content_http_read_only_wire_observed_cancel_and_body_capacity() {
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            let artifact = p
                .artifact(0, "协作讲义😀\n<script>inert text</script>".as_bytes())
                .await;
            let task = p
                .task_manifest(
                    10,
                    0,
                    manifest("不是以代码为中心", vec![artifact.reference]),
                )
                .await;
            let connector = p.connector().await;
            let expected = projected(&p.read(&connector, 0, &task).await.unwrap().unwrap());
            let path = path(&task);
            for bad in [0, 17] {
                assert!(matches!(
                    TaskContentReadHttpState::with_capacity(p.connector().await, bad),
                    Err(TaskContentHttpConfigError::InvalidCapacity)
                ));
                assert!(matches!(
                    PlatformWorkspaceHttpState::with_content_capacities(
                        p.storage().await,
                        1,
                        1,
                        1,
                        bad
                    ),
                    Err(TaskContentHttpConfigError::InvalidCapacity)
                ));
            }
            let old = build_platform_session_router(
                PlatformSessionHttpState::new(p.storage().await).unwrap(),
            );
            rejected(
                &old,
                request(&p, "GET", &path, 0, unread_body()),
                StatusCode::NOT_FOUND,
                "not_found",
                "not_attempted",
            )
            .await;
            let combined = build_platform_workspace_router(
                PlatformWorkspaceHttpState::with_content_capacities(p.storage().await, 1, 1, 1, 1)
                    .unwrap(),
            );
            assert_eq!(
                value(
                    within(
                        combined
                            .clone()
                            .oneshot(request(&p, "GET", &path, 0, Body::empty()))
                    )
                    .await
                    .unwrap(),
                    StatusCode::OK
                )
                .await,
                expected
            );
            let route = build_task_content_read_router(
                TaskContentReadHttpState::with_capacity(connector, 1).unwrap(),
            );
            for method in ["POST", "PUT", "DELETE", "OPTIONS"] {
                rejected(
                    &route,
                    request(&p, method, &path, 0, unread_body()),
                    StatusCode::METHOD_NOT_ALLOWED,
                    "method_not_allowed",
                    "not_attempted",
                )
                .await;
            }
            for missing in [
                "/platform/v1/tasks".to_owned(),
                format!("{path}/content"),
                format!("{path}/status"),
                format!("{path}/extra"),
            ] {
                rejected(
                    &route,
                    request(&p, "GET", &missing, 0, unread_body()),
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "not_attempted",
                )
                .await;
            }
            for bad in [
                format!("{path}?owner=other"),
                format!("{path}?"),
                "/platform/v1/tasks/not-an-id".into(),
            ] {
                rejected(
                    &route,
                    request(&p, "GET", &bad, 0, unread_body()),
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    "not_attempted",
                )
                .await;
            }
            let mut no_marker = request(&p, "GET", &path, 0, unread_body());
            no_marker.headers_mut().remove("x-cyanrex-platform-request");
            rejected(
                &route,
                no_marker,
                StatusCode::FORBIDDEN,
                "request_forbidden",
                "not_attempted",
            )
            .await;
            let mut foreign_origin = request(&p, "GET", &path, 0, unread_body());
            foreign_origin.headers_mut().insert(
                "origin",
                HeaderValue::from_static("https://elsewhere.example"),
            );
            rejected(
                &route,
                foreign_origin,
                StatusCode::FORBIDDEN,
                "request_forbidden",
                "not_attempted",
            )
            .await;
            rejected(
                &route,
                request(&p, "GET", &path, 1, Body::empty()),
                StatusCode::NOT_FOUND,
                "not_found",
                "unconfirmed",
            )
            .await;
            rejected(
                &route,
                request(&p, "GET", &path, 0, Body::from("not empty")),
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "not_attempted",
            )
            .await;
            let before = snapshot(f).await;
            let files = p.files();
            let (fence, pid) = super::waits::holder(f, task.task.reference).await;
            let mut pending = Pending(tokio::spawn(route.clone().oneshot(request(
                &p,
                "GET",
                &path,
                0,
                Body::empty(),
            ))));
            let waited = super::waits::blocked(f, pid).await;
            assert!(
                waited && !pending.0.is_finished(),
                "cancel at the actual private Task row fence"
            );
            rejected(
                &route,
                request(&p, "GET", &path, 0, unread_body()),
                StatusCode::SERVICE_UNAVAILABLE,
                "capacity_exhausted",
                "not_attempted",
            )
            .await;
            pending.0.abort();
            assert!((&mut pending.0).await.unwrap_err().is_cancelled());
            super::waits::release(fence).await;
            assert!(super::storage_helpers::no_connections(f).await);
            let response =
                within(
                    route
                        .clone()
                        .oneshot(request(&p, "GET", &path, 0, Body::empty())),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            headers(&response);
            rejected(
                &route,
                request(&p, "GET", &path, 0, unread_body()),
                StatusCode::SERVICE_UNAVAILABLE,
                "capacity_exhausted",
                "not_attempted",
            )
            .await;
            let mut body = response.into_body();
            let bytes = within(body.frame())
                .await
                .unwrap()
                .unwrap()
                .into_data()
                .unwrap();
            assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap(), expected);
            assert!(
                body.is_end_stream(),
                "last JSON frame releases capacity while Body is retained"
            );
            let response =
                within(
                    route
                        .clone()
                        .oneshot(request(&p, "GET", &path, 0, Body::empty())),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            rejected(
                &route,
                request(&p, "GET", &path, 0, unread_body()),
                StatusCode::SERVICE_UNAVAILABLE,
                "capacity_exhausted",
                "not_attempted",
            )
            .await;
            drop(response);
            assert_eq!(
                value(
                    within(
                        route
                            .clone()
                            .oneshot(request(&p, "GET", &path, 0, Body::empty()))
                    )
                    .await
                    .unwrap(),
                    StatusCode::OK
                )
                .await,
                expected
            );
            assert!(before == snapshot(f).await && files == p.files());
            drop((old, combined, route, pending, body));
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })
    })
    .await;
}
