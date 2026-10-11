use super::{bytes::*, control::*, fixture::*, support::*};
use cyanrex_engine::services::{
    file_upload_chunk_journal::PersistentChunkState,
    file_upload_staging::{JournaledUploadChunkError, UploadChunkError, UploadChunkState},
    file_upload_store::FileUploadStoreError,
};
use std::sync::atomic::Ordering;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_unknown_commit_acknowledgement_precedes_any_file_creation() {
    let fixture = Fixture::ready(17).await;
    let source = fixture.rows(false).await;
    let journal = fixture.rows(true).await;
    let source_oids = fixture.oids(false).await;
    let journal_oids = fixture.oids(true).await;
    let (holder, pid) = advisory(&fixture).await;
    fault(
        &fixture,
        "INSERT",
        &format!(
            "PERFORM pg_advisory_xact_lock(hashtext('{}'),998);",
            fixture.journal_schema
        ),
        true,
    )
    .await;
    let mut attempt = fixture
        .journaled
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    assert_eq!(attempt.state(), UploadChunkState::Ready);
    assert_eq!(fixture.rows(false).await, source);
    assert_eq!(fixture.rows(true).await, journal);
    let (reader, release, started, finished) = held(0, 17, 0);
    let (reader, calls) = counted(reader);
    let mut pending = Box::pin(fixture.journaled.stage_chunk(&mut attempt, reader));
    tokio::select! {result=&mut pending=>panic!("stage completed before the actual journal SQL wait: {result:?}"),_=fixture.wait_blocked(true,pid)=>{}}
    // This observes O_EXCL ordering too: a zero-byte file would already be visible here.
    assert!(tree(&fixture.root).is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(hit(&fixture).await);
    assert_eq!(
        fixture
            .journal
            .get(upload(1), owner(1), account(1), 0)
            .await
            .unwrap(),
        None
    );
    assert_eq!(fixture.rows(true).await, journal);
    assert_eq!(fixture.rows(false).await, source);
    holder.rollback().await.unwrap();
    tokio::select! {result=&mut pending=>panic!("stage completed before its real reader barrier: {result:?}"),_=signal(started)=>{}}
    let acknowledged = fixture.rows(true).await;
    assert_eq!(acknowledged[0], journal[0]);
    assert_eq!(acknowledged[1].len(), 1);
    let partial = tree(&fixture.root);
    assert_eq!(partial.len(), 1);
    assert_eq!(partial[0].length, 0);
    metadata(
        &fixture
            .journal
            .get(upload(1), owner(1), account(1), 0)
            .await
            .unwrap()
            .unwrap(),
        0,
        &fixture.manifest,
        PersistentChunkState::Unknown,
    );
    assert_eq!(fixture.rows(true).await, acknowledged);
    assert_eq!(tree(&fixture.root), partial);
    drop(release);
    let receipt = pending.await.unwrap();
    signal(finished).await;
    assert_eq!(receipt.manifest(), &fixture.manifest);
    assert_eq!(attempt.state(), UploadChunkState::Confirmed);
    let before = fixture.rows(true).await;
    let files = tree(&fixture.root);
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
    assert_eq!(fixture.rows(true).await, before);
    assert_eq!(tree(&fixture.root), files);
    assert_eq!(fixture.rows(false).await, source);
    assert_eq!(fixture.oids(false).await, source_oids);
    assert_eq!(fixture.oids(true).await, journal_oids);
    remove_fault(&fixture).await;
    fixture.pool_restored(false).await;
    fixture.pool_restored(true).await;
    fixture.cleanup().await;

    // A legitimate source mutation in the deferred journal event commits with Unknown. The fresh
    // source check after that acknowledgement must reject before even O_EXCL, not after file work.
    let fixture = Fixture::ready(17).await;
    let source = fixture.rows(false).await;
    let journal = fixture.rows(true).await;
    let source_oids = fixture.oids(false).await;
    let journal_oids = fixture.oids(true).await;
    fault(
        &fixture,
        "INSERT",
        &format!(
            "UPDATE {}.{SOURCE_ROWS} SET owner_id='{}'::uuid;",
            fixture.source_schema,
            owner(2).principal_id.as_uuid()
        ),
        true,
    )
    .await;
    let mut attempt = fixture
        .journaled
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    let reader = Pattern::new(0, 17);
    let calls = reader.calls.clone();
    assert!(matches!(
        fixture.journaled.stage_chunk(&mut attempt, reader).await,
        Err(JournaledUploadChunkError::Local(UploadChunkError::Store(
            FileUploadStoreError::InvalidRecord
        )))
    ));
    assert!(hit(&fixture).await);
    assert_eq!(attempt.state(), UploadChunkState::Unknown);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(tree(&fixture.root).is_empty());
    let changed_source = fixture.rows(false).await;
    assert_eq!(changed_source[0], source[0]);
    assert_eq!(changed_source[1].len(), 1);
    let mut expected: serde_json::Value = serde_json::from_str(&source[1][0]).unwrap();
    expected["owner_id"] = serde_json::json!(owner(2).principal_id.as_uuid());
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&changed_source[1][0]).unwrap(),
        expected
    );
    let acknowledged = fixture.rows(true).await;
    assert_eq!(acknowledged[0], journal[0]);
    assert_eq!(acknowledged[1].len(), 1);
    metadata(
        &fixture
            .journal
            .get(upload(1), owner(1), account(1), 0)
            .await
            .unwrap()
            .unwrap(),
        0,
        &fixture.manifest,
        PersistentChunkState::Unknown,
    );
    assert_eq!(fixture.rows(true).await, acknowledged);
    assert_eq!(fixture.rows(false).await, changed_source);
    assert!(tree(&fixture.root).is_empty());
    assert_eq!(fixture.oids(false).await, source_oids);
    assert_eq!(fixture.oids(true).await, journal_oids);
    remove_fault(&fixture).await;
    healthy(&fixture, 2).await;
    fixture.pool_restored(false).await;
    fixture.pool_restored(true).await;
    fixture.cleanup().await;
}
