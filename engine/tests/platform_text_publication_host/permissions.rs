use super::fixture::*;
use axum::{body::Body, http::StatusCode};
use sqlx_core::{query::query, row::Row};

async fn acl(f: &Fixture) -> String {
    within(query("SELECT md5(coalesce(string_agg(c.oid::text||':'||coalesce(c.relacl::text,'')||':'||(SELECT coalesce(string_agg(a.attnum::text||':'||coalesce(a.attacl::text,''),'|' ORDER BY a.attnum),'') FROM pg_catalog.pg_attribute a WHERE a.attrelid=c.oid AND a.attnum>0),'|' ORDER BY c.oid),'')) AS acl FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=ANY($1)").bind(vec![f.source.clone(),f.task.clone(),f.artifacts.clone()]).fetch_one(f.owner())).await.unwrap().get("acl")
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_text_publication_workspace_minimal_grants_and_unknown_orphans_are_not_repaired(
) {
    Fixture::checked(|f|Box::pin(async move {
        let p=Prepared::new(f).await;let route=p.router([1;7]).await;
        f.sql(&format!("REVOKE UPDATE(artifact_id) ON {}.collaboration_artifacts FROM {}",f.artifacts,f.role)).await;
        let row=within(query("SELECT NOT r.rolsuper AND NOT r.rolinherit AND NOT r.rolcreatedb AND NOT r.rolcreaterole AND NOT r.rolbypassrls AND NOT EXISTS(SELECT 1 FROM pg_catalog.pg_auth_members WHERE member=r.oid) AND NOT has_database_privilege(r.oid,current_database(),'CREATE') AND NOT has_database_privilege(r.oid,current_database(),'TEMP') AS role, NOT has_column_privilege(r.oid,$2,'password_hash','SELECT') AND NOT has_column_privilege(r.oid,$2,'totp_secret','SELECT') AND NOT has_column_privilege(r.oid,$2,'otp_last_counter','SELECT') AS credentials, NOT has_column_privilege(r.oid,$3,'artifact_id','UPDATE') AND NOT has_table_privilege(r.oid,$3,'UPDATE') AND NOT has_table_privilege(r.oid,$3,'DELETE') AS artifact FROM pg_catalog.pg_roles r WHERE rolname=$1").bind(&f.role).bind(format!("{}.users",f.source)).bind(format!("{}.collaboration_artifacts",f.artifacts)).fetch_one(f.owner())).await.unwrap();
        assert!(row.get::<bool,_>("role")&&row.get::<bool,_>("credentials")&&row.get::<bool,_>("artifact"));
        let grants=acl(f).await;let files=p.files();
        let published=call(&route,request(&p,0,"POST",PATH,wire(1,"No credential or head UPDATE grant","Create-only publication remains independent")),StatusCode::CREATED).await;
        let artifact:cyanrex_engine::models::collaboration::ArtifactRevision=serde_json::from_value(published["artifact"].clone()).unwrap();
        let linked=p.task_manifest(1,0,manifest("Old reader keeps its exact-head lock policy",vec![artifact.reference.clone()])).await;
        assert_eq!(grants,acl(f).await);p.old_files_unchanged(&files,1);
        error(&route,request(&p,0,"GET",&format!("/platform/v1/tasks/{}",linked.task.reference.task_id),Body::empty()),StatusCode::SERVICE_UNAVAILABLE,"service_unavailable","unconfirmed").await;
        assert_eq!(grants,acl(f).await);
        // A controlled owner-only restoration proves this exact valid Task failed on the
        // missing old-reader lock privilege, not corrupt content or an unrelated outage.
        let before_read=snapshot(f).await;let retained=p.files();
        f.sql(&format!("GRANT UPDATE(artifact_id) ON {}.collaboration_artifacts TO {}",f.artifacts,f.role)).await;
        let reopened=call(&route,request(&p,0,"GET",&format!("/platform/v1/tasks/{}",linked.task.reference.task_id),Body::empty()),StatusCode::OK).await;
        assert_eq!(reopened["task"],serde_json::json!(linked.task));assert_eq!(reopened["manifest"],serde_json::json!(linked.manifest));
        assert_eq!(reopened["contents"],serde_json::json!([{"artifact":artifact.reference,"text":"Create-only publication remains independent"}]));
        assert_eq!(before_read,snapshot(f).await);assert_eq!(retained,p.files());
        f.sql(&format!("REVOKE UPDATE(artifact_id) ON {}.collaboration_artifacts FROM {}",f.artifacts,f.role)).await;
        assert_eq!(grants,acl(f).await);
        for(index,table)in["collaboration_artifacts","collaboration_artifact_revisions","collaboration_artifact_outbox"].into_iter().enumerate() {
            f.sql(&format!("REVOKE INSERT ON {}.{table} FROM {}",f.artifacts,f.role)).await;
            let before=snapshot(f).await;let files=p.files();let grants=acl(f).await;
            error(&route,request(&p,0,"POST",PATH,wire(index as u128+10,"Do not repair permissions","Denied fresh bytes")),StatusCode::SERVICE_UNAVAILABLE,"service_unavailable","unconfirmed").await;
            assert_eq!(before,snapshot(f).await);assert_eq!(grants,acl(f).await);
            let additions=p.files().len()-files.len();assert!(additions<=usize::from(table=="collaboration_artifact_outbox"));p.old_files_unchanged(&files,additions);
            f.sql(&format!("GRANT INSERT ON {}.{table} TO {}",f.artifacts,f.role)).await;
        }
        f.sql(&format!("CREATE SEQUENCE {}.publication_host_fault_hits",f.artifacts)).await;
        f.sql(&format!("CREATE FUNCTION {0}.publication_host_fault() RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$ BEGIN PERFORM pg_catalog.nextval('{0}.publication_host_fault_hits'); RAISE EXCEPTION 'private pre-commit publication failure'; END $$",f.artifacts)).await;
        f.sql(&format!("CREATE CONSTRAINT TRIGGER publication_host_fault AFTER INSERT ON {0}.collaboration_artifact_outbox DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION {0}.publication_host_fault()",f.artifacts)).await;
        let before=snapshot(f).await;let files=p.files();let grants=acl(f).await;
        error(&route,request(&p,0,"POST",PATH,wire(90,"Unconfirmed is not cleanup authority","Unpublished evidence stays private")),StatusCode::SERVICE_UNAVAILABLE,"service_unavailable","unconfirmed").await;
        let hits:i64=within(query(&format!("SELECT CASE WHEN is_called THEN last_value ELSE 0 END AS hits FROM {}.publication_host_fault_hits",f.artifacts)).fetch_one(f.owner())).await.unwrap().get("hits");assert_eq!(hits,1);
        assert_eq!(before,snapshot(f).await);assert_eq!(grants,acl(f).await);p.old_files_unchanged(&files,1);
        f.sql(&format!("DROP TRIGGER publication_host_fault ON {}.collaboration_artifact_outbox",f.artifacts)).await;
        let retained=p.files();
        error(&route,request(&p,0,"POST",PATH,wire(90,"Unconfirmed is not cleanup authority","Unpublished evidence stays private")),StatusCode::CONFLICT,"conflict","unconfirmed").await;
        assert_eq!(before,snapshot(f).await);assert_eq!(retained,p.files());p.counts(1).await;task_counts(f,1,1).await;
        // The fault is observed before COMMIT; no real lost acknowledgement is injected here.
        drop(route);assert!(super::storage_helpers::no_connections(f).await);assert!(p.finish());
    })).await;
}
