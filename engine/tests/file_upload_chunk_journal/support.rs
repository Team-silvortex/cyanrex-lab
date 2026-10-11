use super::fixture::*;
use cyanrex_engine::services::file_upload_chunk_journal::{
    PersistentChunkState, UploadChunkRecord,
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::Postgres;
use std::time::Duration;
use std::{
    io::{self, Read},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

pub type Tx<'a> = sqlx_core::transaction::Transaction<'a, Postgres>;
pub async fn backend(tx: &mut Tx<'_>) -> i32 {
    query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
        .get("pid")
}
pub async fn advisory(fixture: &Fixture) -> (Tx<'_>, i32) {
    let mut tx = fixture.journal_admin.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext($1),998)")
        .bind(&fixture.journal_schema)
        .execute(&mut *tx)
        .await
        .unwrap();
    let pid = backend(&mut tx).await;
    (tx, pid)
}
pub async fn lock(fixture: &Fixture, journal: bool) -> (Tx<'_>, i32) {
    let (schema, tables) = if journal {
        (&fixture.journal_schema, [JOURNAL_MARKER, JOURNAL_ROWS])
    } else {
        (&fixture.source_schema, [SOURCE_MARKER, SOURCE_ROWS])
    };
    let pool = if journal {
        &fixture.journal_admin
    } else {
        &fixture.source_admin
    };
    let mut tx = pool.begin().await.unwrap();
    query(&format!(
        "LOCK TABLE {schema}.{}, {schema}.{} IN ACCESS EXCLUSIVE MODE",
        tables[0], tables[1]
    ))
    .execute(&mut *tx)
    .await
    .unwrap();
    let pid = backend(&mut tx).await;
    (tx, pid)
}
pub async fn fault(fixture: &Fixture, event: &str, body: &str, deferred: bool) {
    assert!(["INSERT", "UPDATE"].contains(&event));
    fixture.sql(true, "CREATE SEQUENCE chunk_fault_hits").await;
    fixture
        .sql(
            true,
            &format!(
                "CREATE FUNCTION chunk_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.chunk_fault_hits'); {body} RETURN NEW; END $$",
                fixture.journal_schema
            ),
        )
        .await;
    fixture.sql(true,&format!("CREATE {}TRIGGER chunk_fault {} {event} ON {JOURNAL_ROWS} {}FOR EACH ROW EXECUTE FUNCTION chunk_fault()",
        if deferred {"CONSTRAINT "}else {""},if deferred {"AFTER"}else {"BEFORE"},if deferred {"DEFERRABLE INITIALLY DEFERRED "}else {""})).await;
}
pub async fn hit(fixture: &Fixture) -> bool {
    query("SELECT is_called FROM chunk_fault_hits")
        .fetch_one(&fixture.journal_pool)
        .await
        .unwrap()
        .get("is_called")
}
pub async fn remove_fault(fixture: &Fixture) {
    fixture
        .sql(true, &format!("DROP TRIGGER chunk_fault ON {JOURNAL_ROWS}"))
        .await;
    fixture.sql(true, "DROP FUNCTION chunk_fault()").await;
    fixture.sql(true, "DROP SEQUENCE chunk_fault_hits").await;
}
pub async fn drain(fixture: &Fixture) {
    tokio::time::timeout(Duration::from_secs(5), async {
        let (tx, _) = lock(fixture, true).await;
        tx.rollback().await.unwrap();
    })
    .await
    .expect("cancelled journal operation retained a transaction lock");
}
pub async fn admin_rows(fixture: &Fixture, journal: bool) -> Vec<Vec<String>> {
    let (schema, tables) = if journal {
        (&fixture.journal_schema, [JOURNAL_MARKER, JOURNAL_ROWS])
    } else {
        (&fixture.source_schema, [SOURCE_MARKER, SOURCE_ROWS])
    };
    let mut all = Vec::new();
    for table in tables {
        let mut rows: Vec<String> = query(&format!(
            "SELECT row_to_json(t)::text AS value FROM {schema}.{table} t"
        ))
        .fetch_all(&fixture.source_admin)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get("value"))
        .collect();
        rows.sort();
        all.push(rows);
    }
    all
}
pub fn metadata(
    record: &UploadChunkRecord,
    index: u32,
    expected: &cyanrex_engine::models::collaboration::FileTransferManifest,
    state: PersistentChunkState,
) {
    assert_eq!(record.upload_id(), upload(1));
    assert_eq!(record.owner(), owner(1));
    assert_eq!(record.account_id(), account(1));
    assert_eq!(record.chunk_index(), index);
    let spec = expected.chunk(index).unwrap();
    assert_eq!(record.byte_length(), spec.byte_length);
    assert_eq!(record.sha256(), &spec.sha256);
    assert_eq!(record.state(), state);
}
pub async fn reopen(
    fixture: &Fixture,
) -> (
    cyanrex_engine::services::file_upload_chunk_journal::FileUploadChunkJournal,
    sqlx_postgres::PgPool,
) {
    let url =
        std::env::var("CYANREX_TEST_DATABASE_URL").expect("explicit disposable database required");
    let pool = pool_for(&url, &fixture.journal_schema, 4).await;
    let journal = cyanrex_engine::services::file_upload_chunk_journal::FileUploadChunkJournal::new(
        pool.clone(),
        scope(),
        fixture.journal_installation_id,
        &fixture.journal_schema,
        fixture.entries,
    )
    .unwrap();
    (journal, pool)
}

pub struct Counted<R> {
    pub reader: R,
    pub calls: Arc<AtomicUsize>,
}
impl<R: Read> Read for Counted<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.reader.read(bytes)
    }
}
pub fn counted<R>(reader: R) -> (Counted<R>, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    (
        Counted {
            reader,
            calls: calls.clone(),
        },
        calls,
    )
}
pub struct FailedReader;
impl Read for FailedReader {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::ErrorKind::Interrupted.into())
    }
}

pub async fn healthy(fixture: &Fixture, seed: u128) {
    let manifest = manifest(17, seed);
    fixture
        .source
        .reserve(upload(seed), owner(1), account(seed), &manifest)
        .await
        .unwrap();
    let source = fixture.rows(false).await;
    let source_oids = fixture.oids(false).await;
    let journal = fixture.rows(true).await;
    let old = super::bytes::tree(&fixture.root);
    let mut attempt = fixture
        .journaled
        .prepare(upload(seed), owner(1), account(seed), 0)
        .await
        .unwrap();
    let receipt = fixture
        .journaled
        .stage_chunk(&mut attempt, super::bytes::Pattern::new(0, 17))
        .await
        .unwrap();
    assert_eq!(receipt.upload_id(), upload(seed));
    assert_eq!(
        attempt.state(),
        cyanrex_engine::services::file_upload_staging::UploadChunkState::Confirmed
    );
    assert_eq!(fixture.rows(false).await, source);
    assert_eq!(fixture.oids(false).await, source_oids);
    let after = fixture.rows(true).await;
    assert_eq!(after[0], journal[0]);
    assert_eq!(after[1].len(), journal[1].len() + 1);
    let files = super::bytes::tree(&fixture.root);
    for entry in old {
        assert!(files.contains(&entry));
    }
}
