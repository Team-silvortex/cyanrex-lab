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
        build_task_content_index_router, PlatformSessionHttpState, PlatformWorkspaceHttpState,
        TaskContentHttpConfigError, TaskContentIndexHttpState,
    },
    services::task_store::TaskContentSnapshot,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

const COLLECTION: &str = "/platform/v1/task-index";
fn request(p: &Prepared<'_>, path: &str, body: Body) -> Request<Body> {
    let mut cookie =
        HeaderValue::from_str(&format!("cyanrex_platform_session={}", p.tokens[0])).unwrap();
    cookie.set_sensitive(true);
    Request::builder()
        .method("GET")
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
            panic!("rejected admission/query must not poll the request body")
        },
    ))
}
fn headers(response: &Response) {
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
async fn rejected(route: &Router, request: Request<Body>, code: &str, status: StatusCode) {
    assert_eq!(
        value(
            within(route.clone().oneshot(request)).await.unwrap(),
            status
        )
        .await,
        json!({"schema_version":1,"error":{"code":code,"outcome":"not_attempted"}})
    );
}
fn projected(task: &TaskContentSnapshot) -> Value {
    let item = entry(task);
    json!({"schema_version":1,"entries":[{"reference":item.reference,"revision":item.revision,
        "title":item.title,"status":item.status,"definition_ref":null,"updated_at":item.updated_at,
        "payload_items":item.payload_items}],"next_after":null})
}
struct Pending(tokio::task::JoinHandle<Result<Response, std::convert::Infallible>>);
impl Drop for Pending {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_index_http_observed_wait_cancellation_private_wire_and_body_capacity(
) {
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            let task = p.task(10, 0, "Owner-private teaching plan").await;
            for bad in [0, 17] {
                assert!(matches!(
                    TaskContentIndexHttpState::with_platform_storage(p.storage().await, bad),
                    Err(TaskContentHttpConfigError::InvalidCapacity)
                ));
                for capacities in [(bad, 1, 1), (1, bad, 1), (1, 1, bad)] {
                    assert!(matches!(
                        PlatformWorkspaceHttpState::with_capacities(
                            p.storage().await,
                            capacities.0,
                            capacities.1,
                            capacities.2
                        ),
                        Err(TaskContentHttpConfigError::InvalidCapacity)
                    ));
                }
            }
            let old = build_platform_session_router(
                PlatformSessionHttpState::new(p.storage().await).unwrap(),
            );
            rejected(
                &old,
                request(&p, COLLECTION, unread_body()),
                "not_found",
                StatusCode::NOT_FOUND,
            )
            .await;
            let combined = build_platform_workspace_router(
                PlatformWorkspaceHttpState::new(p.storage().await).unwrap(),
            );
            assert_eq!(
                value(
                    within(
                        combined
                            .clone()
                            .oneshot(request(&p, COLLECTION, Body::empty()))
                    )
                    .await
                    .unwrap(),
                    StatusCode::OK
                )
                .await,
                projected(&task)
            );
            drop((old, combined));
            let route = build_task_content_index_router(
                TaskContentIndexHttpState::with_platform_storage(p.storage().await, 1).unwrap(),
            );
            rejected(
                &route,
                request(&p, "/platform/v1/task-index?owner=other", unread_body()),
                "invalid_request",
                StatusCode::BAD_REQUEST,
            )
            .await;
            let before = snapshot(f).await;
            let (fence, pid) = super::waits::holder(f, task.task.reference).await;
            let req = request(&p, COLLECTION, Body::empty());
            let mut pending = Pending(tokio::spawn(route.clone().oneshot(req)));
            let waited = super::waits::blocked(f, pid).await;
            assert!(
                waited && !pending.0.is_finished(),
                "cancel only at an observed real owned Task row wait"
            );
            rejected(
                &route,
                request(&p, COLLECTION, unread_body()),
                "capacity_exhausted",
                StatusCode::SERVICE_UNAVAILABLE,
            )
            .await;
            pending.0.abort();
            assert!((&mut pending.0).await.unwrap_err().is_cancelled());
            super::waits::release(fence).await;
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(before == snapshot(f).await);
            let response = within(
                route
                    .clone()
                    .oneshot(request(&p, COLLECTION, Body::empty())),
            )
            .await
            .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            headers(&response);
            rejected(
                &route,
                request(&p, COLLECTION, unread_body()),
                "capacity_exhausted",
                StatusCode::SERVICE_UNAVAILABLE,
            )
            .await;
            let mut body = response.into_body();
            let frame = within(body.frame())
                .await
                .unwrap()
                .unwrap()
                .into_data()
                .unwrap();
            assert_eq!(
                serde_json::from_slice::<Value>(&frame).unwrap(),
                projected(&task)
            );
            assert!(
                body.is_end_stream(),
                "consuming the final JSON frame releases capacity while Body still exists"
            );
            let response = within(
                route
                    .clone()
                    .oneshot(request(&p, COLLECTION, Body::empty())),
            )
            .await
            .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            headers(&response);
            rejected(
                &route,
                request(&p, COLLECTION, unread_body()),
                "capacity_exhausted",
                StatusCode::SERVICE_UNAVAILABLE,
            )
            .await;
            drop(response);
            assert_eq!(
                value(
                    within(
                        route
                            .clone()
                            .oneshot(request(&p, COLLECTION, Body::empty()))
                    )
                    .await
                    .unwrap(),
                    StatusCode::OK
                )
                .await,
                projected(&task)
            );
            assert!(before == snapshot(f).await);
            drop((pending, route, body));
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })
    })
    .await;
}
