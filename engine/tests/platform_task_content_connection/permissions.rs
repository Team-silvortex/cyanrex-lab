use super::fixture::*;
use sqlx_core::{query::query, row::Row};

async fn acl(f: &Fixture) -> String {
    within(query("SELECT md5(coalesce(string_agg(c.oid::text||':'||coalesce(c.relacl::text,'')||':'||
        (SELECT coalesce(string_agg(a.attnum::text||':'||coalesce(a.attacl::text,''),'|' ORDER BY a.attnum),'')
         FROM pg_catalog.pg_attribute a WHERE a.attrelid=c.oid AND a.attnum>0),'|' ORDER BY c.oid),'')) AS acl
        FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
        WHERE n.nspname=ANY($1)")
        .bind(vec![f.source.clone(),f.task.clone(),f.artifacts.clone()]).fetch_one(f.owner())).await.unwrap().get("acl")
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_content_narrow_runtime_privileges_never_fall_back_or_auto_grant() {
    Fixture::checked(|f| Box::pin(async move {
        let p = Prepared::new(f).await;
        let artifact = p.artifact(0, b"Explicit low-privilege content read").await;
        let task = p.task_manifest(10, 0, manifest("只读入口，明确行锁权限", vec![artifact.reference])).await;
        let connector = p.connector().await;
        assert!(p.read(&connector, 0, &task).await.unwrap().is_some());
        let row = within(query("SELECT
            NOT r.rolsuper AND NOT r.rolinherit AND NOT r.rolcreatedb AND NOT r.rolcreaterole AND NOT r.rolbypassrls
            AND NOT EXISTS(SELECT 1 FROM pg_catalog.pg_auth_members WHERE member=r.oid)
            AND NOT has_database_privilege(r.oid,current_database(),'CREATE')
            AND NOT has_database_privilege(r.oid,current_database(),'TEMP') AS role,
            NOT EXISTS(SELECT 1 FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
                WHERE n.nspname=ANY($2) AND c.relkind='r'
                AND (has_table_privilege(r.oid,c.oid,'INSERT') OR has_table_privilege(r.oid,c.oid,'UPDATE')
                    OR has_table_privilege(r.oid,c.oid,'DELETE') OR has_table_privilege(r.oid,c.oid,'TRIGGER'))) AS writes,
            NOT has_column_privilege(r.oid,$3,'password_hash','SELECT')
            AND NOT has_column_privilege(r.oid,$3,'totp_secret','SELECT')
            AND NOT has_column_privilege(r.oid,$3,'otp_last_counter','SELECT') AS credentials
            FROM pg_catalog.pg_roles r WHERE rolname=$1")
            .bind(&f.role).bind(vec![f.source.clone(),f.task.clone(),f.artifacts.clone()])
            .bind(format!("{}.users",f.source)).fetch_one(f.owner())).await.unwrap();
        for key in ["role", "writes", "credentials"] { assert!(row.get::<bool,_>(key), "unexpected runtime {key} privilege"); }
        for (table, restore) in [
            ("collaboration_artifact_revisions", "SELECT"),
            ("collaboration_artifacts", "SELECT"),
            ("collaboration_artifact_schema", "SELECT"),
        ] {
            f.sql(&format!("REVOKE SELECT ON {}.{table} FROM {}", f.artifacts, f.role)).await;
            assert!(p.read(&connector, 0, &task).await.is_err(), "missing {table} privilege must not use a privileged pool");
            let original_acl = acl(f).await;
            assert!(p.read(&connector, 0, &task).await.is_err());
            assert!(original_acl == acl(f).await, "connector must not repair table or column ACLs");
            f.sql(&format!("GRANT {restore} ON {}.{table} TO {}", f.artifacts, f.role)).await;
            assert!(p.read(&connector, 0, &task).await.unwrap().is_some());
        }
        f.sql(&format!("ALTER ROLE {} IN DATABASE {} SET default_transaction_read_only=on", f.role, f.database)).await;
        assert!(p.read(&connector, 0, &task).await.is_err(), "connector preserves explicit transaction policy");
        f.sql(&format!("ALTER ROLE {} IN DATABASE {} SET default_transaction_read_only=off", f.role, f.database)).await;
        drop(connector);
        assert!(super::storage_helpers::no_connections(f).await);
        assert!(p.finish());
    })).await;
}
