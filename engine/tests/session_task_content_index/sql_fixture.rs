use super::fixture::*;
use super::*;
use cyanrex_engine::services::task_store::TaskContentIndexPage;
use std::{os::unix::fs::MetadataExt, time::Duration};

pub type Tx<'a> = sqlx_core::transaction::Transaction<'a, sqlx_postgres::Postgres>;

pub fn task_id(n: u64) -> TaskId {
    format!("10000000-0000-0000-0000-{n:012x}").parse().unwrap()
}

#[derive(Debug, PartialEq, Eq)]
pub struct Snapshot {
    tables: Vec<(String, Vec<String>)>,
    files: Vec<(String, Vec<u8>, u32)>,
    root: (u64, u64, u32),
}

pub async fn snapshot(f: &Fixture) -> Snapshot {
    snapshot_with(f, &[]).await
}

pub async fn snapshot_with(f: &Fixture, extra: &[&str]) -> Snapshot {
    let mut schemas = vec![
        f.auth.base.schema.as_str(),
        f.base.task_schema.as_str(),
        f.artifact_schema.as_str(),
        f.task_schema.as_str(),
    ];
    schemas.extend_from_slice(extra);
    let mut tables = Vec::new();
    for schema in schemas {
        let names =
            query("SELECT tablename FROM pg_tables WHERE schemaname = $1 ORDER BY tablename")
                .bind(schema)
                .fetch_all(&f.auth.base.admin)
                .await
                .unwrap();
        for row in names {
            let table: String = row.get("tablename");
            let values = query(&format!(
                "SELECT row_to_json(t)::text AS value FROM \"{schema}\".\"{table}\" t ORDER BY row_to_json(t)::text"
            )).fetch_all(&f.auth.base.admin).await.unwrap()
                .into_iter().map(|row| row.get("value")).collect();
            tables.push((format!("{schema}.{table}"), values));
        }
    }
    let mut files: Vec<_> = fs::read_dir(&f.directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().into_string().unwrap(),
                fs::read(entry.path()).unwrap(),
                entry.metadata().unwrap().mode(),
            )
        })
        .collect();
    files.sort();
    let root = fs::metadata(&f.directory).unwrap();
    Snapshot {
        tables,
        files,
        root: (root.dev(), root.ino(), root.mode()),
    }
}

pub async fn list(
    f: &Fixture,
    workspace: &SessionTaskContentIndexWorkspace,
    token: &str,
    after: Option<TaskRef>,
    limit: Option<usize>,
) -> Result<TaskContentIndexPage, SessionTaskError> {
    let before = snapshot(f).await;
    let result = f
        .auth
        .source
        .list_session_content_tasks(token, workspace, after, limit)
        .await;
    assert_eq!(
        snapshot(f).await,
        before,
        "index must not modify any source/resource rows or file bytes/modes"
    );
    result
}

pub async fn create_at(
    f: &Fixture,
    number: u64,
    definition: Option<&TaskDefinition>,
    token: &str,
    value: TaskContentManifest,
) -> TaskContentSnapshot {
    match definition {
        Some(definition) => f
            .auth
            .source
            .create_session_catalog_content_task(
                token,
                &content(f, &values()),
                task_id(number),
                &definition.reference,
                value,
            )
            .await
            .unwrap(),
        None => f.request(token, task_id(number), value).await.unwrap(),
    }
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
                AND $1 = ANY(pg_blocking_pids(pid)) ORDER BY pid LIMIT 1",
            )
            .bind(blocker)
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

pub async fn lock_task(f: &Fixture, reference: TaskRef) -> Tx<'_> {
    let mut tx = f.task_pool.begin().await.unwrap();
    query("SELECT task_id FROM collaboration_tasks WHERE task_id = $1 FOR UPDATE")
        .bind(reference.task_id.as_uuid())
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    tx
}

pub async fn expire_soon(f: &Fixture) {
    assert_eq!(query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1")
        .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap().rows_affected(), 1);
}

pub async fn wait_expired(f: &Fixture) -> bool {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let expired: bool = query(&format!(
                "SELECT clock_timestamp() >= expires_at AS expired
                FROM {}.sessions WHERE token = $1",
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
