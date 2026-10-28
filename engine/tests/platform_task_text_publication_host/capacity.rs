use super::fixture::*;
use axum::{
    body::{Body, HttpBody},
    http::StatusCode,
    response::Response,
};
use cyanrex_engine::platform_http::{
    build_platform_workspace_router, PlatformWorkspaceHttpState, TaskContentHttpConfigError,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_text_publication_eight_response_lanes_and_capacity_validation() {
    Fixture::checked(|f| Box::pin(async move {
        let p = Prepared::new(f).await; let old = p.task(10, 0, "Original Task for lane isolation").await; let files = p.files();
        for slot in 0..8 { for bad in [0, 17] {
            let storage = p.storage().await; let mut caps = [1; 8]; caps[slot] = bad;
            assert!(p.root.swap()); let [a,b,c,d,e,g,h,i] = caps;
            assert!(matches!(PlatformWorkspaceHttpState::with_task_text_publication_capacities(storage, a,b,c,d,e,g,h,i), Err(TaskContentHttpConfigError::InvalidCapacity)));
            assert!(p.root.restore()); assert_eq!(files, p.files());
        } }
        let state = PlatformWorkspaceHttpState::with_task_text_publication_capacities(p.storage().await,1,1,1,1,1,1,1,1).unwrap();
        let route = build_platform_workspace_router(state.clone()); let clone = build_platform_workspace_router(state);
        let task_path = format!("/platform/v1/tasks/{}", old.task.reference.task_id); let edit_path = format!("{task_path}/content");
        let cases = vec![
            ("POST", "/platform/v1/session/login".to_owned(), json!({"schema_version":1,"username":"index-owner-0","password":"public-synthetic","otp":"123456"}).to_string(), StatusCode::SERVICE_UNAVAILABLE),
            ("GET", "/platform/v1/session".into(), String::new(), StatusCode::OK),
            ("GET", "/platform/v1/task-index".into(), String::new(), StatusCode::OK),
            ("GET", task_path.clone(), String::new(), StatusCode::OK),
            ("POST", "/platform/v1/tasks".into(), json!({"schema_version":1,"task_id":task_id(20),"title":"Independent create lane"}).to_string(), StatusCode::CREATED),
            ("PUT", edit_path, json!({"schema_version":1,"expected_revision":1,"manifest":manifest("Independent replacement lane", vec![])}).to_string(), StatusCode::OK),
        ];
        let mut held: Vec<Response> = Vec::new();
        for (method, target, body, status) in &cases {
            let mut req = request(&p, 0, method, target, Body::from(body.clone())); if target.ends_with("/login") { req.headers_mut().remove("cookie"); }
            let response = within(route.clone().oneshot(req)).await.unwrap(); assert_eq!(response.status(), *status); headers(&response); held.push(response);
            let mut req = request(&p, 0, method, target, unread()); if target.ends_with("/login") { req.headers_mut().remove("cookie"); }
            error(&clone, req, StatusCode::SERVICE_UNAVAILABLE, "capacity_exhausted", "not_attempted").await;
        }
        let unbound = within(route.clone().oneshot(request(&p, 0, "POST", PATH, unbound_wire(1, "Independent old publication lane", "Old notes")))).await.unwrap();
        assert_eq!(unbound.status(), StatusCode::CREATED); headers(&unbound);
        error(&clone, request(&p, 0, "POST", PATH, unread()), StatusCode::SERVICE_UNAVAILABLE, "capacity_exhausted", "not_attempted").await;
        let expected = current(&p, 10, 0).await;
        let published = within(route.clone().oneshot(request(&p, 0, "POST", &path(&expected), wire(&expected, 2, "New guarded notes")))).await.unwrap();
        assert_eq!(published.status(), StatusCode::CREATED); headers(&published);
        error(&clone, request(&p, 0, "POST", &path(&expected), unread()), StatusCode::SERVICE_UNAVAILABLE, "capacity_exhausted", "not_attempted").await;
        for (method, target, _, _) in &cases {
            let mut req = request(&p, 0, method, target, unread()); if target.ends_with("/login") { req.headers_mut().remove("cookie"); }
            error(&clone, req, StatusCode::SERVICE_UNAVAILABLE, "capacity_exhausted", "not_attempted").await;
        }
        let mut body = published.into_body(); let bytes = within(body.frame()).await.unwrap().unwrap().into_data().unwrap();
        let reply: Value = serde_json::from_slice(&bytes).unwrap(); assert_eq!(reply["artifact"]["reference"]["artifact_id"], json!(artifact_id(2)));
        assert!(body.is_end_stream(), "terminal new-lane JSON releases admission while Body stays owned");
        let next = within(clone.clone().oneshot(request(&p, 0, "POST", &path(&expected), wire(&expected, 3, "After terminal frame")))).await.unwrap();
        assert_eq!(next.status(), StatusCode::CREATED); drop(next);
        error(&route, request(&p, 0, "POST", &path(&expected), wire(&expected, 3, "After terminal frame")), StatusCode::CONFLICT, "conflict", "unconfirmed").await;
        drop(unbound);
        call(&clone, request(&p, 0, "POST", PATH, unbound_wire(4, "After response drop", "Old lane remains independent")), StatusCode::CREATED).await;
        for ((method, target, _, _), response) in cases.iter().zip(held) {
            within(response.into_body().collect()).await.unwrap();
            let mut req = request(&p, 0, method, target, Body::from("{}")); if target.ends_with("/login") { req.headers_mut().remove("cookie"); }
            error(&clone, req, StatusCode::BAD_REQUEST, "invalid_request", "not_attempted").await;
        }
        p.counts(4).await; task_counts(f, 2, 3).await; p.old_files_unchanged(&files, 4); unchanged(&p, 10, 0, &expected).await;
        drop((route, clone, body)); assert!(super::storage_helpers::no_connections(f).await); assert!(p.finish());
    })).await;
}
