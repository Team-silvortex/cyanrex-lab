use super::fixture::*;
use sqlx_core::{query::query, row::Row};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_index_narrow_runtime_acl_and_missing_privilege_never_fall_back() {
    Fixture::checked(|f| Box::pin(async move {
        let p = Prepared::new(f).await;
        p.task(10, 0, "No credential or content privileges required").await;
        let connector = p.connector().await;
        assert_eq!(p.page(&connector, 0, None, None).await.unwrap().entries.len(), 1);
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
            AND NOT has_column_privilege(r.oid,$3,'otp_last_counter','SELECT') AS credentials,
            NOT has_table_privilege(r.oid,$4,'SELECT') AND NOT has_table_privilege(r.oid,$5,'SELECT') AS artifacts
            FROM pg_catalog.pg_roles r WHERE rolname=$1")
            .bind(&f.role).bind(vec![f.source.clone(),f.task.clone(),f.artifacts.clone()])
            .bind(format!("{}.users",f.source)).bind(format!("{}.collaboration_artifacts",f.artifacts))
            .bind(format!("{}.collaboration_artifact_revisions",f.artifacts)).fetch_one(f.owner())).await.unwrap();
        for key in ["role", "writes", "credentials", "artifacts"] { assert!(row.get::<bool,_>(key), "unexpected runtime {key} privilege"); }
        f.sql(&format!("REVOKE SELECT ON {}.collaboration_tasks FROM {}", f.task, f.role)).await;
        f.sql(&format!("GRANT SELECT(task_id,owner_id,revision,snapshot) ON {}.collaboration_tasks TO {}", f.task, f.role)).await;
        assert!(p.page(&connector, 0, None, None).await.is_err(), "missing manifest privilege must not use a privileged pool");
        f.sql(&format!("GRANT SELECT ON {}.collaboration_tasks TO {}", f.task, f.role)).await;
        assert_eq!(p.page(&connector, 0, None, None).await.unwrap().entries.len(), 1);
        f.sql(&format!("ALTER ROLE {} IN DATABASE {} SET default_transaction_read_only=on", f.role, f.database)).await;
        assert!(p.page(&connector, 0, None, None).await.is_err(), "connector must preserve explicit transaction read-only policy");
        f.sql(&format!("ALTER ROLE {} IN DATABASE {} SET default_transaction_read_only=off", f.role, f.database)).await;
        drop(connector);
        assert!(super::storage_helpers::no_connections(f).await);
        assert!(p.finish());
    })).await;
}
