use super::{bytes::*, fixture::*, support::*};
use cyanrex_engine::services::{
    file_upload_chunk_journal::{FileUploadChunkJournalError, PersistentChunkState},
    file_upload_staging::{JournaledUploadChunkError, UploadChunkError, UploadChunkState},
    file_upload_store::FileUploadStoreError,
};
use std::{fs, sync::atomic::Ordering};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_status_observation_never_accesses_source_or_files_or_restores_dispatch() {
    let fixture = Fixture::ready(17).await;
    let mut confirmed = fixture
        .journaled
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    let _receipt = fixture
        .journaled
        .stage_chunk(&mut confirmed, Pattern::new(0, 17))
        .await
        .unwrap();
    let other = manifest(17, 2);
    fixture
        .source
        .reserve(upload(2), owner(2), account(2), &other)
        .await
        .unwrap();
    fixture
        .staging
        .stage_chunk(&other, 0, Pattern::new(0, 17))
        .await
        .unwrap();
    let legacy = tree(&fixture.root);
    assert!(legacy
        .iter()
        .any(|entry| entry.name.to_string_lossy().starts_with("v1-")));
    let mut unknown = fixture
        .journaled
        .prepare(upload(2), owner(2), account(2), 0)
        .await
        .unwrap();
    assert!(matches!(
        fixture
            .journaled
            .stage_chunk(&mut unknown, FailedReader)
            .await,
        Err(JournaledUploadChunkError::Unconfirmed)
    ));
    assert_eq!(unknown.state(), UploadChunkState::Unknown);
    fixture
        .staging
        .verify_staged_chunk(&other, 0)
        .await
        .unwrap();
    let journal = fixture.rows(true).await;
    let source = admin_rows(&fixture, false).await;
    let oids = fixture.oids(true).await;
    let files = tree(&fixture.root);
    assert_eq!(files.len(), 3);
    for entry in legacy {
        assert!(files.contains(&entry));
    }
    // Neither current pathname availability nor a usable source connection is required for status.
    let moved = fixture.directory.join("inaccessible-old-root");
    fs::rename(&fixture.root, &moved).unwrap();
    fixture.source_pool.close().await;
    let (reopened, pool) = reopen(&fixture).await;
    metadata(
        &reopened
            .get(upload(1), owner(1), account(1), 0)
            .await
            .unwrap()
            .unwrap(),
        0,
        &fixture.manifest,
        PersistentChunkState::Confirmed,
    );
    let observed = reopened
        .get(upload(2), owner(2), account(2), 0)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(observed.upload_id(), upload(2));
    assert_eq!(observed.owner(), owner(2));
    assert_eq!(observed.account_id(), account(2));
    assert_eq!(observed.sha256(), &other.chunk(0).unwrap().sha256);
    assert_eq!(observed.state(), PersistentChunkState::Unknown);
    for (id, actor, generation, index) in [
        (upload(99), owner(1), account(1), 0),
        (upload(1), owner(2), account(1), 0),
        (upload(1), owner(1), account(2), 0),
        (upload(1), owner(1), account(1), 1),
    ] {
        assert_eq!(
            reopened.get(id, actor, generation, index).await.unwrap(),
            None
        );
        assert_eq!(fixture.rows(true).await, journal);
        assert_eq!(admin_rows(&fixture, false).await, source);
        assert_eq!(tree(&moved), files);
    }
    let mut foreign = owner(1);
    foreign.authority_id = uuid::Uuid::from_u128(900).try_into().unwrap();
    assert_eq!(
        reopened.get(upload(1), foreign, account(1), 0).await,
        Err(FileUploadChunkJournalError::ScopeMismatch)
    );
    assert_eq!(
        reopened.get(upload(1), owner(1), account(1), 16_384).await,
        Err(FileUploadChunkJournalError::InvalidInput)
    );
    assert!(matches!(
        fixture
            .journaled
            .prepare(upload(1), owner(1), account(1), 0)
            .await,
        Err(JournaledUploadChunkError::Local(UploadChunkError::Store(
            FileUploadStoreError::StorageUnavailable
        )))
    ));
    let reader = Pattern::new(0, 17);
    let calls = reader.calls.clone();
    assert!(matches!(
        fixture.journaled.stage_chunk(&mut unknown, reader).await,
        Err(JournaledUploadChunkError::Local(
            UploadChunkError::InvalidInput
        ))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(unknown.state(), UploadChunkState::Unknown);
    assert_eq!(fixture.rows(true).await, journal);
    assert_eq!(admin_rows(&fixture, false).await, source);
    assert_eq!(fixture.oids(true).await, oids);
    assert_eq!(tree(&moved), files);
    pool.close().await;
    fixture.pool_restored(true).await;
    fixture.cleanup().await;
}
