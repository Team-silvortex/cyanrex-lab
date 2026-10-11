use super::{fixture::*, support::*};
use cyanrex_engine::services::{
    file_upload_publication::FileUploadPublicationStoreError,
    file_upload_staging::{FileUploadPublicationError, FileUploadPublisher},
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::PgPool;
use std::time::Duration;

async fn poison(pool: &PgPool) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = pool.acquire().await.unwrap();
        query("SET default_transaction_isolation='repeatable read'")
            .execute(&mut *c)
            .await
            .unwrap();
        query("SET synchronous_commit='off'")
            .execute(&mut *c)
            .await
            .unwrap();
        held.push(c);
    }
}
async fn settings(pool: &PgPool) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = pool.acquire().await.unwrap();
        let row=query("SELECT current_setting('default_transaction_isolation') AS isolation,current_setting('synchronous_commit') AS sync")
            .fetch_one(&mut *c).await.unwrap();
        assert_eq!(row.get::<String, _>("isolation"), "repeatable read");
        assert_eq!(row.get::<String, _>("sync"), "off");
        held.push(c);
    }
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_independent_rc_writers_cannot_oversubscribe_publication_capacity() {
    let f = Fixture::with_limit(0, 2).await;
    publish(&f, &f.publisher, 1, &f.manifest).await;
    for seed in [2, 3] {
        f.source
            .reserve(upload(seed), owner(seed), account(seed), &manifest(0, seed))
            .await
            .unwrap();
    }
    let (store, pool) = reopen(&f).await;
    let independent = FileUploadPublisher::new(f.local.clone(), f.journal.clone(), store).unwrap();
    let mut first = f
        .publisher
        .prepare(upload(2), owner(2), account(2))
        .await
        .unwrap();
    let mut second = independent
        .prepare(upload(3), owner(3), account(3))
        .await
        .unwrap();
    poison(&f.publication_pool).await;
    poison(&pool).await;
    fault(&f,"IF current_setting('transaction_isolation')<>'read committed'
        OR current_setting('transaction_read_only')<>'off' OR current_setting('synchronous_commit')<>'on'
        THEN RAISE EXCEPTION 'explicit writable RC and synchronous publication required'; END IF;
        PERFORM set_config('synchronous_commit','off',true);",false).await;
    f.sql(
        Store::Publication,
        "CREATE SEQUENCE publication_commit_hits",
    )
    .await;
    f.sql(Store::Publication,"CREATE FUNCTION publication_commit_check() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('publication_commit_hits'); IF current_setting('synchronous_commit')<>'on'
        THEN RAISE EXCEPTION 'synchronous setting must be restored before deferred confirmation'; END IF; RETURN NEW; END $$").await;
    f.sql(
        Store::Publication,
        &format!(
            "CREATE CONSTRAINT TRIGGER publication_commit_check AFTER INSERT ON {PUBLICATION_ROWS}
        DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION publication_commit_check()"
        ),
    )
    .await;
    let before = snapshot_all(&f).await;
    let (holder, pid) = lock(&f, Store::Publication).await;
    let publisher = f.publisher.clone();
    let a = tokio::spawn(async move { publisher.publish(&mut first).await });
    let b = tokio::spawn(async move { independent.publish(&mut second).await });
    tokio::time::timeout(Duration::from_secs(5),async{loop {
        let n:i64=query("SELECT count(*) AS n FROM pg_stat_activity WHERE application_name=$1 AND $2=ANY(pg_blocking_pids(pid))")
            .bind(&f.publication_schema).bind(pid).fetch_one(&f.publication_admin).await.unwrap().get("n");
        if n==2{break;}tokio::task::yield_now().await;
    }}).await.expect("both independently pooled writers must actually wait on publication lock");
    holder.rollback().await.unwrap();
    let results = [a.await.unwrap(), b.await.unwrap()];
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(
                r,
                Err(FileUploadPublicationError::Publication(
                    FileUploadPublicationStoreError::CapacityExhausted
                ))
            ))
            .count(),
        1
    );
    assert!(hit(&f).await);
    assert!(query("SELECT is_called FROM publication_commit_hits")
        .fetch_one(&f.publication_pool)
        .await
        .unwrap()
        .get::<bool, _>("is_called"));
    let after = snapshot_all(&f).await;
    assert_eq!(after.rows[..2], before.rows[..2]);
    assert_eq!(after.oids, before.oids);
    assert_eq!(after.rows[2][0], before.rows[2][0]);
    assert_eq!(after.rows[2][1].len(), 2);
    assert!(after.rows[2][1].contains(&before.rows[2][1][0]));
    assert_eq!(after.files, before.files);
    assert_eq!(after.root, before.root);
    for (result, seed) in results.iter().zip([2, 3]) {
        let stored = f
            .publication
            .get(upload(seed), owner(seed), account(seed))
            .await
            .unwrap();
        if let Ok(receipt) = result {
            receipt_metadata(receipt, seed, &manifest(0, seed));
            record_metadata(&stored.unwrap(), seed, &manifest(0, seed));
        } else {
            assert_eq!(stored, None);
        }
    }
    record_metadata(
        &f.publication
            .get(upload(1), owner(1), account(1))
            .await
            .unwrap()
            .unwrap(),
        1,
        &f.manifest,
    );
    assert_eq!(snapshot_all(&f).await, after);
    settings(&f.publication_pool).await;
    settings(&pool).await;
    pool.close().await;
    f.pool_restored(Store::Publication).await;
    f.cleanup().await;
}
