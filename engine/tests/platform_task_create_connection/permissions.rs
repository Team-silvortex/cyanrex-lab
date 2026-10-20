use super::fixture::*;
use sqlx_core::{query::query, row::Row};

async fn acl(f: &Fixture) -> String {
    within(query("SELECT md5(coalesce(string_agg(c.oid::text||':'||coalesce(c.relacl::text,'')||':'||(SELECT coalesce(string_agg(a.attnum::text||':'||coalesce(a.attacl::text,''),'|' ORDER BY a.attnum),'') FROM pg_catalog.pg_attribute a WHERE a.attrelid=c.oid AND a.attnum>0),'|' ORDER BY c.oid),'')) AS acl FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=ANY($1)")
        .bind(vec![f.source.clone(),f.task.clone(),f.artifacts.clone()]).fetch_one(f.owner())).await.unwrap().get("acl")
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_create_requires_narrow_insert_privileges_and_explicit_read_write_role(
) {
    Fixture::checked(|f|Box::pin(async move {
        let p=Prepared::new(f).await;
        let connector=p.connector().await;
        let row=within(query("SELECT NOT r.rolsuper AND NOT r.rolinherit AND NOT r.rolcreatedb AND NOT r.rolcreaterole AND NOT r.rolbypassrls AND NOT EXISTS(SELECT 1 FROM pg_catalog.pg_auth_members WHERE member=r.oid) AND NOT has_database_privilege(r.oid,current_database(),'CREATE') AND NOT has_database_privilege(r.oid,current_database(),'TEMP') AS role, NOT has_column_privilege(r.oid,$2,'password_hash','SELECT') AND NOT has_column_privilege(r.oid,$2,'totp_secret','SELECT') AND NOT has_column_privilege(r.oid,$2,'otp_last_counter','SELECT') AS credentials FROM pg_catalog.pg_roles r WHERE rolname=$1")
            .bind(&f.role).bind(format!("{}.users",f.source)).fetch_one(f.owner())).await.unwrap();
        assert!(row.get::<bool,_>("role")&&row.get::<bool,_>("credentials"));
        let only_task_insert:bool=within(query("SELECT NOT EXISTS(SELECT 1 FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace JOIN pg_catalog.pg_roles r ON r.rolname=$1 WHERE n.nspname=ANY($2) AND c.relkind='r' AND (has_table_privilege(r.oid,c.oid,'UPDATE') OR has_table_privilege(r.oid,c.oid,'DELETE') OR has_table_privilege(r.oid,c.oid,'TRIGGER') OR (has_table_privilege(r.oid,c.oid,'INSERT') AND NOT(n.nspname=$3 AND c.relname IN ('collaboration_tasks','collaboration_task_outbox'))))) AS narrow")
            .bind(&f.role).bind(vec![f.source.clone(),f.task.clone(),f.artifacts.clone()]).bind(&f.task).fetch_one(f.owner())).await.unwrap().get("narrow");
        assert!(only_task_insert,"only two Task INSERT grants and existing row-lock column privileges are allowed");
        let no_artifact_read:bool=within(query("SELECT NOT EXISTS(SELECT 1 FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace JOIN pg_catalog.pg_roles r ON r.rolname=$1 WHERE n.nspname=$2 AND c.relname IN ('collaboration_artifacts','collaboration_artifact_revisions','collaboration_artifact_outbox') AND has_table_privilege(r.oid,c.oid,'SELECT')) AS no_read")
            .bind(&f.role).bind(&f.artifacts).fetch_one(f.owner())).await.unwrap().get("no_read");
        assert!(no_artifact_read,"empty creation must work without Artifact business-table SELECT");
        for table in ["collaboration_tasks","collaboration_task_outbox"] {
            f.sql(&format!("REVOKE INSERT ON {}.{table} FROM {}",f.task,f.role)).await;
            let before=snapshot(f).await;
            let original_acl=acl(f).await;
            assert!(p.create(&connector,0,task_id(10),"No privileged fallback").await.is_err(),"{table}");
            assert_eq!(before,snapshot(f).await);
            assert_eq!(original_acl,acl(f).await,"no repair of missing INSERT privilege");
            f.sql(&format!("GRANT INSERT ON {}.{table} TO {}",f.task,f.role)).await;
        }
        f.sql(&format!("ALTER ROLE {} IN DATABASE {} SET default_transaction_read_only=on",f.role,f.database)).await;
        let before=snapshot(f).await;
        assert!(p.create(&connector,0,task_id(10),"Do not override READ ONLY").await.is_err());
        assert_eq!(before,snapshot(f).await);
        f.sql(&format!("ALTER ROLE {} IN DATABASE {} SET default_transaction_read_only=off",f.role,f.database)).await;
        let created=p.create(&connector,0,task_id(10),"Explicitly provisioned lane").await.unwrap();
        p.created(&created,1).await;
        drop(connector);
        assert!(super::storage_helpers::no_connections(f).await);
        assert!(p.finish());
    })).await;
}
