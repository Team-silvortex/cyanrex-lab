use super::fixture::*;
use axum::{body::Body, http::StatusCode};
use cyanrex_engine::{
    models::collaboration::ArtifactRevision,
    platform_http::{build_platform_workspace_router, PlatformWorkspaceHttpState},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_text_publication_workspace_nine_routes_and_opt_in_boundaries() {
    Fixture::checked(|f| Box::pin(async move {
        let p = Prepared::new(f).await; let files = p.files();
        let task_path = format!("/platform/v1/tasks/{}", task_id(10)); let publish_path = format!("{task_path}/text-artifacts");
        let legacy = build_platform_workspace_router(PlatformWorkspaceHttpState::new(p.storage().await).unwrap());
        for target in [PATH, &publish_path] { error(&legacy, request(&p, 0, "POST", target, unread()), StatusCode::NOT_FOUND, "not_found", "not_attempted").await; }
        let unbound = build_platform_workspace_router(PlatformWorkspaceHttpState::with_text_publication_capacities(p.storage().await, 1, 1, 1, 1, 1, 1, 1).unwrap());
        error(&unbound, request(&p, 0, "POST", &publish_path, unread()), StatusCode::NOT_FOUND, "not_found", "not_attempted").await;
        call(&unbound, request(&p, 0, "POST", PATH, unbound_wire(99, "Old explicit mode", "Old unbound capability remains compatible")), StatusCode::CREATED).await;
        let route = router(&p, [2; 8]).await; let token = login(&p, &route).await;
        assert_eq!(call(&route, request_token(&p, &token, "GET", "/platform/v1/session", Body::empty()), StatusCode::OK).await,
            json!({"schema_version":1,"authenticated":true}));
        let created = call(&route, request_token(&p, &token, "POST", "/platform/v1/tasks",
            Body::from(json!({"schema_version":1,"task_id":task_id(10),"title":"Collaborative seminar notes, no code or Run required"}).to_string())), StatusCode::CREATED).await;
        assert_eq!(created["task"]["revision"], 1); assert_eq!(created["manifest"]["payload"], json!([]));
        let old = current(&p, 10, 0).await; let private = p.non_artifacts().await;
        let first = call(&route, request_token(&p, &token, "POST", PATH, unbound_wire(1, "Independent legacy publication", "First optional reading notes")), StatusCode::CREATED).await;
        let first: ArtifactRevision = serde_json::from_value(first["artifact"].clone()).unwrap();
        let second = call(&route, request_token(&p, &token, "POST", &publish_path, wire(&old, 2, "Task-bound optional notes😀")), StatusCode::CREATED).await;
        assert_eq!(second.as_object().unwrap().len(), 2); assert!(second.get("task").is_none() && second.get("text").is_none());
        let second: ArtifactRevision = serde_json::from_value(second["artifact"].clone()).unwrap();
        assert_eq!(second.owner, p.owners[0]); assert_eq!(second.title, old.task.title);
        p.read_committed(&second, "Task-bound optional notes😀".as_bytes()).await;
        assert_eq!(private, p.non_artifacts().await); unchanged(&p, 10, 0, &old).await;
        let desired = manifest("Explicit later attachment only", vec![first.reference.clone(), second.reference.clone()]);
        let replaced = call(&route, request_token(&p, &token, "PUT", &format!("{task_path}/content"),
            Body::from(json!({"schema_version":1,"expected_revision":1,"manifest":desired}).to_string())), StatusCode::OK).await;
        assert_eq!(replaced["task"]["revision"], 2);
        let reopened = call(&route, request_token(&p, &token, "GET", &task_path, Body::empty()), StatusCode::OK).await;
        assert_eq!(reopened["task"], replaced["task"]); assert_eq!(reopened["manifest"], replaced["manifest"]);
        assert_eq!(reopened["contents"], json!([{"artifact":first.reference,"text":"First optional reading notes"},
            {"artifact":second.reference,"text":"Task-bound optional notes😀"}]));
        let index = call(&route, request_token(&p, &token, "GET", "/platform/v1/task-index", Body::empty()), StatusCode::OK).await;
        assert_eq!(index["entries"][0]["payload_items"], 2);
        error(&route, request(&p, 1, "POST", &publish_path, wire(&current(&p, 10, 0).await, 3, "No teacher shortcut")), StatusCode::NOT_FOUND, "not_found", "unconfirmed").await;
        for absent in [format!("{task_path}/status"), "/api/state".into()] {
            error(&route, request(&p, 0, "POST", &absent, unread()), StatusCode::NOT_FOUND, "not_found", "not_attempted").await;
        }
        let response = within(route.clone().oneshot(request_token(&p, &token, "POST", "/platform/v1/session/logout", Body::empty()))).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK); assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(response.headers()["set-cookie"], "cyanrex_platform_session=; Path=/platform/v1; HttpOnly; SameSite=Strict; Max-Age=0; Secure");
        let value: Value = serde_json::from_slice(&within(response.into_body().collect()).await.unwrap().to_bytes()).unwrap();
        assert_eq!(value, json!({"schema_version":1,"ok":true}));
        error(&route, request_token(&p, &token, "POST", &publish_path, wire(&current(&p, 10, 0).await, 3, "Revoked current Session")), StatusCode::UNAUTHORIZED, "invalid_session", "unconfirmed").await;
        p.counts(3).await; task_counts(f, 1, 2).await; p.old_files_unchanged(&files, 3);
        drop((route, unbound, legacy)); assert!(super::storage_helpers::no_connections(f).await); assert!(p.finish());
    })).await;
}
