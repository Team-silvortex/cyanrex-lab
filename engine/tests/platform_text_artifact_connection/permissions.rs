use super::fixture::*;
use sqlx_core::{query::query, row::Row};

async fn acl(f: &Fixture) -> String {
    within(query("SELECT md5(coalesce(string_agg(c.oid::text||':'||coalesce(c.relacl::text,'')||':'||(SELECT coalesce(string_agg(a.attnum::text||':'||coalesce(a.attacl::text,''),'|' ORDER BY a.attnum),'') FROM pg_catalog.pg_attribute a WHERE a.attrelid=c.oid AND a.attnum>0),'|' ORDER BY c.oid),'')) AS acl FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=ANY($1)")
        .bind(vec![f.source.clone(),f.task.clone(),f.artifacts.clone()]).fetch_one(f.owner())).await.unwrap().get("acl")
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_text_artifact_narrow_insert_profile_and_read_only_policy_are_not_repaired(
) {
    Fixture::checked(|f|Box::pin(async move {
        let p=Prepared::new(f).await; let connector=p.connector().await;
        let row=within(query("SELECT NOT r.rolsuper AND NOT r.rolinherit AND NOT r.rolcreatedb AND NOT r.rolcreaterole AND NOT r.rolbypassrls AND NOT EXISTS(SELECT 1 FROM pg_catalog.pg_auth_members WHERE member=r.oid) AND NOT has_database_privilege(r.oid,current_database(),'CREATE') AND NOT has_database_privilege(r.oid,current_database(),'TEMP') AS role, NOT has_column_privilege(r.oid,$2,'password_hash','SELECT') AND NOT has_column_privilege(r.oid,$2,'totp_secret','SELECT') AND NOT has_column_privilege(r.oid,$2,'otp_last_counter','SELECT') AS credentials FROM pg_catalog.pg_roles r WHERE rolname=$1")
            .bind(&f.role).bind(format!("{}.users",f.source)).fetch_one(f.owner())).await.unwrap();
        assert!(row.get::<bool,_>("role")&&row.get::<bool,_>("credentials"));
        let narrow:bool=within(query("SELECT NOT EXISTS(SELECT 1 FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace JOIN pg_catalog.pg_roles r ON r.rolname=$1 WHERE n.nspname=ANY($2) AND c.relkind='r' AND (has_table_privilege(r.oid,c.oid,'UPDATE') OR has_table_privilege(r.oid,c.oid,'DELETE') OR has_table_privilege(r.oid,c.oid,'TRIGGER') OR (has_table_privilege(r.oid,c.oid,'INSERT') AND NOT(n.nspname=$3 AND c.relname IN ('collaboration_artifacts','collaboration_artifact_revisions','collaboration_artifact_outbox'))))) AS narrow")
            .bind(&f.role).bind(vec![f.source.clone(),f.task.clone(),f.artifacts.clone()]).bind(&f.artifacts).fetch_one(f.owner())).await.unwrap().get("narrow");
        assert!(narrow,"only three Artifact INSERT grants plus previously reviewed SELECT and row-lock column privileges");
        let no_business_updates:bool=within(query("SELECT NOT EXISTS(SELECT 1 FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace JOIN pg_catalog.pg_attribute a ON a.attrelid=c.oid AND a.attnum>0 AND NOT a.attisdropped JOIN pg_catalog.pg_roles r ON r.rolname=$1 WHERE ((n.nspname=$2 AND c.relname IN ('collaboration_artifacts','collaboration_artifact_revisions','collaboration_artifact_outbox')) OR (n.nspname=$3 AND c.relname IN ('collaboration_tasks','collaboration_task_outbox'))) AND has_column_privilege(r.oid,c.oid,a.attname,'UPDATE')) AS none")
            .bind(&f.role).bind(&f.artifacts).bind(&f.task).fetch_one(f.owner())).await.unwrap().get("none");
        assert!(no_business_updates,"no Artifact business UPDATE column and no Task write column");
        for (index,table) in ["collaboration_artifacts","collaboration_artifact_revisions","collaboration_artifact_outbox"].into_iter().enumerate() {
            f.sql(&format!("REVOKE INSERT ON {}.{table} FROM {}",f.artifacts,f.role)).await;
            let before=snapshot(f).await; let original_acl=acl(f).await; let files=p.files();
            assert!(p.create(&connector,0,index as u128+10,"No privilege fallback").await.is_err(),"{table}");
            assert_eq!(before,snapshot(f).await); assert_eq!(original_acl,acl(f).await);
            // A late missing event privilege may leave one unpublished immutable file, never cleanup/adoption.
            let additions=p.files().len()-files.len(); assert!(additions<=usize::from(table=="collaboration_artifact_outbox"));
            p.old_files_unchanged(&files,additions);
            f.sql(&format!("GRANT INSERT ON {}.{table} TO {}",f.artifacts,f.role)).await;
        }
        f.sql(&format!("ALTER ROLE {} IN DATABASE {} SET default_transaction_read_only=on",f.role,f.database)).await;
        let before=snapshot(f).await; let files=p.files(); let original_acl=acl(f).await;
        assert!(p.create(&connector,0,90,"Do not override READ ONLY").await.is_err());
        assert_eq!(before,snapshot(f).await); assert_eq!(files,p.files()); assert_eq!(original_acl,acl(f).await);
        f.sql(&format!("ALTER ROLE {} IN DATABASE {} SET default_transaction_read_only=off",f.role,f.database)).await;
        let committed=p.create(&connector,0,99,"Explicit least-privilege lane").await.unwrap();
        p.counts(1).await; p.read_committed(&committed,b"Explicit least-privilege lane").await;
        drop(connector); assert!(super::storage_helpers::no_connections(f).await); assert!(p.finish());
    })).await;
}
