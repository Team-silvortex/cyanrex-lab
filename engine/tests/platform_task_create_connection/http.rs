use super::fixture::*;
use axum::{
    body::{Body, HttpBody},
    http::{HeaderValue, Request, StatusCode},
    response::Response,
    Router,
};
use cyanrex_engine::platform_http::{
    build_platform_session_router, build_platform_workspace_router, build_task_create_router,
    PlatformSessionHttpState, PlatformWorkspaceHttpState, TaskContentHttpConfigError,
    TaskCreateHttpState,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

fn request(p: &Prepared<'_>, method: &str, path: &str, body: Body) -> Request<Body> {
    let mut cookie =
        HeaderValue::from_str(&format!("cyanrex_platform_session={}", p.tokens[0])).unwrap();
    cookie.set_sensitive(true);
    Request::builder()
        .method(method)
        .uri(path)
        .header("origin", p.f.config().origin())
        .header("x-cyanrex-platform-request", "1")
        .header("cookie", cookie)
        .header("content-type", "application/json")
        .body(body)
        .unwrap()
}
fn wire(id: u128) -> Body {
    Body::from(
        json!({"schema_version":1,"task_id":task_id(id),"title":"协作提纲，不要求代码😀"})
            .to_string(),
    )
}
fn unread_body() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<axum::body::Bytes, std::io::Error>>> {
            panic!("preflight rejection must not poll the body")
        },
    ))
}
fn headers(response: &Response) {
    for (key, expected) in [
        ("cache-control", "no-store"),
        ("pragma", "no-cache"),
        ("x-content-type-options", "nosniff"),
        ("content-type", "application/json"),
    ] {
        assert_eq!(response.headers()[key], expected);
    }
    for key in [
        "set-cookie",
        "retry-after",
        "location",
        "access-control-allow-origin",
    ] {
        assert!(!response.headers().contains_key(key));
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
        assert_eq!(response.headers()["allow"], "POST");
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
async fn postgres_platform_task_create_http_strict_wire_observed_cancel_and_response_capacity() {
    Fixture::checked(|f|Box::pin(async move {
        let p=Prepared::new(f).await;
        for bad in [0,17] {
            assert!(matches!(TaskCreateHttpState::with_capacity(p.connector().await,bad),Err(TaskContentHttpConfigError::InvalidCapacity)));
            assert!(matches!(PlatformWorkspaceHttpState::with_create_capacities(p.storage().await,1,1,1,1,bad),Err(TaskContentHttpConfigError::InvalidCapacity)));
        }
        let old=build_platform_session_router(PlatformSessionHttpState::new(p.storage().await).unwrap());
        rejected(&old,request(&p,"POST","/platform/v1/tasks",unread_body()),StatusCode::NOT_FOUND,"not_found","not_attempted").await;
        let route=build_task_create_router(TaskCreateHttpState::with_capacity(p.connector().await,1).unwrap());
        for method in ["GET","PUT","DELETE","OPTIONS"] {
            rejected(&route,request(&p,method,"/platform/v1/tasks",unread_body()),StatusCode::METHOD_NOT_ALLOWED,"method_not_allowed","not_attempted").await;
        }
        for path in ["/platform/v1/tasks/extra","/platform/v1/tasks/","/platform/v1/catalog-tasks","/platform/v1/text-artifacts"] {
            rejected(&route,request(&p,"POST",path,unread_body()),StatusCode::NOT_FOUND,"not_found","not_attempted").await;
        }
        rejected(&route,request(&p,"POST","/platform/v1/tasks?owner=other",unread_body()),StatusCode::BAD_REQUEST,"invalid_request","not_attempted").await;
        for header in ["origin","x-cyanrex-platform-request"] {
            let mut r=request(&p,"POST","/platform/v1/tasks",unread_body());
            r.headers_mut().remove(header);
            rejected(&route,r,StatusCode::FORBIDDEN,"request_forbidden","not_attempted").await;
        }
        let mut r=request(&p,"POST","/platform/v1/tasks",unread_body());
        r.headers_mut().insert("origin",HeaderValue::from_static("https://other.example"));
        rejected(&route,r,StatusCode::FORBIDDEN,"request_forbidden","not_attempted").await;
        let before=snapshot(f).await;
        let base=json!({"schema_version":1,"task_id":task_id(10),"title":"Valid title"});
        let mut invalid=vec!["[]".to_owned(),"null".to_owned(),"{}".to_owned(),format!("{{\"schema_version\":1,\"task_id\":\"{}\",\"title\":\"One\",\"title\":\"Two\"}}",task_id(10))];
        for field in ["owner","scope","status","revision","definition_ref","manifest","payload"] {
            let mut v=base.clone();v[field]=json!("not caller authority");invalid.push(v.to_string());
        }
        for title in ["","   ","forbidden\u{0000}control"] {
            let mut v=base.clone();v["title"]=json!(title);invalid.push(v.to_string());
        }
        for body in invalid {
            rejected(&route,request(&p,"POST","/platform/v1/tasks",Body::from(body)),StatusCode::BAD_REQUEST,"invalid_request","not_attempted").await;
        }
        assert_eq!(before,snapshot(f).await);
        let (fence,pid)=super::waits::holder(f).await;
        let mut pending=Pending(tokio::spawn(route.clone().oneshot(request(&p,"POST","/platform/v1/tasks",wire(10)))));
        assert!(super::waits::blocked(f,pid).await&&!pending.0.is_finished());
        rejected(&route,request(&p,"POST","/platform/v1/tasks",unread_body()),StatusCode::SERVICE_UNAVAILABLE,"capacity_exhausted","not_attempted").await;
        pending.0.abort();
        assert!((&mut pending.0).await.unwrap_err().is_cancelled());
        super::waits::release(fence).await;
        assert!(super::storage_helpers::no_connections(f).await);
        assert_eq!(before,snapshot(f).await);
        let response=within(route.clone().oneshot(request(&p,"POST","/platform/v1/tasks",wire(20)))).await.unwrap();
        assert_eq!(response.status(),StatusCode::CREATED);headers(&response);
        rejected(&route,request(&p,"POST","/platform/v1/tasks",unread_body()),StatusCode::SERVICE_UNAVAILABLE,"capacity_exhausted","not_attempted").await;
        let mut body=response.into_body();
        let bytes=within(body.frame()).await.unwrap().unwrap().into_data().unwrap();
        let created:Value=serde_json::from_slice(&bytes).unwrap();
        assert_eq!(created["task"]["reference"]["task_id"],json!(task_id(20)));
        assert_eq!(created["task"]["owner"],json!(p.owners[0]));
        assert_eq!(created["manifest"]["payload"],json!([]));
        assert!(created.get("contents").is_none());
        assert!(body.is_end_stream(),"last JSON frame must release capacity while Body is retained");
        let response=within(route.clone().oneshot(request(&p,"POST","/platform/v1/tasks",wire(30)))).await.unwrap();
        assert_eq!(response.status(),StatusCode::CREATED);
        rejected(&route,request(&p,"POST","/platform/v1/tasks",unread_body()),StatusCode::SERVICE_UNAVAILABLE,"capacity_exhausted","not_attempted").await;
        drop(response);
        rejected(&route,request(&p,"POST","/platform/v1/tasks",wire(30)),StatusCode::CONFLICT,"conflict","unconfirmed").await;
        let combined=build_platform_workspace_router(PlatformWorkspaceHttpState::with_create_capacities(p.storage().await,1,1,1,1,1).unwrap());
        let created=value(within(combined.clone().oneshot(request(&p,"POST","/platform/v1/tasks",wire(40)))).await.unwrap(),StatusCode::CREATED).await;
        assert_eq!(created["task"]["reference"]["task_id"],json!(task_id(40)));
        assert_eq!(p.count("collaboration_tasks").await,3);
        assert_eq!(p.count("collaboration_task_outbox").await,3);
        assert_eq!(std::fs::read_dir(p.root.path()).unwrap().count(),1);
        drop((old,route,combined,pending,body));
        assert!(super::storage_helpers::no_connections(f).await);
        assert!(p.finish());
    })).await;
}
