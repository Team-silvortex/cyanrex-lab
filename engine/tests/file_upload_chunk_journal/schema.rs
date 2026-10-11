use super::{bytes::*, fixture::*, support::*};
use cyanrex_engine::services::{
    file_upload_chunk_journal::{FileUploadChunkJournal, FileUploadChunkJournalError},
    file_upload_staging::{JournaledFileUploadStaging, JournaledUploadChunkError},
};
use sqlx_core::{query::query, row::Row};
use uuid::Uuid;

pub async fn checks(fixture: &Fixture, table: &str, kind: &str) -> Vec<(String, String)> {
    assert!([JOURNAL_MARKER, JOURNAL_ROWS].contains(&table));
    assert!(["c", "p", "u"].contains(&kind));
    let rows=query("SELECT conname,pg_get_constraintdef(oid) AS definition FROM pg_constraint WHERE conrelid=to_regclass($1) AND contype::text=$2 ORDER BY conname")
        .bind(format!("{}.{table}",fixture.journal_schema)).bind(kind).fetch_all(&fixture.journal_pool).await.unwrap();
    let mut saved = Vec::new();
    for row in rows {
        let name: String = row.get("conname");
        let definition: String = row.get("definition");
        assert!(name
            .bytes()
            .all(|value| value.is_ascii_alphanumeric() || value == b'_'));
        fixture
            .sql(true, &format!("ALTER TABLE {table} DROP CONSTRAINT {name}"))
            .await;
        saved.push((name, definition));
    }
    assert!(!saved.is_empty());
    saved
}
pub async fn restore(fixture: &Fixture, table: &str, saved: Vec<(String, String)>) {
    for (name, definition) in saved {
        fixture
            .sql(
                true,
                &format!("ALTER TABLE {table} ADD CONSTRAINT {name} {definition}"),
            )
            .await;
    }
}
async fn rejected(
    fixture: &Fixture,
    journal: &FileUploadChunkJournal,
    error: FileUploadChunkJournalError,
) {
    let source = fixture.rows(false).await;
    let before = fixture.rows(true).await;
    let files = tree(&fixture.root);
    let oids = fixture.oids(true).await;
    assert_eq!(
        journal.get(upload(1), owner(1), account(1), 0).await,
        Err(error)
    );
    assert_eq!(fixture.rows(false).await, source);
    assert_eq!(fixture.rows(true).await, before);
    assert_eq!(tree(&fixture.root), files);
    assert_eq!(fixture.oids(true).await, oids);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_journal_installation_schema_pins_and_bad_records_fail_closed() {
    let occupied = Fixture::new(4, 2, 16).await;
    let source = occupied.rows(false).await;
    occupied
        .sql(true, "CREATE TABLE unrelated(value bytea NOT NULL)")
        .await;
    occupied
        .sql(true, "INSERT INTO unrelated VALUES(decode('aabb','hex'))")
        .await;
    let before: String = query("SELECT row_to_json(t)::text AS value FROM unrelated t")
        .fetch_one(&occupied.journal_pool)
        .await
        .unwrap()
        .get("value");
    assert_eq!(
        occupied.journal.install_empty_namespace().await,
        Err(FileUploadChunkJournalError::NamespaceNotEmpty)
    );
    let after: String = query("SELECT row_to_json(t)::text AS value FROM unrelated t")
        .fetch_one(&occupied.journal_pool)
        .await
        .unwrap()
        .get("value");
    assert_eq!(after, before);
    let count: i64 =
        query("SELECT count(*) AS n FROM pg_class WHERE relnamespace=to_regnamespace($1)")
            .bind(&occupied.journal_schema)
            .fetch_one(&occupied.journal_admin)
            .await
            .unwrap()
            .get("n");
    assert_eq!(count, 1);
    assert_eq!(occupied.rows(false).await, source);
    assert!(tree(&occupied.root).is_empty());
    occupied.cleanup().await;

    let fixture = Fixture::ready(17).await;
    let before = fixture.rows(true).await;
    assert_eq!(
        fixture.journal.install_empty_namespace().await,
        Err(FileUploadChunkJournalError::NamespaceNotEmpty)
    );
    assert_eq!(fixture.rows(true).await, before);
    for (installation, entries, error) in [
        (
            Uuid::new_v4(),
            16,
            FileUploadChunkJournalError::IncarnationMismatch,
        ),
        (
            fixture.journal_installation_id,
            15,
            FileUploadChunkJournalError::UnsupportedSchema,
        ),
    ] {
        let journal = FileUploadChunkJournal::new(
            fixture.journal_pool.clone(),
            scope(),
            installation,
            &fixture.journal_schema,
            entries,
        )
        .unwrap();
        rejected(&fixture, &journal, error).await;
    }
    for authority in [true, false] {
        let mut foreign = scope();
        if authority {
            foreign.authority_id = Uuid::from_u128(901).try_into().unwrap();
        } else {
            foreign.workspace_id = Uuid::from_u128(902).try_into().unwrap();
        }
        let journal = FileUploadChunkJournal::new(
            fixture.journal_pool.clone(),
            foreign,
            fixture.journal_installation_id,
            &fixture.journal_schema,
            16,
        )
        .unwrap();
        let mut actor = owner(1);
        actor.authority_id = foreign.authority_id;
        let before = fixture.rows(true).await;
        assert_eq!(
            journal.get(upload(1), actor, account(1), 0).await,
            Err(FileUploadChunkJournalError::ScopeMismatch)
        );
        assert_eq!(fixture.rows(true).await, before);
    }
    for column in ["namespace_oid", "schema_oid", "chunks_oid"] {
        let original: i64 = query(&format!("SELECT {column} AS value FROM {JOURNAL_MARKER}"))
            .fetch_one(&fixture.journal_pool)
            .await
            .unwrap()
            .get("value");
        fixture
            .sql(
                true,
                &format!("UPDATE {JOURNAL_MARKER} SET {column}={}", original + 1),
            )
            .await;
        rejected(
            &fixture,
            &fixture.journal,
            FileUploadChunkJournalError::UnsupportedSchema,
        )
        .await;
        fixture
            .sql(
                true,
                &format!("UPDATE {JOURNAL_MARKER} SET {column}={original}"),
            )
            .await;
        assert_eq!(
            fixture
                .journal
                .get(upload(1), owner(1), account(1), 0)
                .await
                .unwrap(),
            None
        );
    }
    let primary = checks(&fixture, JOURNAL_ROWS, "p").await;
    rejected(
        &fixture,
        &fixture.journal,
        FileUploadChunkJournalError::UnsupportedSchema,
    )
    .await;
    restore(&fixture, JOURNAL_ROWS, primary).await;
    let nonce = checks(&fixture, JOURNAL_ROWS, "u").await;
    rejected(
        &fixture,
        &fixture.journal,
        FileUploadChunkJournalError::UnsupportedSchema,
    )
    .await;
    restore(&fixture, JOURNAL_ROWS, nonce).await;
    for (damage,repair) in [
        (format!("ALTER TABLE {JOURNAL_ROWS} ALTER COLUMN account_id DROP NOT NULL"),format!("ALTER TABLE {JOURNAL_ROWS} ALTER COLUMN account_id SET NOT NULL")),
        (format!("ALTER TABLE {JOURNAL_ROWS} ALTER COLUMN account_id TYPE text USING account_id::text"),format!("ALTER TABLE {JOURNAL_ROWS} ALTER COLUMN account_id TYPE uuid USING account_id::uuid")),
        (format!("ALTER TABLE {JOURNAL_ROWS} ENABLE ROW LEVEL SECURITY"),format!("ALTER TABLE {JOURNAL_ROWS} DISABLE ROW LEVEL SECURITY")),
    ] {
        fixture.sql(true,&damage).await;rejected(&fixture,&fixture.journal,FileUploadChunkJournalError::UnsupportedSchema).await;fixture.sql(true,&repair).await;
        assert_eq!(fixture.journal.get(upload(1),owner(1),account(1),0).await.unwrap(),None);
    }
    // Prepared attempts retain original relation OIDs even if the replacement has identical columns/data.
    let mut prepared = fixture
        .journaled
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    for statement in [
        format!("ALTER TABLE {JOURNAL_ROWS} RENAME TO original_chunks"),
        format!("CREATE TABLE {JOURNAL_ROWS}(LIKE original_chunks INCLUDING ALL)"),
        format!("INSERT INTO {JOURNAL_ROWS} SELECT * FROM original_chunks"),
        format!("UPDATE {JOURNAL_MARKER} SET chunks_oid='{JOURNAL_ROWS}'::regclass::oid::bigint"),
    ] {
        fixture.sql(true, &statement).await;
    }
    let rows = fixture.rows(true).await;
    let source = fixture.rows(false).await;
    let oids = fixture.oids(true).await;
    let reader = Pattern::new(0, 17);
    let calls = reader.calls.clone();
    assert!(matches!(
        fixture.journaled.stage_chunk(&mut prepared, reader).await,
        Err(JournaledUploadChunkError::Journal(
            FileUploadChunkJournalError::UnsupportedSchema
        ))
    ));
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(fixture.rows(true).await, rows);
    assert_eq!(fixture.rows(false).await, source);
    assert_eq!(fixture.oids(true).await, oids);
    assert!(tree(&fixture.root).is_empty());
    // A fresh explicitly trusted call may capture the current valid installation; it is not restoration.
    let fresh =
        JournaledFileUploadStaging::new(fixture.local.clone(), fixture.journal.clone()).unwrap();
    let mut attempt = fresh
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    fresh
        .stage_chunk(&mut attempt, Pattern::new(0, 17))
        .await
        .unwrap();
    assert_eq!(fixture.rows(false).await, source);
    fixture.pool_restored(true).await;
    fixture.cleanup().await;

    super::records::corrupt_foreign_records().await;
    query_shadow().await;
}

async fn query_shadow() {
    let fixture = Fixture::ready(17).await;
    let source = fixture.rows(false).await;
    let rows = fixture.rows(true).await;
    let oids = fixture.oids(true).await;
    for shadow in ["temp", "path", "namespace"] {
        let url = std::env::var("CYANREX_TEST_DATABASE_URL").unwrap();
        let pool = pool_for(&url, &fixture.journal_schema, 1).await;
        let statement = match shadow {
            "temp" => "CREATE TEMP TABLE unrelated(value integer)".to_owned(),
            "path" => format!("SET search_path TO {}, public", fixture.journal_schema),
            "namespace" => format!("SET search_path TO {}", fixture.source_schema),
            _ => unreachable!(),
        };
        query(&statement).execute(&pool).await.unwrap();
        let journal = FileUploadChunkJournal::new(
            pool.clone(),
            scope(),
            fixture.journal_installation_id,
            &fixture.journal_schema,
            16,
        )
        .unwrap();
        let before: String = query("SHOW search_path")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(
            journal.get(upload(1), owner(1), account(1), 0).await,
            Err(FileUploadChunkJournalError::UnsupportedSchema)
        );
        let after: String = query("SHOW search_path")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(before, after);
        assert_eq!(fixture.rows(false).await, source);
        assert_eq!(fixture.rows(true).await, rows);
        assert_eq!(fixture.oids(true).await, oids);
        assert!(tree(&fixture.root).is_empty());
        pool.close().await;
    }
    healthy(&fixture, 2).await;
    fixture.cleanup().await;
}
