use super::fixture::*;
use cyanrex_engine::services::{
    auth_service::durable_source::SessionTaskError, task_store::TaskStoreError,
};
use sqlx_core::{query::query, row::Row};
use std::fs;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_text_artifact_deferred_task_drift_rolls_back_rows_but_preserves_uncertain_blob(
) {
    Fixture::checked(|f| Box::pin(async move {
        let p = Prepared::new(f).await;
        let old = p.task(10, 0, "Pinned Task before publication").await;
        let connector = p.connector().await;
        f.sql(&format!("CREATE SEQUENCE {}.task_text_fault_hits", f.artifacts)).await;
        // Change a complete, mutually coherent Task/manifest/outbox triple at the same revision.
        // The inserted Artifact remains exact; only the selected Task baseline has drifted.
        f.sql(&format!("CREATE FUNCTION {0}.task_text_fault() RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$ BEGIN PERFORM pg_catalog.nextval('{0}.task_text_fault_hits'); UPDATE {1}.collaboration_tasks SET snapshot=pg_catalog.jsonb_set(snapshot::jsonb,'{{title}}','\"Deferred Task change\"')::text,manifest=pg_catalog.jsonb_set(manifest::jsonb,'{{title}}','\"Deferred Task change\"')::text WHERE task_id='{2}'; UPDATE {1}.collaboration_task_outbox SET snapshot=pg_catalog.jsonb_set(snapshot::jsonb,'{{title}}','\"Deferred Task change\"')::text,manifest=pg_catalog.jsonb_set(manifest::jsonb,'{{title}}','\"Deferred Task change\"')::text WHERE task_id='{2}'; RETURN NEW; END $$", f.artifacts, f.task, old.task.reference.task_id)).await;
        f.sql(&format!("CREATE CONSTRAINT TRIGGER task_text_fault AFTER INSERT ON {0}.collaboration_artifact_outbox DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION {0}.task_text_fault()", f.artifacts)).await;
        let before = snapshot(f).await;
        let files = p.files();
        let text = "Unconfirmed immutable bytes must remain private";
        assert_eq!(p.create(&connector, 0, &old, 1, text).await,
            Err(SessionTaskError::Task(TaskStoreError::InvalidRecord)));
        let hits: i64 = within(query(&format!("SELECT CASE WHEN is_called THEN last_value ELSE 0 END AS hits FROM {}.task_text_fault_hits", f.artifacts))
            .fetch_one(f.owner())).await.unwrap().get("hits");
        assert_eq!(hits, 1, "the deferred Task mutation must actually execute before rejection");
        assert_eq!(before, snapshot(f).await);
        p.unchanged(&old).await;
        p.counts(0).await;
        p.old_files_unchanged(&files, 1);
        assert_eq!(fs::read(p.blob(&p.reference(1, text.as_bytes()))).unwrap(), text.as_bytes());
        drop(connector); assert!(super::storage_helpers::no_connections(f).await); assert!(p.finish());
    })).await;
}
