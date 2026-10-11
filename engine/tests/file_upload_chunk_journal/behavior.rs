use super::{bytes::*, fixture::*, support::*};
use cyanrex_engine::{
    models::collaboration::FILE_TRANSFER_CHUNK_BYTES,
    services::{
        file_upload_chunk_journal::{FileUploadChunkJournalError, PersistentChunkState},
        file_upload_staging::{JournaledUploadChunkError, UploadChunkState},
    },
};
use std::{
    io::{Read, Seek, SeekFrom},
    sync::atomic::Ordering,
};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_real_cross_chunk_confirmations_persist_reopen_and_never_allow_restage() {
    let length = FILE_TRANSFER_CHUNK_BYTES + 17;
    let fixture = Fixture::ready(length).await;
    let source = fixture.rows(false).await;
    let source_oids = fixture.oids(false).await;
    let journal = fixture.rows(true).await;
    for index in 0..2 {
        let mut attempt = fixture
            .journaled
            .prepare(upload(1), owner(1), account(1), index)
            .await
            .unwrap();
        assert_eq!(attempt.state(), UploadChunkState::Ready);
        assert_eq!(fixture.rows(false).await, source);
        assert_eq!(fixture.rows(true).await[0], journal[0]);
        let mut file = fixture.real_file(length);
        file.seek(SeekFrom::Start(
            u64::from(index) * FILE_TRANSFER_CHUNK_BYTES,
        ))
        .unwrap();
        let receipt = fixture
            .journaled
            .clone()
            .stage_chunk(
                &mut attempt,
                file.take(fixture.manifest.chunk(index).unwrap().byte_length),
            )
            .await
            .unwrap();
        assert_eq!(attempt.state(), UploadChunkState::Confirmed);
        assert_eq!(receipt.upload_id(), upload(1));
        assert_eq!(receipt.owner(), owner(1));
        assert_eq!(receipt.account_id(), account(1));
        assert_eq!(receipt.chunk_index(), index);
        assert_eq!(receipt.manifest(), &fixture.manifest);
        let before = fixture.rows(true).await;
        let files = tree(&fixture.root);
        metadata(
            &fixture
                .journal
                .get(upload(1), owner(1), account(1), index)
                .await
                .unwrap()
                .unwrap(),
            index,
            &fixture.manifest,
            PersistentChunkState::Confirmed,
        );
        assert_eq!(fixture.rows(true).await, before);
        assert_eq!(tree(&fixture.root), files);
    }
    let persisted = fixture.rows(true).await;
    assert_eq!(persisted[0], journal[0]);
    assert_eq!(persisted[1].len(), 2);
    let files = tree(&fixture.root);
    assert_eq!(files.len(), 2);
    let (reopened, pool) = reopen(&fixture).await;
    for index in 0..2 {
        metadata(
            &reopened
                .get(upload(1), owner(1), account(1), index)
                .await
                .unwrap()
                .unwrap(),
            index,
            &fixture.manifest,
            PersistentChunkState::Confirmed,
        );
    }
    let mut repeated = fixture
        .journaled
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    let reader = Pattern::new(0, FILE_TRANSFER_CHUNK_BYTES);
    let calls = reader.calls.clone();
    assert!(matches!(
        fixture.journaled.stage_chunk(&mut repeated, reader).await,
        Err(JournaledUploadChunkError::Journal(
            FileUploadChunkJournalError::Conflict
        ))
    ));
    assert_eq!(repeated.state(), UploadChunkState::Unknown);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.rows(true).await, persisted);
    assert_eq!(fixture.rows(false).await, source);
    assert_eq!(fixture.oids(false).await, source_oids);
    assert_eq!(tree(&fixture.root), files);
    pool.close().await;
    fixture.pool_restored(false).await;
    fixture.pool_restored(true).await;
    fixture.cleanup().await;
}
