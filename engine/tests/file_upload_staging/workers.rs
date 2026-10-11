use super::{bytes::*, control::*, fixture::*};
use cyanrex_engine::services::{
    file_transfer::staging::ChunkStagingError,
    file_upload_staging::{FileUploadStaging, UploadChunkError, UploadChunkState},
};
use std::{
    io::{self, Read},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_control_admission_and_cancelled_real_worker_preserve_attempt_states() {
    let fixture = Fixture::new(1, 1).await;
    let first_manifest = manifest(17, 1);
    let other_manifest = manifest(17, 2);
    fixture
        .source
        .reserve(upload(1), owner(1), account(1), &first_manifest)
        .await
        .unwrap();
    fixture
        .source
        .reserve(upload(2), owner(1), account(2), &other_manifest)
        .await
        .unwrap();
    let independent =
        FileUploadStaging::new(fixture.source.clone(), fixture.staging.clone(), 1).unwrap();
    let mut first = fixture
        .adapter
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    let mut waiting = fixture
        .adapter
        .prepare(upload(2), owner(1), account(2), 0)
        .await
        .unwrap();
    let mut other = independent
        .prepare(upload(2), owner(1), account(2), 0)
        .await
        .unwrap();
    let before = fixture.rows().await;
    let (reader, release, started, finished) = held(0, 17, 0);
    let mut pending = Box::pin(fixture.adapter.stage_chunk(&mut first, reader));
    tokio::select! {result=&mut pending=>panic!("stage completed before held reader: {result:?}"),_ =signal(started)=>{} }
    let clone = fixture.adapter.clone();
    let reader = Pattern::new(0, 17);
    let calls = reader.calls.clone();
    assert!(matches!(
        clone.stage_chunk(&mut waiting, reader).await,
        Err(UploadChunkError::CapacityExhausted)
    ));
    assert_eq!(waiting.state(), UploadChunkState::Ready);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let partial = tree(&fixture.root);
    assert_eq!(partial.len(), 1);
    assert_eq!(partial[0].length, 0);
    drop(pending);
    assert_eq!(first.state(), UploadChunkState::Unknown);
    let reader = Pattern::new(0, 17);
    let calls = reader.calls.clone();
    assert!(matches!(
        independent.stage_chunk(&mut other, reader).await,
        Err(UploadChunkError::CapacityExhausted)
    ));
    assert_eq!(
        other.state(),
        UploadChunkState::Unknown,
        "independent controls admitted, but the real staging worker remains full"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(tree(&fixture.root), partial);
    assert_eq!(fixture.rows().await, before);
    drop(release);
    signal(finished).await;
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match fixture
                .staging
                .verify_staged_chunk(&manifest(17, 900), 0)
                .await
            {
                Err(ChunkStagingError::CapacityExhausted) => tokio::task::yield_now().await,
                Err(ChunkStagingError::Unavailable) => break,
                result => {
                    panic!("fresh nonexistent legacy coordinate unexpectedly verified: {result:?}")
                }
            }
        }
    })
    .await
    .expect("actual blocking worker did not release its slot");
    let retained = tree(&fixture.root);
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].length, 17);
    assert_eq!(retained[0].inode, partial[0].inode);
    assert_eq!(first.state(), UploadChunkState::Unknown);
    fixture
        .adapter
        .stage_chunk(&mut waiting, Pattern::new(0, 17))
        .await
        .unwrap();
    assert_eq!(waiting.state(), UploadChunkState::Confirmed);
    assert!(matches!(
        fixture
            .adapter
            .stage_chunk(&mut first, Pattern::new(0, 17))
            .await,
        Err(UploadChunkError::InvalidInput)
    ));
    let mut new_trusted_call = independent
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    assert!(matches!(
        independent
            .stage_chunk(&mut new_trusted_call, Pattern::new(0, 17))
            .await,
        Err(UploadChunkError::Conflict)
    ));
    assert_eq!(new_trusted_call.state(), UploadChunkState::Unknown);
    assert_eq!(fixture.rows().await, before);
    let now = tree(&fixture.root);
    assert_eq!(now.len(), 2);
    assert!(now.iter().any(|entry| entry == &retained[0]));
    fixture.pool_restored().await;
    fixture.cleanup().await;
    // This new prepare is a fresh trusted call, not restoration of the cancelled in-memory attempt.
}

