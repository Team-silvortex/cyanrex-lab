use super::fixture::*;
use axum::{
    body::{Body, HttpBody},
    http::StatusCode,
    response::Response,
};
use cyanrex_engine::platform_http::{build_platform_workspace_router, PlatformWorkspaceHttpState};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_text_publication_workspace_seven_independent_response_lanes_share_clones(
) {
    Fixture::checked(|f|Box::pin(async move {
        let p=Prepared::new(f).await;let original=p.task(10,0,"Original empty task").await;let files=p.files();
        let state=PlatformWorkspaceHttpState::with_text_publication_capacities(p.storage().await,1,1,1,1,1,1,1).unwrap();
        let route=build_platform_workspace_router(state.clone());let clone=build_platform_workspace_router(state);
        let task_path=format!("/platform/v1/tasks/{}",original.task.reference.task_id);let edit_path=format!("{task_path}/content");
        // Each response retains its own lane after the SQL command has completed. The login
        // lane needs no credential grant here: its admitted private error owns a slot too.
        let cases=vec![
            ("POST","/platform/v1/session/login".to_owned(),json!({"schema_version":1,"username":"index-owner-0","password":"public-synthetic","otp":"123456"}).to_string(),StatusCode::SERVICE_UNAVAILABLE),
            ("GET","/platform/v1/session".to_owned(),String::new(),StatusCode::OK),
            ("GET","/platform/v1/task-index".to_owned(),String::new(),StatusCode::OK),
            ("GET",task_path.clone(),String::new(),StatusCode::OK),
            ("POST","/platform/v1/tasks".to_owned(),json!({"schema_version":1,"task_id":task_id(20),"title":"Independent create lane"}).to_string(),StatusCode::CREATED),
            ("PUT",edit_path.clone(),json!({"schema_version":1,"expected_revision":1,"manifest":manifest("Independent replace lane",vec![])}).to_string(),StatusCode::OK),
        ];
        let mut held:Vec<Response>=Vec::new();
        for(method,path,body,status)in &cases {
            let mut req=request(&p,0,method,path,Body::from(body.clone()));if path.ends_with("/login"){req.headers_mut().remove("cookie");}
            let response=within(route.clone().oneshot(req)).await.unwrap();
            assert_eq!(response.status(),*status,"{path}");headers(&response);held.push(response);
            let mut req=request(&p,0,method,path,unread());if path.ends_with("/login"){req.headers_mut().remove("cookie");}
            error(&clone,req,StatusCode::SERVICE_UNAVAILABLE,"capacity_exhausted","not_attempted").await;
        }
        let response=within(route.clone().oneshot(request(&p,0,"POST",PATH,wire(1,"Independent publication lane","First committed notes")))).await.unwrap();assert_eq!(response.status(),StatusCode::CREATED);headers(&response);
        error(&clone,request(&p,0,"POST",PATH,unread()),StatusCode::SERVICE_UNAVAILABLE,"capacity_exhausted","not_attempted").await;
        for(method,path,_,_)in &cases {let mut req=request(&p,0,method,path,unread());if path.ends_with("/login"){req.headers_mut().remove("cookie");}error(&clone,req,StatusCode::SERVICE_UNAVAILABLE,"capacity_exhausted","not_attempted").await;}
        let mut body=response.into_body();let bytes=within(body.frame()).await.unwrap().unwrap().into_data().unwrap();let value:Value=serde_json::from_slice(&bytes).unwrap();assert_eq!(value["artifact"]["reference"]["artifact_id"],json!(artifact_id(1)));assert!(body.is_end_stream());
        let next=within(clone.clone().oneshot(request(&p,0,"POST",PATH,wire(2,"After terminal frame","Second committed notes")))).await.unwrap();assert_eq!(next.status(),StatusCode::CREATED);drop(next);
        error(&route,request(&p,0,"POST",PATH,wire(2,"After terminal frame","Second committed notes")),StatusCode::CONFLICT,"conflict","unconfirmed").await;
        for response in held {within(response.into_body().collect()).await.unwrap();}
        call(&clone,request(&p,0,"GET","/platform/v1/task-index",Body::empty()),StatusCode::OK).await;
        p.counts(2).await;task_counts(f,2,3).await;p.old_files_unchanged(&files,2);
        drop((route,clone,body));assert!(super::storage_helpers::no_connections(f).await);assert!(p.finish());
    })).await;
}
