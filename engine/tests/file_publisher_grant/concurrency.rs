use super::{fixture::*, support::*, *};
use std::time::Duration;

async fn writer_backends(pool: &PgPool) -> Vec<i32> {
    let mut held = Vec::new();
    let mut pids = Vec::new();
    for _ in 0..4 {
        let mut connection = pool.acquire().await.unwrap();
        pids.push(
            query("SELECT pg_backend_pid() AS pid")
                .fetch_one(&mut *connection)
                .await
                .unwrap()
                .get("pid"),
        );
        held.push(connection);
    }
    pids
}

async fn independent_writer_chains_wait(f: &Fixture, holder: i32, a: &[i32], b: &[i32]) -> bool {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            // The first writer can hold the marker while waiting for HEADS; the second
            // then waits for that first writer. Require actual Lock waits from BOTH
            // independent pools whose recursive blocker chains reach the real holder.
            let row = query(
                "WITH RECURSIVE waiting AS (
                SELECT pid FROM pg_stat_activity WHERE application_name=$1
                AND state='active' AND wait_event_type='Lock'
                AND query LIKE '%LOCK TABLE collaboration_file_publisher_grant_schema%'),
                chains(writer,pid,visited) AS (
                    SELECT w.pid,b.pid,ARRAY[w.pid,b.pid] FROM waiting w
                    CROSS JOIN LATERAL unnest(pg_blocking_pids(w.pid)) b(pid)
                    UNION ALL SELECT c.writer,b.pid,c.visited || b.pid FROM chains c
                    CROSS JOIN LATERAL unnest(pg_blocking_pids(c.pid)) b(pid)
                    WHERE NOT b.pid=ANY(c.visited))
                SELECT count(DISTINCT writer) FILTER (WHERE writer=ANY($3)) AS a,
                       count(DISTINCT writer) FILTER (WHERE writer=ANY($4)) AS b
                FROM chains WHERE pid=$2",
            )
            .bind(&f.schema)
            .bind(holder)
            .bind(a)
            .bind(b)
            .fetch_one(&f.admin)
            .await
            .unwrap();
            if row.get::<i64, _>("a") == 1 && row.get::<i64, _>("b") == 1 {
                return true;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or(false)
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_queued_rr_publishers_use_fresh_ledger_and_only_one_wins_quota() {
    let f = ready(2).await;
    let old = issue(&f, 1, account(1), account(1), validity())
        .await
        .unwrap();
    let (other, pool) = f.reopen().await;
    for p in [&f.pool, &pool] {
        configure(p, "SET default_transaction_isolation='repeatable read'").await;
        configure(p, "SET synchronous_commit='off'").await;
    }
    let original_backends = writer_backends(&f.pool).await;
    let other_backends = writer_backends(&pool).await;
    let before = snapshot(&f).await;
    let mut holder = f.admin.begin().await.unwrap();
    let pid = backend(&mut holder).await;
    query(&format!(
        "LOCK TABLE {}.{HEADS} IN ACCESS EXCLUSIVE MODE",
        f.schema
    ))
    .execute(&mut *holder)
    .await
    .unwrap();
    let mut a = Box::pin(f.store.issue(grant(2), account(2), account(2), validity()));
    let mut b = Box::pin(other.issue(grant(3), account(3), account(3), validity()));
    let waited = tokio::select! { _=&mut a=>false,_=&mut b=>false,
    w=independent_writer_chains_wait(&f,pid,&original_backends,&other_backends)=>w };
    holder.commit().await.unwrap();
    let (a, b) = tokio::time::timeout(Duration::from_secs(5), async { tokio::join!(a, b) })
        .await
        .unwrap();
    let after = snapshot(&f).await;
    let mut winner = None;
    for (seed, result) in [(2, a), (3, b)] {
        match result {
            Ok(record) => {
                assert!(winner.replace(seed).is_none());
                assert_eq!(record.grant().id(), grant(seed));
                assert_eq!(record.issuer(), account(seed));
                assert_eq!(record.grant().subject(), account(seed));
                assert_eq!(record.grant().revision(), revision(1));
                assert_eq!(get(&f, seed, account(seed)).await.unwrap(), Some(record));
            }
            Err(e) => {
                assert_eq!(e, FilePublisherGrantStoreError::QuotaExceeded);
                assert!(get(&f, seed, account(seed)).await.unwrap().is_none());
            }
        }
    }
    assert!(winner.is_some());
    assert_eq!(after.oids, before.oids);
    assert_eq!(rows(&after, MARKER), rows(&before, MARKER));
    for table in [HEADS, CHANGES] {
        assert_eq!(rows(&after, table).len(), 2);
        assert!(rows(&after, table).contains(&rows(&before, table)[0]));
    }
    assert_eq!(get(&f, 1, account(1)).await.unwrap(), Some(old));
    for p in [&f.pool, &pool] {
        settings(p, "repeatable read", "off", "off").await;
    }
    pool.close().await;
    f.cleanup().await;
    assert!(
        waited,
        "one writer from each independent pool must have a real lock-wait chain to the holder"
    );

    let f = ready(2).await;
    configure(&f.pool, "SET default_transaction_read_only='on'").await;
    let before = snapshot(&f).await;
    let result = f
        .store
        .issue(grant(1), account(1), account(1), validity())
        .await;
    assert_eq!(snapshot(&f).await, before);
    assert_eq!(
        result,
        Err(FilePublisherGrantStoreError::StorageUnavailable)
    );
    assert!(get(&f, 1, account(1)).await.unwrap().is_none());
    settings(&f.pool, "read committed", "on", "on").await;
    configure(&f.pool, "SET default_transaction_read_only='off'").await;
    issue(&f, 1, account(1), account(1), validity())
        .await
        .unwrap();
    f.cleanup().await;
}
