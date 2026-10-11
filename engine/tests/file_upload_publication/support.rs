use super::{bytes::*, fixture::*};
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        file_upload_publication::FileUploadPublicationRecord,
        file_upload_staging::{
            FileUploadPublicationError, FileUploadPublicationReceipt, FileUploadPublicationState,
            FileUploadPublisher,
        },
    },
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, Postgres};
use std::io::{Read, Seek, SeekFrom};

pub type Tx<'a> = sqlx_core::transaction::Transaction<'a, Postgres>;

#[derive(Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub rows: Vec<Vec<Vec<String>>>,
    pub oids: Vec<Vec<i64>>,
    pub files: Vec<Entry>,
    pub root: Entry,
}
pub async fn snapshot_all(f: &Fixture) -> Snapshot {
    let mut rows = Vec::new();
    let mut oids = Vec::new();
    for store in [Store::Source, Store::Journal, Store::Publication] {
        rows.push(f.rows(store).await);
        oids.push(f.oids(store).await);
    }
    Snapshot {
        rows,
        oids,
        files: tree(&f.root),
        root: snapshot(&f.root),
    }
}
pub async fn stage(f: &Fixture, seed: u128, m: &FileTransferManifest) {
    for index in 0..m.chunk_sha256().len() as u32 {
        let mut source = f.real_file(m.byte_length());
        source
            .seek(SeekFrom::Start(index as u64 * FILE_TRANSFER_CHUNK_BYTES))
            .unwrap();
        let mut attempt = f
            .journaled
            .prepare(upload(seed), owner(seed), account(seed), index)
            .await
            .unwrap();
        let receipt = f
            .journaled
            .stage_chunk(
                &mut attempt,
                source.take(m.chunk(index).unwrap().byte_length),
            )
            .await
            .unwrap();
        assert_eq!(receipt.manifest(), m);
        drop(receipt);
    }
}
pub async fn stage_all(f: &Fixture) {
    stage(f, 1, &f.manifest).await;
}
pub fn receipt_metadata(r: &FileUploadPublicationReceipt, seed: u128, m: &FileTransferManifest) {
    assert_eq!(r.upload_id(), upload(seed));
    assert_eq!(r.owner(), owner(seed));
    assert_eq!(r.account_id(), account(seed));
    assert_eq!(r.manifest(), m);
}
pub fn record_metadata(r: &FileUploadPublicationRecord, seed: u128, m: &FileTransferManifest) {
    assert_eq!(r.upload_id(), upload(seed));
    assert_eq!(r.owner(), owner(seed));
    assert_eq!(r.account_id(), account(seed));
    assert_eq!(r.manifest(), m);
}
pub async fn publish(
    f: &Fixture,
    publisher: &FileUploadPublisher,
    seed: u128,
    m: &FileTransferManifest,
) {
    let before = snapshot_all(f).await;
    let mut attempt = publisher
        .prepare(upload(seed), owner(seed), account(seed))
        .await
        .unwrap();
    assert_eq!(attempt.state(), FileUploadPublicationState::Ready);
    assert_eq!(attempt.upload_id(), upload(seed));
    assert_eq!(attempt.owner(), owner(seed));
    assert_eq!(attempt.account_id(), account(seed));
    assert_eq!(attempt.manifest(), m);
    assert_eq!(
        snapshot_all(f).await,
        before,
        "prepare performs no filesystem work or publication writes"
    );
    let receipt = publisher.publish(&mut attempt).await.unwrap();
    receipt_metadata(&receipt, seed, m);
    assert_eq!(attempt.state(), FileUploadPublicationState::Confirmed);
    let after = snapshot_all(f).await;
    assert_eq!(after.rows[..2], before.rows[..2]);
    assert_eq!(after.oids, before.oids);
    assert_eq!(after.rows[2][0], before.rows[2][0]);
    assert_eq!(after.rows[2][1].len(), before.rows[2][1].len() + 1);
    assert_eq!(after.files, before.files);
    assert_eq!(after.root, before.root);
    let record = f
        .publication
        .get(upload(seed), owner(seed), account(seed))
        .await
        .unwrap()
        .unwrap();
    record_metadata(&record, seed, m);
    assert_eq!(snapshot_all(f).await, after);
    assert_eq!(
        publisher.publish(&mut attempt).await.unwrap_err(),
        FileUploadPublicationError::InvalidInput
    );
    assert_eq!(snapshot_all(f).await, after);
}
pub async fn healthy(f: &Fixture, seed: u128) {
    let m = manifest(17, seed);
    f.source
        .reserve(upload(seed), owner(seed), account(seed), &m)
        .await
        .unwrap();
    stage(f, seed, &m).await;
    publish(f, &f.publisher, seed, &m).await;
}
pub async fn backend(tx: &mut Tx<'_>) -> i32 {
    query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
        .get("pid")
}
pub async fn lock(f: &Fixture, store: Store) -> (Tx<'_>, i32) {
    let mut tx = f.admin(store).begin().await.unwrap();
    let [marker, rows] = Fixture::tables(store);
    let schema = f.schema(store);
    query(&format!(
        "LOCK TABLE {schema}.{marker},{schema}.{rows} IN ACCESS EXCLUSIVE MODE"
    ))
    .execute(&mut *tx)
    .await
    .unwrap();
    let pid = backend(&mut tx).await;
    (tx, pid)
}
pub async fn state_in_tx(
    f: &Fixture,
    store: Store,
    tx: &mut Tx<'_>,
) -> (Vec<Vec<String>>, Vec<i64>) {
    let mut rows = Vec::new();
    let mut oids = Vec::new();
    for table in Fixture::tables(store) {
        let mut values: Vec<String> = query(&format!(
            "SELECT row_to_json(t)::text AS value FROM {}.{table} t",
            f.schema(store)
        ))
        .fetch_all(&mut **tx)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get("value"))
        .collect();
        values.sort();
        rows.push(values);
        oids.push(
            query("SELECT to_regclass($1)::oid::bigint AS value")
                .bind(format!("{}.{table}", f.schema(store)))
                .fetch_one(&mut **tx)
                .await
                .unwrap()
                .get("value"),
        );
    }
    (rows, oids)
}
pub async fn reopen(
    f: &Fixture,
) -> (
    cyanrex_engine::services::file_upload_publication::FileUploadPublicationStore,
    PgPool,
) {
    let pool = pool_for(
        &std::env::var("CYANREX_TEST_DATABASE_URL").expect("explicit database"),
        &f.publication_schema,
        4,
    )
    .await;
    let store = cyanrex_engine::services::file_upload_publication::FileUploadPublicationStore::new(
        pool.clone(),
        scope(),
        f.publication_installation_id,
        &f.publication_schema,
        f.max_publications,
    )
    .unwrap();
    (store, pool)
}
pub async fn advisory(f: &Fixture) -> (Tx<'_>, i32) {
    let mut tx = f.publication_admin.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext($1),877)")
        .bind(&f.publication_schema)
        .execute(&mut *tx)
        .await
        .unwrap();
    let pid = backend(&mut tx).await;
    (tx, pid)
}
pub async fn fault(f: &Fixture, body: &str, deferred: bool) {
    f.sql(Store::Publication, "CREATE SEQUENCE publication_fault_hits")
        .await;
    f.sql(
        Store::Publication,
        &format!(
            "CREATE FUNCTION publication_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('publication_fault_hits'); {body} RETURN NEW; END $$"
        ),
    )
    .await;
    f.sql(Store::Publication,&if deferred {format!("CREATE CONSTRAINT TRIGGER publication_fault AFTER INSERT ON {PUBLICATION_ROWS}
        DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION publication_fault()")}
        else{format!("CREATE TRIGGER publication_fault BEFORE INSERT ON {PUBLICATION_ROWS} FOR EACH ROW EXECUTE FUNCTION publication_fault()")}).await;
}
pub async fn hit(f: &Fixture) -> bool {
    query("SELECT is_called FROM publication_fault_hits")
        .fetch_one(&f.publication_pool)
        .await
        .unwrap()
        .get("is_called")
}
pub async fn remove_fault(f: &Fixture) {
    f.sql(
        Store::Publication,
        &format!("DROP TRIGGER publication_fault ON {PUBLICATION_ROWS}"),
    )
    .await;
    f.sql(Store::Publication, "DROP FUNCTION publication_fault()")
        .await;
    f.sql(Store::Publication, "DROP SEQUENCE publication_fault_hits")
        .await;
}
