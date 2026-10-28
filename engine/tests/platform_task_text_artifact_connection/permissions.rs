use super::fixture::*;
use sqlx_core::{query::query, row::Row};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_text_artifact_narrow_insert_task_lock_and_readonly_profile_are_not_repaired(
) {
    Fixture::checked(|f| Box::pin(async move {
        let p = Prepared::new(f).await;
        let old = p.task(10, 0, "Narrow private publication").await;
        let connector = p.connector().await;
        let narrow: bool = within(query("SELECT NOT EXISTS(SELECT 1 FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace JOIN pg_catalog.pg_roles r ON r.rolname=$1 WHERE n.nspname=ANY($2) AND c.relkind='r' AND (has_table_privilege(r.oid,c.oid,'UPDATE') OR has_table_privilege(r.oid,c.oid,'DELETE') OR has_table_privilege(r.oid,c.oid,'TRIGGER') OR (has_table_privilege(r.oid,c.oid,'INSERT') AND NOT(n.nspname=$3 AND c.relname IN ('collaboration_artifacts','collaboration_artifact_revisions','collaboration_artifact_outbox'))))) AS narrow")
            .bind(&f.role).bind(vec![f.source.clone(), f.task.clone(), f.artifacts.clone()]).bind(&f.artifacts).fetch_one(f.owner())).await.unwrap().get("narrow");
        assert!(narrow);
        let updates: Vec<(String, String)> = within(query("SELECT c.relname::text AS table_name,a.attname::text AS column_name FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace JOIN pg_catalog.pg_attribute a ON a.attrelid=c.oid AND a.attnum>0 AND NOT a.attisdropped JOIN pg_catalog.pg_roles r ON r.rolname=$1 WHERE ((n.nspname=$2 AND c.relname IN ('collaboration_artifacts','collaboration_artifact_revisions','collaboration_artifact_outbox')) OR (n.nspname=$3 AND c.relname IN ('collaboration_tasks','collaboration_task_outbox'))) AND has_column_privilege(r.oid,c.oid,a.attname,'UPDATE') ORDER BY c.relname,a.attname")
            .bind(&f.role).bind(&f.artifacts).bind(&f.task).fetch_all(f.owner())).await.unwrap().into_iter()
            .map(|row| (row.get("table_name"), row.get("column_name"))).collect();
        assert_eq!(updates, vec![("collaboration_tasks".to_owned(), "task_id".to_owned())]);
        for (index, table) in ["collaboration_artifacts", "collaboration_artifact_revisions", "collaboration_artifact_outbox"].into_iter().enumerate() {
            f.sql(&format!("REVOKE INSERT ON {}.{table} FROM {}", f.artifacts, f.role)).await;
            let before = snapshot(f).await; let files = p.files(); let acl = p.acl().await;
            assert!(p.create(&connector, 0, &old, 10 + index as u128, "No permission repair").await.is_err());
            assert_eq!(before, snapshot(f).await); assert_eq!(acl, p.acl().await);
            let additions = p.files().len() - files.len();
            assert!(additions <= usize::from(table == "collaboration_artifact_outbox"));
            p.old_files_unchanged(&files, additions);
            f.sql(&format!("GRANT INSERT ON {}.{table} TO {}", f.artifacts, f.role)).await;
        }
        f.sql(&format!("REVOKE UPDATE(task_id) ON {}.collaboration_tasks FROM {}", f.task, f.role)).await;
        let before = snapshot(f).await; let files = p.files(); let acl = p.acl().await;
        assert!(p.create(&connector, 0, &old, 20, "Do not grant Task lock").await.is_err());
        assert_eq!(before, snapshot(f).await); assert_eq!(files, p.files()); assert_eq!(acl, p.acl().await);
        f.sql(&format!("GRANT UPDATE(task_id) ON {}.collaboration_tasks TO {}", f.task, f.role)).await;
        f.sql(&format!("ALTER ROLE {} IN DATABASE {} SET default_transaction_read_only=on", f.role, f.database)).await;
        let before = snapshot(f).await; let files = p.files(); let acl = p.acl().await;
        assert!(p.create(&connector, 0, &old, 30, "Do not override readonly").await.is_err());
        assert_eq!(before, snapshot(f).await); assert_eq!(files, p.files()); assert_eq!(acl, p.acl().await);
        f.sql(&format!("ALTER ROLE {} IN DATABASE {} SET default_transaction_read_only=off", f.role, f.database)).await;
        let created = p.create(&connector, 0, &old, 40, "Explicit narrow profile").await.unwrap();
        p.read_committed(&created, b"Explicit narrow profile").await; p.unchanged(&old).await;
        p.counts(1).await;
        drop(connector); assert!(super::storage_helpers::no_connections(f).await); assert!(p.finish());
    })).await;
}
