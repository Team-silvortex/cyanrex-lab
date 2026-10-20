use super::fixture::*;
use axum::{
    body::{Body, HttpBody},
    http::{HeaderValue, Request, StatusCode},
    response::Response,
    Router,
};
use cyanrex_engine::platform_http::{
    build_platform_session_router, build_platform_workspace_router,
    build_task_content_replace_router, PlatformSessionHttpState, PlatformWorkspaceHttpState,
    TaskContentHttpConfigError, TaskContentReplaceHttpState,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

fn path() -> String {
    format!("/platform/v1/tasks/{}/content", task_id(10))
}
fn wire(revision: u64, title: &str) -> Body {
    Body::from(
        json!({"schema_version":1,"expected_revision":revision,"manifest":manifest(title,vec![])})
            .to_string(),
    )
}
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
fn unread() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<axum::body::Bytes, std::io::Error>>> {
            panic!("preflight rejection must not poll body")
        },
    ))
}
fn headers(r: &Response) {
    for (key, value) in [
        ("cache-control", "no-store"),
        ("pragma", "no-cache"),
        ("x-content-type-options", "nosniff"),
        ("content-type", "application/json"),
    ] {
        assert_eq!(r.headers()[key], value);
    }
    for key in [
        "set-cookie",
        "retry-after",
        "location",
        "access-control-allow-origin",
    ] {
        assert!(!r.headers().contains_key(key));
    }
}
async fn value(r: Response, status: StatusCode) -> Value {
    assert_eq!(r.status(), status);
    headers(&r);
    serde_json::from_slice(&within(r.into_body().collect()).await.unwrap().to_bytes()).unwrap()
}
async fn rejected(
    route: &Router,
    request: Request<Body>,
    status: StatusCode,
    code: &str,
    outcome: &str,
) {
    let r = within(route.clone().oneshot(request)).await.unwrap();
    if status == StatusCode::METHOD_NOT_ALLOWED {
        assert_eq!(r.headers()["allow"], "PUT");
    }
    assert_eq!(
        value(r, status).await,
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
async fn postgres_platform_task_replace_http_strict_manifest_private_outcomes_and_body_lifetime_capacity(
) {
    Fixture::checked(|f|Box::pin(async move{
        let p=Prepared::new(f).await;let old=p.task(10,0,"Original empty task").await;let path=path();
        for bad in[0,17]{
            assert!(matches!(TaskContentReplaceHttpState::with_capacity(p.connector().await,bad),Err(TaskContentHttpConfigError::InvalidCapacity)));
            assert!(matches!(PlatformWorkspaceHttpState::with_replace_capacities(p.storage().await,1,1,1,1,1,bad),Err(TaskContentHttpConfigError::InvalidCapacity)));
        }
        let legacy=build_platform_session_router(PlatformSessionHttpState::new(p.storage().await).unwrap());
        rejected(&legacy,request(&p,"PUT",&path,unread()),StatusCode::NOT_FOUND,"not_found","not_attempted").await;
        let route=build_task_content_replace_router(TaskContentReplaceHttpState::with_capacity(p.connector().await,1).unwrap());
        for method in["GET","POST","PATCH","DELETE","OPTIONS"]{rejected(&route,request(&p,method,&path,unread()),StatusCode::METHOD_NOT_ALLOWED,"method_not_allowed","not_attempted").await;}
        for absent in["/platform/v1/tasks".to_owned(),format!("/platform/v1/tasks/{}",task_id(10)),format!("{path}/extra"),format!("/platform/v1/tasks/{}/status",task_id(10))]{rejected(&route,request(&p,"PUT",&absent,unread()),StatusCode::NOT_FOUND,"not_found","not_attempted").await;}
        rejected(&route,request(&p,"PUT",&format!("{path}?owner=other"),unread()),StatusCode::BAD_REQUEST,"invalid_request","not_attempted").await;
        for header in["origin","x-cyanrex-platform-request"]{let mut r=request(&p,"PUT",&path,unread());r.headers_mut().remove(header);rejected(&route,r,StatusCode::FORBIDDEN,"request_forbidden","not_attempted").await;}
        let before=snapshot(f).await;let files=p.files();
        let base=json!({"schema_version":1,"expected_revision":1,"manifest":manifest("Valid metadata",vec![])});
        let mut invalid=vec!["[]".to_owned(),"null".to_owned(),"{}".to_owned(),format!("{{\"schema_version\":1,\"expected_revision\":1,\"expected_revision\":2,\"manifest\":{}}}",serde_json::to_string(&manifest("Duplicated revision",vec![])).unwrap())];
        for field in["owner","scope","task_id","status","contents","definition_ref"]{let mut v=base.clone();v[field]=json!("not caller authority");invalid.push(v.to_string());}
        for revision in[json!(0),json!(-1),json!("1"),json!(1.5),json!(9007199254740992u64)]{let mut v=base.clone();v["expected_revision"]=revision;invalid.push(v.to_string());}
        for body in invalid{rejected(&route,request(&p,"PUT",&path,Body::from(body)),StatusCode::BAD_REQUEST,"invalid_request","not_attempted").await;}
        assert_eq!(before,snapshot(f).await);assert_eq!(files,p.files());
        let dummy:cyanrex_engine::models::collaboration::ArtifactId=uuid::Uuid::new_v4().try_into().unwrap();
        let before=snapshot(f).await;let files=p.files();
        let(fence,pid)=super::waits::holder(f,"event",old.task.reference.task_id,dummy).await;
        let mut pending=Pending(tokio::spawn(route.clone().oneshot(request(&p,"PUT",&path,wire(1,"Cancelled metadata edit")))));
        assert!(super::waits::blocked(f,pid).await&&!pending.0.is_finished());
        rejected(&route,request(&p,"PUT",&path,unread()),StatusCode::SERVICE_UNAVAILABLE,"capacity_exhausted","not_attempted").await;
        pending.0.abort();assert!((&mut pending.0).await.unwrap_err().is_cancelled());super::waits::release(fence).await;
        assert!(super::storage_helpers::no_connections(f).await);assert_eq!(before,snapshot(f).await);assert_eq!(files,p.files());
        let response=within(route.clone().oneshot(request(&p,"PUT",&path,wire(1,"Committed first metadata edit")))).await.unwrap();assert_eq!(response.status(),StatusCode::OK);headers(&response);
        rejected(&route,request(&p,"PUT",&path,unread()),StatusCode::SERVICE_UNAVAILABLE,"capacity_exhausted","not_attempted").await;
        let mut body=response.into_body();let bytes=within(body.frame()).await.unwrap().unwrap().into_data().unwrap();let changed:Value=serde_json::from_slice(&bytes).unwrap();
        assert_eq!(changed["task"]["revision"],2);assert_eq!(changed["task"]["owner"],json!(p.owners[0]));assert!(changed.get("contents").is_none());assert!(body.is_end_stream());
        let response=within(route.clone().oneshot(request(&p,"PUT",&path,wire(2,"Committed but response body dropped")))).await.unwrap();assert_eq!(response.status(),StatusCode::OK);
        rejected(&route,request(&p,"PUT",&path,unread()),StatusCode::SERVICE_UNAVAILABLE,"capacity_exhausted","not_attempted").await;drop(response);
        rejected(&route,request(&p,"PUT",&path,wire(2,"No stale replay")),StatusCode::CONFLICT,"stale_revision","unconfirmed").await;
        let mut foreign=request(&p,"PUT",&path,wire(3,"No teacher shortcut"));foreign.headers_mut().insert("cookie",HeaderValue::from_str(&format!("cyanrex_platform_session={}",p.tokens[1])).unwrap());
        rejected(&route,foreign,StatusCode::NOT_FOUND,"not_found","unconfirmed").await;
        let combined=build_platform_workspace_router(PlatformWorkspaceHttpState::with_replace_capacities(p.storage().await,1,1,1,1,1,1).unwrap());
        let final_result=value(within(combined.clone().oneshot(request(&p,"PUT",&path,wire(3,"Combined explicit replacement lane")))).await.unwrap(),StatusCode::OK).await;assert_eq!(final_result["task"]["revision"],4);
        assert_eq!(p.count("collaboration_tasks").await,1);assert_eq!(p.count("collaboration_task_outbox").await,4);assert_eq!(files,p.files());
        drop((legacy,route,combined,pending,body));assert!(super::storage_helpers::no_connections(f).await);assert!(p.finish());
    })).await;
}
