use super::fixture::*;
use axum::http::StatusCode;
use cyanrex_engine::models::collaboration::ArtifactRevision;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_text_publication_workspace_original_binding_and_current_authority_fail_closed(
) {
    for mode in [
        "root",
        "source_marker",
        "task_marker",
        "artifact_marker",
        "revoked",
        "expired",
        "suspended",
        "unbound",
    ] {
        Fixture::checked(|f|Box::pin(async move {
            let p=Prepared::new(f).await;let route=p.router([1;7]).await;
            let committed=call(&route,request(&p,0,"POST",PATH,wire(1,"Committed original","Preserved exact notes")),StatusCode::CREATED).await;
            let original:ArtifactRevision=serde_json::from_value(committed["artifact"].clone()).unwrap();
            let files=p.files();let mut saved=None;
            if mode=="root" {assert!(p.root.swap());}
            else if mode.ends_with("marker") {
                let(namespace,table)=match mode {"source_marker"=>(&f.source,"collaboration_auth_source_schema"),"task_marker"=>(&f.task,"collaboration_task_schema"),_=>(&f.artifacts,"collaboration_artifact_schema")};
                let marker=format!("{namespace}.{table}");let original=format!("{namespace}.publication_host_original");
                f.sql(&format!("ALTER TABLE {marker} RENAME TO publication_host_original")).await;
                f.sql(&format!("CREATE TABLE {marker} (LIKE {original} INCLUDING ALL)")).await;f.sql(&format!("INSERT INTO {marker} SELECT * FROM {original}")).await;
                f.sql(&format!("GRANT SELECT,UPDATE(singleton) ON {marker} TO {}",f.role)).await;saved=Some((marker,original,table));
            } else if mode=="revoked" {f.sql(&format!("DELETE FROM {}.sessions WHERE token='{}'",f.source,token_hash(&p.tokens[0]))).await;}
            else if mode=="expired" {f.sql(&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE token='{}'",f.source,token_hash(&p.tokens[0]))).await;}
            else if mode=="suspended" {p.suspend(0).await;}
            let before=snapshot(f).await;
            let(owner,status,code)=match mode {"unbound"=>(2,StatusCode::FORBIDDEN,"access_denied"),"suspended"=>(0,StatusCode::FORBIDDEN,"access_denied"),"revoked"|"expired"=>(0,StatusCode::UNAUTHORIZED,"invalid_session"),_=>(0,StatusCode::SERVICE_UNAVAILABLE,"service_unavailable")};
            error(&route,request(&p,owner,"POST",PATH,wire(2,"No inherited authority","A genuinely new well-formed publication")),status,code,"unconfirmed").await;
            assert_eq!(before,snapshot(f).await);
            if mode=="root" {assert!(p.root.restore());}
            if let Some((marker,original,table))=saved {f.sql(&format!("DROP TABLE {marker}")).await;f.sql(&format!("ALTER TABLE {original} RENAME TO {table}")).await;}
            assert_eq!(files,p.files());p.counts(1).await;p.read_committed(&original,b"Preserved exact notes").await;
            drop(route);assert!(super::storage_helpers::no_connections(f).await);assert!(p.finish());
        })).await;
    }
}
