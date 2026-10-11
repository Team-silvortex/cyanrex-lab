use super::{
    bytes::*,
    fixture::*,
    schema::{checks, restore},
    support::*,
};
use cyanrex_engine::services::{
    file_upload_chunk_journal::{FileUploadChunkJournalError, PersistentChunkState},
    file_upload_staging::{JournaledUploadChunkError, UploadChunkState},
};
use sqlx_core::{query::query, row::Row};
use std::sync::atomic::Ordering;
use uuid::Uuid;

pub async fn corrupt_foreign_records() {
    let fixture = Fixture::ready(17).await;
    let mut first = fixture
        .journaled
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    fixture
        .journaled
        .stage_chunk(&mut first, Pattern::new(0, 17))
        .await
        .unwrap();
    fixture
        .source
        .reserve(upload(2), owner(2), account(2), &manifest(17, 2))
        .await
        .unwrap();
    let canonical = query(&format!(
        "SELECT source_key,root_key,sha256,nonce FROM {JOURNAL_ROWS}"
    ))
    .fetch_one(&fixture.journal_pool)
    .await
    .unwrap();
    let source_key: Vec<u8> = canonical.get("source_key");
    let root_key: Vec<u8> = canonical.get("root_key");
    let sha256: String = canonical.get("sha256");
    let nonce: Uuid = canonical.get("nonce");
    let saved = checks(&fixture, JOURNAL_ROWS, "c").await;
    for (damage, read_invalid) in [
        ("upload_id='00000000-0000-0000-0000-000000000000'", false),
        ("owner_id='00000000-0000-0000-0000-000000000000'", false),
        ("account_id='00000000-0000-0000-0000-000000000000'", false),
        ("chunk_index=-1", false),
        ("chunk_index=16384", false),
        ("source_key=decode('00','hex')", true),
        ("root_key=decode(repeat('00',65537),'hex')", true),
        ("sha256=repeat('A',64)", true),
        ("sha256=repeat('a',65)", true),
        ("nonce='00000000-0000-0000-0000-000000000000'", true),
        ("nonce='00000000-0000-1000-8000-000000000001'", true),
        ("total_length=0", true),
        ("total_length=68719476737", true),
        ("chunk_length=-1", true),
        ("chunk_length=18", true),
    ] {
        fixture
            .sql(true, &format!("UPDATE {JOURNAL_ROWS} SET {damage}"))
            .await;
        let rows = fixture.rows(true).await;
        let source = fixture.rows(false).await;
        let files = tree(&fixture.root);
        let oids = fixture.oids(true).await;
        if read_invalid {
            assert_eq!(
                fixture
                    .journal
                    .get(upload(1), owner(1), account(1), 0)
                    .await,
                Err(FileUploadChunkJournalError::InvalidRecord)
            );
        }
        let mut target = fixture
            .journaled
            .prepare(upload(2), owner(2), account(2), 0)
            .await
            .unwrap();
        let reader = Pattern::new(0, 17);
        let calls = reader.calls.clone();
        assert!(
            matches!(
                fixture.journaled.stage_chunk(&mut target, reader).await,
                Err(JournaledUploadChunkError::Journal(
                    FileUploadChunkJournalError::InvalidRecord
                ))
            ),
            "foreign corruption must fail before dispatch: {damage}"
        );
        assert_eq!(target.state(), UploadChunkState::Unknown);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.rows(true).await, rows);
        assert_eq!(fixture.rows(false).await, source);
        assert_eq!(tree(&fixture.root), files);
        assert_eq!(fixture.oids(true).await, oids);
        query(&format!("UPDATE {JOURNAL_ROWS} SET upload_id=$1,owner_id=$2,account_id=$3,chunk_index=0,source_key=$4,root_key=$5,total_length=17,chunk_length=17,sha256=$6,nonce=$7,confirmed=TRUE"))
            .bind(upload(1).as_uuid()).bind(owner(1).principal_id.as_uuid()).bind(account(1).as_uuid()).bind(&source_key).bind(&root_key).bind(&sha256).bind(nonce).execute(&fixture.journal_pool).await.unwrap();
        let restored = fixture.rows(true).await;
        metadata(
            &fixture
                .journal
                .get(upload(1), owner(1), account(1), 0)
                .await
                .unwrap()
                .unwrap(),
            0,
            &fixture.manifest,
            PersistentChunkState::Confirmed,
        );
        assert_eq!(fixture.rows(true).await, restored);
        assert_eq!(fixture.rows(false).await, source);
        assert_eq!(tree(&fixture.root), files);
    }
    restore(&fixture, JOURNAL_ROWS, saved).await;
    healthy(&fixture, 3).await;
    fixture.pool_restored(false).await;
    fixture.pool_restored(true).await;
    fixture.cleanup().await;
}
