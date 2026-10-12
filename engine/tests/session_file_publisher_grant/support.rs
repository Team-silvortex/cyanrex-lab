use crate::{fixture::*, *};
use std::time::Duration;

pub type Tx<'a> = sqlx_core::transaction::Transaction<'a, sqlx_postgres::Postgres>;
pub type ReadResult = Result<Option<StoredFilePublisherGrant>, SessionFilePublisherGrantError>;

pub async fn read(f: &Fixture, token: &str, seed: u128) -> ReadResult {
    let before = f.snapshot().await;
    let result = f
        .auth
        .source
        .get_session_file_publisher_grant(token, &f.workspace, grant(seed))
        .await;
    assert_eq!(
        f.snapshot().await,
        before,
        "successful/absent/denied metadata read must be strictly read-only"
    );
    result
}
pub async fn read_direct(f: &Fixture, token: &str, seed: u128) -> ReadResult {
    f.auth
        .source
        .get_session_file_publisher_grant(token, &f.workspace, grant(seed))
        .await
}
pub async fn denied_workspace(
    f: &Fixture,
    workspace: &SessionFilePublisherGrantWorkspace,
    expected: SessionFilePublisherGrantError,
) {
    let before = f.snapshot().await;
    let result = f
        .auth
        .source
        .get_session_file_publisher_grant(&f.auth.learner_token, workspace, grant(1))
        .await;
    assert_eq!(result, Err(expected));
    assert_eq!(f.snapshot().await, before);
}
pub async fn backend(tx: &mut Tx<'_>) -> i32 {
    query("SELECT pg_backend_pid() AS n")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
        .get("n")
}
pub async fn blocked(f: &Fixture, holder: i32) -> bool {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let found: bool = query("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name=$1 AND $2=ANY(pg_blocking_pids(pid))) AS yes")
                .bind(&f.auth.base.schema).bind(holder).fetch_one(&f.auth.base.admin).await.unwrap().get("yes");
            if found { return true; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap_or(false)
}
pub async fn holder<'a>(f: &'a Fixture, table: &str) -> Tx<'a> {
    assert!(matches!(table, MARKER | HEADS | CHANGES));
    let mut tx = f.auth.base.admin.begin().await.unwrap();
    let filter = if table == MARKER {
        "singleton"
    } else {
        "grant_id=$1"
    };
    let sql = format!(
        "SELECT * FROM {}.{table} WHERE {filter} FOR UPDATE",
        f.schema
    );
    if table == MARKER {
        query(&sql).fetch_all(&mut *tx).await.unwrap();
    } else {
        query(&sql)
            .bind(grant(1).as_uuid())
            .fetch_all(&mut *tx)
            .await
            .unwrap();
    }
    tx
}
pub async fn expire_soon(f: &Fixture) {
    query("UPDATE sessions SET expires_at=clock_timestamp()+INTERVAL '1.3 seconds' WHERE token=$1")
        .bind(source_fixture::token_hash(&f.auth.learner_token))
        .execute(&f.auth.base.pool)
        .await
        .unwrap();
}
pub async fn expired(f: &Fixture) -> bool {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let elapsed: bool = query(&format!(
                "SELECT expires_at<=clock_timestamp() AS yes FROM {}.sessions WHERE token=$1",
                f.auth.base.schema
            ))
            .bind(source_fixture::token_hash(&f.auth.learner_token))
            .fetch_one(&f.auth.base.admin)
            .await
            .unwrap()
            .get("yes");
            if elapsed {
                return true;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or(false)
}
pub async fn refresh_expiry(f: &Fixture) {
    query("UPDATE sessions SET expires_at=clock_timestamp()+INTERVAL '12 hours' WHERE token=$1")
        .bind(source_fixture::token_hash(&f.auth.learner_token))
        .execute(&f.auth.base.pool)
        .await
        .unwrap();
}
pub async fn drain(f: &Fixture) {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut tx = f.auth.base.admin.begin().await.unwrap();
        query(&format!(
            "LOCK TABLE {}.{MARKER},{}.{HEADS},{}.{CHANGES} IN ACCESS EXCLUSIVE MODE",
            f.schema, f.schema, f.schema
        ))
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.rollback().await.unwrap();
        f.auth.base.drain().await;
    })
    .await
    .expect("cancelled metadata read retained source or selected grant locks");
}
/// Capture the writer's injection BEFORE COMMIT, not a potentially mutated post-reader state.
pub async fn snapshot_in_tx(f: &Fixture, tx: &mut Tx<'_>) -> Snapshot {
    snapshot_schemas_in_tx(&[&f.auth.base.schema, &f.schema], tx).await
}
pub async fn snapshot_schemas_in_tx(schemas: &[&str], tx: &mut Tx<'_>) -> Snapshot {
    let mut result = Vec::new();
    for schema in schemas {
        assert!(schema
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_'));
        let tables = query("SELECT c.relname,c.oid::bigint AS oid FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=$1 AND c.relkind='r' ORDER BY c.relname")
            .bind(schema).fetch_all(&mut **tx).await.unwrap();
        for table in tables {
            let name: String = table.get("relname");
            assert!(name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
            let rows = query(&format!(
                "SELECT to_jsonb(t)::text AS row FROM {schema}.{name} t ORDER BY to_jsonb(t)::text"
            ))
            .fetch_all(&mut **tx)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.get("row"))
            .collect();
            result.push((format!("{schema}.{name}"), table.get("oid"), rows));
        }
    }
    Snapshot(result)
}
