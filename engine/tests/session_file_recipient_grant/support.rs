use crate::{fixture::*, *};
use futures_util::FutureExt;
use std::{future::Future, panic::AssertUnwindSafe, pin::Pin, time::Duration};
pub type Tx<'a> = sqlx_core::transaction::Transaction<'a, sqlx_postgres::Postgres>;
pub type ReadResult = Result<Option<StoredFileRecipientGrant>, SessionFileRecipientGrantError>;
pub async fn checked<F>(mut f: Fixture, work: F)
where
    F: for<'a> FnOnce(&'a mut Fixture) -> Pin<Box<dyn Future<Output = ()> + 'a>>,
{
    let result = AssertUnwindSafe(work(&mut f)).catch_unwind().await;
    f.cleanup().await;
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}
pub async fn read(f: &Fixture, token: &str, seed: u128) -> ReadResult {
    let before = f.snapshot().await;
    let result = read_direct(f, token, seed).await;
    assert_eq!(
        f.snapshot().await,
        before,
        "metadata reads cannot mutate Auth/Registry/recipient ledger rows or OIDs"
    );
    result
}
pub async fn read_direct(f: &Fixture, token: &str, seed: u128) -> ReadResult {
    f.auth
        .source
        .get_session_file_recipient_grant(token, &f.workspace, grant(seed))
        .await
}
pub async fn denied_workspace(
    f: &Fixture,
    w: &SessionFileRecipientGrantWorkspace,
    expected: SessionFileRecipientGrantError,
) {
    let before = f.snapshot().await;
    let result = f
        .auth
        .source
        .get_session_file_recipient_grant(&f.auth.learner_token, w, grant(1))
        .await;
    assert_eq!(f.snapshot().await, before);
    assert_eq!(result, Err(expected));
}
pub async fn backend(tx: &mut Tx<'_>) -> i32 {
    query("SELECT pg_backend_pid() AS n")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
        .get("n")
}
pub async fn blocked(f: &Fixture, pid: i32) -> bool {
    tokio::time::timeout(Duration::from_secs(1),async{loop{let yes:bool=query("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name=$1 AND state='active' AND wait_event_type='Lock' AND $2=ANY(pg_blocking_pids(pid))) AS yes").bind(&f.auth.base.schema).bind(pid).fetch_one(&f.auth.base.admin).await.unwrap().get("yes");if yes{return true;}tokio::task::yield_now().await;}}).await.unwrap_or(false)
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
            let yes: bool = query(&format!(
                "SELECT expires_at<=clock_timestamp() AS yes FROM {}.sessions WHERE token=$1",
                f.auth.base.schema
            ))
            .bind(source_fixture::token_hash(&f.auth.learner_token))
            .fetch_one(&f.auth.base.admin)
            .await
            .unwrap()
            .get("yes");
            if yes {
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
    .expect("cancelled read must release all selected-row/source/relation fences");
}
pub async fn snapshot_in_tx(f: &Fixture, tx: &mut Tx<'_>) -> Snapshot {
    let mut result = Vec::new();
    for schema in [&f.auth.base.schema, &f.schema] {
        let tables=query("SELECT relname AS n,oid::bigint AS oid FROM pg_class WHERE relnamespace=to_regnamespace($1) AND relkind='r' ORDER BY relname").bind(schema).fetch_all(&mut **tx).await.unwrap();
        for t in tables {
            let name: String = t.get("n");
            assert!(name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_'));
            let rows = query(&format!(
                "SELECT to_jsonb(t)::text AS v FROM {schema}.{name} t ORDER BY to_jsonb(t)::text"
            ))
            .fetch_all(&mut **tx)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.get("v"))
            .collect();
            result.push((format!("{schema}.{name}"), t.get("oid"), rows));
        }
    }
    Snapshot(result)
}
pub async fn configure(f: &Fixture, readonly: bool) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = f.auth.base.pool.acquire().await.unwrap();
        for sql in [
            "SET default_transaction_isolation='repeatable read'",
            "SET synchronous_commit='off'",
            if readonly {
                "SET default_transaction_read_only='on'"
            } else {
                "SET default_transaction_read_only='off'"
            },
        ] {
            query(sql).execute(&mut *c).await.unwrap();
        }
        held.push(c);
    }
}
pub async fn settings(f: &Fixture, readonly: bool) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = f.auth.base.pool.acquire().await.unwrap();
        let r=query("SELECT current_setting('default_transaction_isolation') AS i,current_setting('default_transaction_read_only') AS r,current_setting('synchronous_commit') AS s").fetch_one(&mut *c).await.unwrap();
        assert_eq!(r.get::<String, _>("i"), "repeatable read");
        assert_eq!(r.get::<String, _>("s"), "off");
        assert_eq!(r.get::<String, _>("r"), if readonly { "on" } else { "off" });
        held.push(c);
    }
}
