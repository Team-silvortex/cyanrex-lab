use super::fixture::*;
use axum::http::StatusCode;
use sqlx_core::{query::query, row::Row};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_text_artifact_http_deferred_task_drift_never_confirms() {
    Fixture::checked(|f| Box::pin(async move {
        let p = Prepared::new(f).await; let old = p.task(10, 0, "Original whole Task").await;
        let route = router(&p, 1).await;
        f.sql(&format!("CREATE SEQUENCE {}.task_text_http_fault_hits", f.artifacts)).await;
        f.sql(&format!("CREATE FUNCTION {0}.task_text_http_fault() RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$ BEGIN UPDATE {1}.collaboration_tasks SET snapshot=pg_catalog.jsonb_set(snapshot::jsonb,'{{title}}','\"Coherent deferred Task drift\"')::text,manifest=pg_catalog.jsonb_set(manifest::jsonb,'{{title}}','\"Coherent deferred Task drift\"')::text WHERE task_id='{2}'; UPDATE {1}.collaboration_task_outbox SET snapshot=pg_catalog.jsonb_set(snapshot::jsonb,'{{title}}','\"Coherent deferred Task drift\"')::text,manifest=pg_catalog.jsonb_set(manifest::jsonb,'{{title}}','\"Coherent deferred Task drift\"')::text WHERE task_id='{2}'; PERFORM pg_catalog.nextval('{0}.task_text_http_fault_hits'); RETURN NEW; END $$", f.artifacts, f.task, old.task.reference.task_id)).await;
        f.sql(&format!("CREATE CONSTRAINT TRIGGER task_text_http_fault AFTER INSERT ON {0}.collaboration_artifact_outbox DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION {0}.task_text_http_fault()", f.artifacts)).await;
        let before = snapshot(f).await; let files = p.files(); let text = "Retain unconfirmed private bytes";
        error(&route, request(&p, 0, "POST", &path(&old), wire(&old, 1, text)), StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable", "unconfirmed").await;
        let hits: i64 = within(query(&format!("SELECT CASE WHEN is_called THEN last_value ELSE 0 END AS hits FROM {}.task_text_http_fault_hits", f.artifacts))
            .fetch_one(f.owner())).await.unwrap().get("hits");
        assert_eq!(hits, 1, "both coherent deferred Task mutations must actually complete");
        assert_eq!(before, snapshot(f).await); p.unchanged(&old).await; p.counts(0).await;
        p.old_files_unchanged(&files, 1);
        let unpublished = p.blob(&p.reference(1, text.as_bytes()));
        assert_eq!(std::fs::read(&unpublished).unwrap(), text.as_bytes());
        f.sql(&format!("DROP TRIGGER task_text_http_fault ON {}.collaboration_artifact_outbox", f.artifacts)).await;
        let retained = p.files();
        error(&route, request(&p, 0, "POST", &path(&old), wire(&old, 1, text)), StatusCode::CONFLICT, "conflict", "unconfirmed").await;
        assert_eq!(before, snapshot(f).await); assert_eq!(retained, p.files()); p.counts(0).await;
        drop(route); assert!(super::storage_helpers::no_connections(f).await); assert!(p.finish());
    })).await;
}
