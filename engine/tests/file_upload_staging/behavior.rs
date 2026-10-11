use super::{bytes::*, fixture::*};
use cyanrex_engine::{
    models::collaboration::FILE_TRANSFER_CHUNK_BYTES,
    services::file_upload_staging::{UploadChunkError, UploadChunkState},
};
use std::io::{Read, Seek, SeekFrom};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_real_cross_chunk_receipts_are_exact_and_attempts_are_single_use() {
    let length = FILE_TRANSFER_CHUNK_BYTES + 17;
    let fixture = Fixture::ready(length).await;
    let source_before = fixture.rows().await;
    let original_oids = fixture.oids().await;
    let mut first = fixture
        .adapter
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    let mut second = fixture
        .adapter
        .prepare(upload(1), owner(1), account(1), 1)
        .await
        .unwrap();
    assert_eq!(fixture.rows().await, source_before);
    assert!(tree(&fixture.root).is_empty());
    for (attempt, index) in [(&first, 0), (&second, 1)] {
        assert_eq!(attempt.upload_id(), upload(1));
        assert_eq!(attempt.owner(), owner(1));
        assert_eq!(attempt.account_id(), account(1));
        assert_eq!(attempt.manifest(), &fixture.manifest);
        assert_eq!(attempt.chunk_index(), index);
        assert_eq!(attempt.state(), UploadChunkState::Ready);
    }
    let file = fixture.real_file(length);
    let receipt = fixture
        .adapter
        .clone()
        .stage_chunk(&mut first, file.take(FILE_TRANSFER_CHUNK_BYTES))
        .await
        .unwrap();
    assert_eq!(first.state(), UploadChunkState::Confirmed);
    assert_eq!(receipt.upload_id(), upload(1));
    assert_eq!(receipt.owner(), owner(1));
    assert_eq!(receipt.account_id(), account(1));
    assert_eq!(receipt.manifest(), &fixture.manifest);
    assert_eq!(receipt.chunk_index(), 0);
    let mut file = fixture.real_file(length);
    file.seek(SeekFrom::Start(FILE_TRANSFER_CHUNK_BYTES))
        .unwrap();
    let receipt = fixture
        .adapter
        .stage_chunk(&mut second, file)
        .await
        .unwrap();
    assert_eq!(second.state(), UploadChunkState::Confirmed);
    assert_eq!(receipt.chunk_index(), 1);
    assert_eq!(receipt.manifest(), &fixture.manifest);
    let entries = tree(&fixture.root);
    assert_eq!(entries.len(), 2);
    for index in 0..2 {
        let spec = fixture.manifest.chunk(index).unwrap();
        let found = entries
            .iter()
            .find(|entry| entry.length == spec.byte_length)
            .unwrap();
        assert!(found.name.to_string_lossy().starts_with("u1-"));
        assert_eq!(
            found.digest.as_deref(),
            Some(spec.sha256.to_string().as_str())
        );
        assert_eq!(found.links, 1);
        assert_eq!(found.mode & 0o7777, 0o400);
    }
    let reader = Pattern::new(0, FILE_TRANSFER_CHUNK_BYTES);
    let calls = reader.calls.clone();
    assert!(matches!(
        fixture.adapter.stage_chunk(&mut first, reader).await,
        Err(UploadChunkError::InvalidInput)
    ));
    assert_eq!(first.state(), UploadChunkState::Confirmed);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(tree(&fixture.root), entries);
    assert_eq!(fixture.rows().await, source_before);
    assert_eq!(fixture.oids().await, original_oids);
    fixture.pool_restored().await;
    fixture.cleanup().await;
}
