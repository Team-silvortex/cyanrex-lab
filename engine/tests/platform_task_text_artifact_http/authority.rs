use super::fixture::*;
use axum::http::StatusCode;
use cyanrex_engine::models::collaboration::ArtifactRevision;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_text_artifact_http_current_owner_revision_and_duplicate_boundaries()
{
    for mode in [
        "cross-owner",
        "forged-owner",
        "missing",
        "stale",
        "coherent-tamper",
        "created-at",
        "updated-at",
        "ready",
        "revoked",
        "expired",
        "suspended",
        "unbound",
        "duplicate",
    ] {
        Fixture::checked(|f| Box::pin(async move {
            let p = Prepared::new(f).await; let old = p.task(10, 1, "Current private Task").await;
            let route = router(&p, 2).await;
            let created = call(&route, request(&p, 1, "POST", &path(&old), wire(&old, 1, "Preserved original")), StatusCode::CREATED).await;
            let original: ArtifactRevision = serde_json::from_value(created["artifact"].clone()).unwrap();
            let mut expected = old.clone(); let mut owner = 1; let mut n = 2;
            let (status, code) = match mode {
                "cross-owner" => { owner = 0; (StatusCode::NOT_FOUND, "not_found") },
                "forged-owner" => { owner = 0; expected.task.owner = p.owners[0]; (StatusCode::NOT_FOUND, "not_found") },
                "missing" => { expected.task.reference.task_id = task_id(99); (StatusCode::NOT_FOUND, "not_found") },
                "stale" => { expected.task.revision = 2.try_into().unwrap(); (StatusCode::CONFLICT, "stale_revision") },
                "coherent-tamper" => { expected.task.title = "Uncommitted but coherent title".into(); expected.manifest = manifest(&expected.task.title, vec![]); (StatusCode::BAD_REQUEST, "invalid_request") },
                "created-at" => { expected.task.created_at -= chrono::Duration::seconds(1); (StatusCode::BAD_REQUEST, "invalid_request") },
                "updated-at" => { expected.task.updated_at += chrono::Duration::seconds(1); (StatusCode::BAD_REQUEST, "invalid_request") },
                "ready" => { expected = p.ready(&old).await; (StatusCode::CONFLICT, "invalid_transition") },
                "revoked" => { f.sql(&format!("DELETE FROM {}.sessions WHERE token='{}'", f.source, token_hash(&p.tokens[1]))).await; (StatusCode::UNAUTHORIZED, "invalid_session") },
                "expired" => { f.sql(&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE token='{}'", f.source, token_hash(&p.tokens[1]))).await; (StatusCode::UNAUTHORIZED, "invalid_session") },
                "suspended" => { p.suspend(1).await; (StatusCode::FORBIDDEN, "access_denied") },
                "unbound" => { owner = 2; (StatusCode::FORBIDDEN, "access_denied") },
                _ => { n = 1; (StatusCode::CONFLICT, "conflict") },
            };
            let before = snapshot(f).await; let files = p.files();
            error(&route, request(&p, owner, "POST", &path(&expected), wire(&expected, n, "Denied new bytes")), status, code, "unconfirmed").await;
            assert_eq!(before, snapshot(f).await); assert_eq!(files, p.files()); p.counts(1).await;
            p.read_committed(&original, b"Preserved original").await;
            p.unchanged(if mode == "ready" { &expected } else { &old }).await;
            drop(route); assert!(super::storage_helpers::no_connections(f).await); assert!(p.finish());
        })).await;
    }
}
