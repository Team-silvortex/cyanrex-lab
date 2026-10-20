use super::fixture::*;
use sqlx_core::{query::query, row::Row};

async fn acl(f: &Fixture) -> String {
    within(query("SELECT md5(coalesce(string_agg(c.oid::text||':'||coalesce(c.relacl::text,'')||':'||(SELECT coalesce(string_agg(a.attnum::text||':'||coalesce(a.attacl::text,''),'|' ORDER BY a.attnum),'') FROM pg_catalog.pg_attribute a WHERE a.attrelid=c.oid AND a.attnum>0),'|' ORDER BY c.oid),'')) AS acl FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=ANY($1)")
        .bind(vec![f.source.clone(),f.task.clone(),f.artifacts.clone()]).fetch_one(f.owner())).await.unwrap().get("acl")
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_replace_requires_only_explicit_task_update_event_insert_and_original_read_profile(
) {
    Fixture::checked(|f|Box::pin(async move{
        let p=Prepared::new(f).await;let artifact=p.artifact(0,b"Read-only published bytes").await;
        let old=p.task_manifest(10,0,manifest("Original",vec![artifact.reference])).await;let connector=p.connector().await;
        let narrow:bool=within(query("SELECT NOT EXISTS(SELECT 1 FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace JOIN pg_catalog.pg_roles r ON r.rolname=$1 WHERE n.nspname=ANY($2) AND c.relkind='r' AND (has_table_privilege(r.oid,c.oid,'UPDATE') OR has_table_privilege(r.oid,c.oid,'DELETE') OR has_table_privilege(r.oid,c.oid,'TRIGGER') OR (has_table_privilege(r.oid,c.oid,'INSERT') AND NOT(n.nspname=$3 AND c.relname='collaboration_task_outbox')))) AS narrow")
            .bind(&f.role).bind(vec![f.source.clone(),f.task.clone(),f.artifacts.clone()]).bind(&f.task).fetch_one(f.owner())).await.unwrap().get("narrow");assert!(narrow,"no Task INSERT or Artifact business writes");
        for(table,privilege,restore)in[
            (format!("{}.collaboration_tasks",f.task),"UPDATE(snapshot)","UPDATE(snapshot)"),
            (format!("{}.collaboration_task_outbox",f.task),"INSERT","INSERT"),
            (format!("{}.collaboration_artifact_revisions",f.artifacts),"SELECT","SELECT"),
        ]{
            f.sql(&format!("REVOKE {privilege} ON {table} FROM {}",f.role)).await;
            let before=snapshot(f).await;let files=p.files();let original_acl=acl(f).await;
            assert!(p.replace(&connector,0,&old,manifest("No privileged fallback",vec![])).await.is_err());
            assert_eq!(before,snapshot(f).await);assert_eq!(files,p.files());assert_eq!(original_acl,acl(f).await);
            f.sql(&format!("GRANT {restore} ON {table} TO {}",f.role)).await;
        }
        f.sql(&format!("ALTER ROLE {} IN DATABASE {} SET default_transaction_read_only=on",f.role,f.database)).await;
        let before=snapshot(f).await;assert!(p.replace(&connector,0,&old,manifest("No READ ONLY override",vec![])).await.is_err());assert_eq!(before,snapshot(f).await);
        f.sql(&format!("ALTER ROLE {} IN DATABASE {} SET default_transaction_read_only=off",f.role,f.database)).await;
        let files=p.files();let changed=p.replace(&connector,0,&old,manifest("Explicit update lane",vec![])).await.unwrap();p.unchanged(&changed,2).await;assert_eq!(files,p.files());
        drop(connector);assert!(super::storage_helpers::no_connections(f).await);assert!(p.finish());
    })).await;
}
