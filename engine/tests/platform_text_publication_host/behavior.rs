use super::fixture::*;
use axum::{body::Body, http::StatusCode};
use cyanrex_engine::{
    models::collaboration::ArtifactRevision,
    platform_http::{build_platform_workspace_router, PlatformWorkspaceHttpState},
};
use serde_json::json;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_text_publication_workspace_explicit_chain_and_old_constructors_stay_compatible(
) {
    Fixture::checked(|f|Box::pin(async move {
        let p=Prepared::new(f).await;let files=p.files();let private=p.non_artifacts().await;
        let old=build_platform_workspace_router(PlatformWorkspaceHttpState::with_replace_capacities(p.storage().await,1,1,1,1,1,1).unwrap());
        error(&old,request(&p,0,"POST",PATH,unread()),StatusCode::NOT_FOUND,"not_found","not_attempted").await;
        let route=p.router([1;7]).await;
        let created=call(&route,request(&p,0,"POST",PATH,wire(1,"Collaborative notes","教学协作内容😀\r\n不是代码，也不要求执行\n")),StatusCode::CREATED).await;
        let artifact:ArtifactRevision=serde_json::from_value(created["artifact"].clone()).unwrap();assert_eq!(artifact.owner,p.owners[0]);
        assert_eq!(private,p.non_artifacts().await);p.read_committed(&artifact,"教学协作内容😀\r\n不是代码，也不要求执行\n".as_bytes()).await;
        let id=task_id(1);let task_path=format!("/platform/v1/tasks/{id}");
        let empty=call(&route,request(&p,0,"POST","/platform/v1/tasks",Body::from(json!({"schema_version":1,"task_id":id,"title":"Manual teaching task"}).to_string())),StatusCode::CREATED).await;
        assert_eq!(empty["manifest"]["payload"],json!([]));assert_eq!(empty["task"]["revision"],1);assert!(empty["task"]["definition_ref"].is_null());
        let manifest=manifest("Optional text is a payload",vec![artifact.reference.clone()]);
        let edited=call(&route,request(&p,0,"PUT",&format!("{task_path}/content"),Body::from(json!({"schema_version":1,"expected_revision":1,"manifest":manifest}).to_string())),StatusCode::OK).await;
        assert_eq!(edited["task"]["revision"],2);assert_eq!(edited["task"]["owner"],json!(p.owners[0]));assert_eq!(edited["manifest"],json!(manifest));assert!(edited.get("contents").is_none());
        let reopened=call(&route,request(&p,0,"GET",&task_path,Body::empty()),StatusCode::OK).await;
        assert_eq!(reopened["task"],edited["task"]);assert_eq!(reopened["manifest"],edited["manifest"]);assert_eq!(reopened["contents"],json!([{"artifact":artifact.reference,"text":"教学协作内容😀\r\n不是代码，也不要求执行\n"}]));
        let page=call(&route,request(&p,0,"GET","/platform/v1/task-index",Body::empty()),StatusCode::OK).await;
        assert_eq!(page["entries"][0]["reference"]["task_id"],json!(id));assert_eq!(page["entries"][0]["payload_items"],1);
        p.counts(1).await;p.old_files_unchanged(&files,1);
        drop((old,route));assert!(super::storage_helpers::no_connections(f).await);assert!(p.finish());
    })).await;
}
