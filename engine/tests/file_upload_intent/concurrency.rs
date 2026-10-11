use super::fixture::*;
use cyanrex_engine::services::file_upload_store::FileUploadStoreError;
use sqlx_core::{query::query, row::Row};

async fn repeatable_defaults(pool: &sqlx_postgres::PgPool) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut connection = pool.acquire().await.unwrap();
        query("SET default_transaction_isolation = 'repeatable read'")
            .execute(&mut *connection)
            .await
            .unwrap();
        query("SET synchronous_commit = 'off'")
            .execute(&mut *connection)
            .await
            .unwrap();
        held.push(connection);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_concurrent_repeatable_read_defaults_cannot_oversubscribe_quota() {
    for different_owners in [false, true] {
        let fixture = Fixture::ready(quota_limits(12, 12, 16, 8)).await;
        fixture
            .store
            .reserve(upload(1), owner(1), account(1), &manifest(4, 1))
            .await
            .unwrap();
        let (independent, pool) = fixture.reopen().await;
        repeatable_defaults(&fixture.pool).await;
        repeatable_defaults(&pool).await;
        fixture.fault("IF current_setting('transaction_isolation') <> 'read committed'
            OR current_setting('transaction_read_only') <> 'off'
            OR current_setting('synchronous_commit') <> 'on'
            THEN RAISE EXCEPTION 'reservation must explicitly select writable RC and synchronous confirmation'; END IF;
            PERFORM set_config('synchronous_commit', 'off', true);", false, false).await;
        fixture.sql("CREATE SEQUENCE upload_commit_hits").await;
        fixture.sql("CREATE FUNCTION upload_commit_check() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM nextval('upload_commit_hits');
            IF current_setting('synchronous_commit') <> 'on' THEN RAISE EXCEPTION 'commit confirmation must restore synchronous setting'; END IF;
            RETURN NEW; END $$").await;
        fixture
            .sql(&format!(
                "CREATE CONSTRAINT TRIGGER upload_commit_check AFTER INSERT ON {LEDGER}
            DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION upload_commit_check()"
            ))
            .await;
        let before = fixture.rows().await;
        let mut holder = fixture.pool.begin().await.unwrap();
        query(&format!(
            "LOCK TABLE {MARKER}, {LEDGER} IN ACCESS EXCLUSIVE MODE"
        ))
        .execute(&mut *holder)
        .await
        .unwrap();
        let blocker = Fixture::backend(&mut holder).await;
        let first_store = fixture.store.clone();
        let first = tokio::spawn(async move {
            first_store
                .reserve(upload(2), owner(1), account(2), &manifest(8, 2))
                .await
        });
        let second_owner = owner(if different_owners { 2 } else { 1 });
        let second = tokio::spawn(async move {
            independent
                .reserve(upload(3), second_owner, account(3), &manifest(8, 3))
                .await
        });
        fixture.wait_blocked(blocker, 2).await;
        holder.rollback().await.unwrap();
        let outcomes = [first.await.unwrap(), second.await.unwrap()];
        assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            outcomes
                .iter()
                .filter(|result| **result == Err(FileUploadStoreError::QuotaExceeded))
                .count(),
            1
        );
        assert!(fixture.hit().await);
        let commit_hit: bool = query("SELECT is_called FROM upload_commit_hits")
            .fetch_one(&fixture.pool)
            .await
            .unwrap()
            .get("is_called");
        assert!(commit_hit);
        let after = fixture.rows().await;
        assert_eq!(after[0], before[0]);
        assert_eq!(after[1].len(), before[1].len() + 1);
        let snapshot = fixture.rows().await;
        assert!(fixture
            .store
            .get(upload(1), owner(1), account(1))
            .await
            .unwrap()
            .is_some());
        assert_eq!(fixture.rows().await, snapshot);
        for selected_pool in [&fixture.pool, &pool] {
            let mut held = Vec::new();
            for _ in 0..4 {
                let mut connection = selected_pool.acquire().await.unwrap();
                let row = query(
                    "SELECT current_setting('default_transaction_isolation') AS isolation,
                    current_setting('synchronous_commit') AS sync",
                )
                .fetch_one(&mut *connection)
                .await
                .unwrap();
                assert_eq!(row.get::<String, _>("isolation"), "repeatable read");
                assert_eq!(row.get::<String, _>("sync"), "off");
                held.push(connection);
            }
        }
        pool.close().await;
        fixture.pool_restored().await;
        fixture.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_cancelled_real_reservation_wait_rolls_back_and_restores_pool() {
    let fixture = Fixture::ready(quota_limits(12, 12, 3, 3)).await;
    fixture
        .store
        .reserve(upload(1), owner(1), account(1), &manifest(4, 1))
        .await
        .unwrap();
    let before = fixture.rows().await;
    let candidate = manifest(8, 2);
    drop(
        fixture
            .store
            .reserve(upload(2), owner(1), account(2), &candidate),
    );
    assert_eq!(fixture.rows().await, before);
    fixture
        .fault(
            "PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 887);",
            false,
            true,
        )
        .await;
    let mut holder = fixture.pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext($1), 887)")
        .bind(&fixture.schema)
        .execute(&mut *holder)
        .await
        .unwrap();
    let blocker = Fixture::backend(&mut holder).await;
    let mut pending = Box::pin(
        fixture
            .store
            .reserve(upload(2), owner(1), account(2), &candidate),
    );
    tokio::select! {
        result = &mut pending => panic!("reservation completed before deliberate insert wait: {result:?}"),
        _ = fixture.wait_blocked(blocker, 1) => {},
    }
    drop(pending);
    holder.rollback().await.unwrap();
    fixture.drain().await;
    fixture.pool_restored().await;
    assert!(fixture.hit().await);
    assert_eq!(fixture.rows().await, before);
    fixture.remove_fault().await;
    fixture
        .store
        .reserve(upload(2), owner(1), account(2), &candidate)
        .await
        .unwrap();
    assert!(fixture
        .store
        .get(upload(1), owner(1), account(1))
        .await
        .unwrap()
        .is_some());
    assert!(fixture
        .store
        .get(upload(2), owner(1), account(2))
        .await
        .unwrap()
        .is_some());
    assert_eq!(fixture.rows().await[1].len(), 2);
    fixture.cleanup().await;
    // This observed pre-COMMIT cancellation proves rollback only for this controlled fixture.
}
