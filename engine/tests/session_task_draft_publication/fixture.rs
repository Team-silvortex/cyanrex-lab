use super::*;

pub use content_fixture::Fixture;
pub type Tx<'a> = sqlx_core::transaction::Transaction<'a, sqlx_postgres::Postgres>;
pub const TITLE: &str = "  Publication · 中文 📝  ";

pub fn plan(items: &[(&str, &str, &str)]) -> TaskDraftPublicationPlan {
    let payload: Vec<_> = items
        .iter()
        .enumerate()
        .map(|(index, (filename, language, text))| {
            serde_json::json!({
                "id": format!("local_{index}"), "revision": 9007199254740991_u64,
                "kind": "text", "filename": filename, "language": language, "text": text,
            })
        })
        .collect();
    TaskDraftPublicationPlan::parse_json(
        &serde_json::to_vec(&serde_json::json!({
            "format": "cyanrex.task-draft", "version": 1, "title": TITLE, "payload": payload,
        }))
        .unwrap(),
    )
    .unwrap()
}

pub fn prepare(f: &Fixture, items: &[(&str, &str, &str)]) -> SessionTaskDraftPublication {
    f.auth
        .source
        .prepare_task_draft_publication(&f.auth.learner_token, f.workspace.clone(), plan(items))
        .unwrap()
}

// Task and Artifact records/outboxes are separate commits. File count includes unconfirmed blobs.
pub async fn counts(f: &Fixture) -> [i64; 6] {
    [
        f.count_artifact("collaboration_artifacts").await,
        f.count_artifact("collaboration_artifact_revisions").await,
        f.count_artifact("collaboration_artifact_outbox").await,
        f.count_task("collaboration_tasks").await,
        f.count_task("collaboration_task_outbox").await,
        f.files() as i64,
    ]
}

pub fn uncertain_artifact(index: usize, reference: ArtifactRef) -> SessionDraftPublicationState {
    SessionDraftPublicationState::Unconfirmed(SessionDraftPublicationStep::Artifact {
        index,
        reference,
    })
}

pub fn uncertain_task(reference: TaskRef) -> SessionDraftPublicationState {
    SessionDraftPublicationState::Unconfirmed(SessionDraftPublicationStep::Task { reference })
}

pub async fn backend(tx: &mut Tx<'_>) -> i32 {
    query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
        .get("pid")
}

pub async fn blocked_by(f: &Fixture, blocker: i32) -> Option<i32> {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(row) = query(
                "SELECT pid FROM pg_stat_activity WHERE datname = current_database()
                 AND application_name = $2 AND $1 = ANY(pg_blocking_pids(pid))
                 ORDER BY pid LIMIT 1",
            )
            .bind(blocker)
            .bind(&f.auth.base.schema)
            .fetch_optional(&f.auth.base.admin)
            .await
            .unwrap()
            {
                return row.get("pid");
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .ok()
}

pub async fn pause_artifact_event(f: &Fixture) -> Tx<'_> {
    f.sql_artifact(
        "CREATE FUNCTION pause_draft_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
         PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 833); RETURN NEW; END $$",
    )
    .await;
    f.sql_artifact(
        "CREATE TRIGGER pause_draft_event BEFORE INSERT ON collaboration_artifact_outbox
         FOR EACH ROW EXECUTE FUNCTION pause_draft_event()",
    )
    .await;
    let mut holder = f.artifact_pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext($1), 833)")
        .bind(&f.artifact_schema)
        .execute(&mut *holder)
        .await
        .unwrap();
    holder
}

pub async fn artifact_fault_calls(f: &Fixture) -> (bool, i64) {
    let row = query("SELECT is_called, last_value FROM draft_fault_calls")
        .fetch_one(&f.artifact_pool)
        .await
        .unwrap();
    (row.get("is_called"), row.get("last_value"))
}

pub async fn wait_expired(f: &Fixture) -> bool {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let expired: bool = query(&format!(
                "SELECT clock_timestamp() >= expires_at AS expired FROM {}.sessions
                 WHERE token = $1",
                f.auth.base.schema
            ))
            .bind(source_fixture::token_hash(&f.auth.learner_token))
            .fetch_one(&f.auth.base.admin)
            .await
            .unwrap()
            .get("expired");
            if expired {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .is_ok()
}