struct FailedReader {
    kind: io::ErrorKind,
    calls: Arc<AtomicUsize>,
}
impl Read for FailedReader {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(self.kind.into())
    }
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_input_failures_retain_unknown_files_and_never_adopt_matching_legacy_bytes() {
    for kind in [io::ErrorKind::Other, io::ErrorKind::Interrupted] {
        let fixture = Fixture::ready(17).await;
        let before = fixture.rows().await;
        fixture
            .staging
            .stage_chunk(&fixture.manifest, 0, Pattern::new(0, 17))
            .await
            .unwrap();
        let legacy = tree(&fixture.root);
        assert_eq!(legacy.len(), 1);
        assert!(legacy[0].name.to_string_lossy().starts_with("v1-"));
        let mut attempt = fixture
            .adapter
            .prepare(upload(1), owner(1), account(1), 0)
            .await
            .unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        assert!(matches!(
            fixture
                .adapter
                .stage_chunk(
                    &mut attempt,
                    FailedReader {
                        kind,
                        calls: calls.clone()
                    }
                )
                .await,
            Err(UploadChunkError::Unconfirmed)
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(attempt.state(), UploadChunkState::Unknown);
        let retained = tree(&fixture.root);
        assert_eq!(retained.len(), 2);
        assert!(retained.iter().any(|entry| entry == &legacy[0]));
        assert!(retained
            .iter()
            .any(|entry| entry.name.to_string_lossy().starts_with("u1-") && entry.length == 0));
        fixture
            .staging
            .verify_staged_chunk(&fixture.manifest, 0)
            .await
            .unwrap();
        assert_eq!(tree(&fixture.root), retained);
        assert_eq!(fixture.rows().await, before);
        let mut repeat = fixture
            .adapter
            .prepare(upload(1), owner(1), account(1), 0)
            .await
            .unwrap();
        let reader = Pattern::new(0, 17);
        let calls = reader.calls.clone();
        assert!(matches!(
            fixture.adapter.stage_chunk(&mut repeat, reader).await,
            Err(UploadChunkError::Conflict)
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(tree(&fixture.root), retained);
        let healthy = manifest(17, 2);
        fixture
            .source
            .reserve(upload(2), owner(1), account(2), &healthy)
            .await
            .unwrap();
        let before = fixture.rows().await;
        let mut fresh = fixture
            .adapter
            .prepare(upload(2), owner(1), account(2), 0)
            .await
            .unwrap();
        fixture
            .adapter
            .stage_chunk(&mut fresh, Pattern::new(0, 17))
            .await
            .unwrap();
        assert_eq!(fresh.state(), UploadChunkState::Confirmed);
        assert_eq!(fixture.rows().await, before);
        let now = tree(&fixture.root);
        assert_eq!(now.len(), 3);
        for old in retained {
            assert!(now.iter().any(|entry| entry == &old));
        }
        fixture.pool_restored().await;
        fixture.cleanup().await;
    }
    let healthy = Fixture::ready(17).await;
    let before = healthy.rows().await;
    healthy
        .staging
        .stage_chunk(&healthy.manifest, 0, Pattern::new(0, 17))
        .await
        .unwrap();
    let mut fresh = healthy
        .adapter
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    healthy
        .adapter
        .stage_chunk(&mut fresh, Pattern::new(0, 17))
        .await
        .unwrap();
    assert_eq!(tree(&healthy.root).len(), 2);
    assert_eq!(fresh.state(), UploadChunkState::Confirmed);
    assert_eq!(healthy.rows().await, before);
    healthy.cleanup().await;
}
