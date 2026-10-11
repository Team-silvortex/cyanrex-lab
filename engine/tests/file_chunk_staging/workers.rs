use super::fixture::*;
use cyanrex_engine::services::file_transfer::staging::{ChunkStaging, ChunkStagingError};
use std::{
    io::{self, Read},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

#[tokio::test]
async fn dropping_unpolled_stage_never_reads_admits_or_creates_output() {
    let fixture = Fixture::new();
    let expected = manifest(17, 60);
    let reader = Pattern::new(0, 17);
    let calls = reader.calls.clone();
    let future = fixture.store.stage_chunk(&expected, 0, reader);
    drop(future);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(tree(&fixture.root).is_empty());
    fixture
        .store
        .stage_chunk(&expected, 0, Pattern::new(0, 17))
        .await
        .unwrap();
    fixture
        .store
        .verify_staged_chunk(&expected, 0)
        .await
        .unwrap();
}

#[tokio::test]
async fn cancelled_caller_keeps_real_running_worker_and_clone_capacity_until_completion() {
    let fixture = Fixture::new();
    let expected = manifest(17, 61);
    let (reader, release, started, finished) = held(17, 0);
    let runner = fixture.store.clone();
    let pending_manifest = expected.clone();
    let waiting =
        tokio::spawn(async move { runner.stage_chunk(&pending_manifest, 0, reader).await });
    signal(started).await;
    waiting.abort();
    assert!(waiting.await.unwrap_err().is_cancelled());
    let pending = snapshot(&fixture.path(&expected, 0));
    let fresh = manifest(17, 62);
    let other_reader = Pattern::new(0, 17);
    let calls = other_reader.calls.clone();
    assert_eq!(
        fixture
            .store
            .clone()
            .stage_chunk(&fresh, 0, other_reader)
            .await,
        Err(ChunkStagingError::CapacityExhausted)
    );
    assert_eq!(
        fixture.store.verify_staged_chunk(&expected, 0).await,
        Err(ChunkStagingError::CapacityExhausted)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(snapshot(&fixture.path(&expected, 0)), pending);
    assert!(!fixture.path(&fresh, 0).exists());
    drop(release);
    signal(finished).await;
    fresh_stage_after_release(&fixture.store, &fresh).await;
    let retained = snapshot(&fixture.path(&expected, 0));
    assert_eq!(retained.inode, pending.inode);
    assert_eq!(retained.length, 17);
    assert_eq!(retained.mode & 0o7777, 0o400);
    fixture
        .store
        .verify_staged_chunk(&expected, 0)
        .await
        .unwrap();
    assert_eq!(snapshot(&fixture.path(&expected, 0)), retained);
    assert_eq!(
        fixture
            .store
            .stage_chunk(&expected, 0, Pattern::new(0, 17))
            .await,
        Err(ChunkStagingError::Conflict)
    );
}

#[tokio::test]
async fn timeout_of_actual_running_worker_retains_capacity_and_output_until_completion() {
    let fixture = Fixture::new();
    let expected = manifest(17, 63);
    let (reader, release, started, finished) = held(17, 0);
    let runner = fixture.store.clone();
    let pending_manifest = expected.clone();
    let waiting = tokio::spawn(async move {
        let operation = runner.stage_chunk(&pending_manifest, 0, reader);
        tokio::pin!(operation);
        tokio::select! {
            result = &mut operation => panic!("held worker completed before release: {result:?}"),
            signal = started => signal.unwrap(),
        }
        tokio::time::timeout(Duration::from_millis(30), operation).await
    });
    assert!(tokio::time::timeout(Duration::from_secs(2), waiting)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    let pending = snapshot(&fixture.path(&expected, 0));
    let fresh = manifest(17, 64);
    assert_eq!(
        fixture
            .store
            .stage_chunk(&fresh, 0, Pattern::new(0, 17))
            .await,
        Err(ChunkStagingError::CapacityExhausted)
    );
    assert_eq!(snapshot(&fixture.path(&expected, 0)), pending);
    drop(release);
    signal(finished).await;
    fresh_stage_after_release(&fixture.store, &fresh).await;
    fixture
        .store
        .verify_staged_chunk(&expected, 0)
        .await
        .unwrap();
    let retained = snapshot(&fixture.path(&expected, 0));
    assert_eq!(retained.length, 17);
    assert_eq!(retained.inode, pending.inode);
}

#[tokio::test]
async fn independently_constructed_stores_have_additive_budgets_but_clones_share_one() {
    let fixture = Fixture::new();
    let independent = ChunkStaging::open(super::scope(), &fixture.root, 1).unwrap();
    let first_manifest = manifest(17, 65);
    let second_manifest = manifest(17, 66);
    let (first_reader, first_release, first_started, first_finished) = held(17, 0);
    let (second_reader, second_release, second_started, second_finished) = held(17, 0);
    let runner = fixture.store.clone();
    let pending = first_manifest.clone();
    let first = tokio::spawn(async move { runner.stage_chunk(&pending, 0, first_reader).await });
    signal(first_started).await;
    let runner = independent.clone();
    let pending = second_manifest.clone();
    let second = tokio::spawn(async move { runner.stage_chunk(&pending, 0, second_reader).await });
    signal(second_started).await;
    let first_pending = snapshot(&fixture.path(&first_manifest, 0));
    let second_pending = snapshot(&fixture.path(&second_manifest, 0));
    let fresh = manifest(17, 67);
    for store in [
        &fixture.store,
        &fixture.store.clone(),
        &independent,
        &independent.clone(),
    ] {
        assert_eq!(
            store.stage_chunk(&fresh, 0, Pattern::new(0, 17)).await,
            Err(ChunkStagingError::CapacityExhausted)
        );
    }
    assert_eq!(snapshot(&fixture.path(&first_manifest, 0)), first_pending);
    assert_eq!(snapshot(&fixture.path(&second_manifest, 0)), second_pending);
    drop(second_release);
    assert_eq!(second.await.unwrap(), Ok(()));
    signal(second_finished).await;
    fresh_stage_after_release(&independent, &fresh).await;
    assert_eq!(
        fixture.store.verify_staged_chunk(&first_manifest, 0).await,
        Err(ChunkStagingError::CapacityExhausted)
    );
    drop(first_release);
    assert_eq!(first.await.unwrap(), Ok(()));
    signal(first_finished).await;
    fixture
        .store
        .verify_staged_chunk(&first_manifest, 0)
        .await
        .unwrap();
    independent
        .verify_staged_chunk(&second_manifest, 0)
        .await
        .unwrap();
}

struct FailedReader {
    kind: io::ErrorKind,
    calls: Arc<AtomicUsize>,
}

impl Read for FailedReader {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(io::Error::from(self.kind))
    }
}

#[tokio::test]
async fn input_io_failures_and_interruptions_retain_output_without_hidden_retries_or_capacity_leaks(
) {
    let fixture = Fixture::new();
    for (variant, kind) in [(68, io::ErrorKind::Other), (69, io::ErrorKind::Interrupted)] {
        let expected = manifest(17, variant);
        let calls = Arc::new(AtomicUsize::new(0));
        assert_eq!(
            fixture
                .store
                .stage_chunk(
                    &expected,
                    0,
                    FailedReader {
                        kind,
                        calls: calls.clone()
                    }
                )
                .await,
            Err(ChunkStagingError::Unconfirmed)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let retained = snapshot(&fixture.path(&expected, 0));
        assert_eq!(retained.length, 0);
        assert_eq!(
            fixture.store.verify_staged_chunk(&expected, 0).await,
            Err(ChunkStagingError::Unavailable)
        );
        assert_eq!(snapshot(&fixture.path(&expected, 0)), retained);
        let fresh = manifest(17, variant + 100);
        fresh_stage_after_release(&fixture.store, &fresh).await;
        assert_eq!(
            fixture
                .store
                .stage_chunk(&expected, 0, Pattern::new(0, 17))
                .await,
            Err(ChunkStagingError::Conflict)
        );
        assert_eq!(snapshot(&fixture.path(&expected, 0)), retained);
    }
}

struct PanicReader;

impl Read for PanicReader {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        panic!("synthetic file worker panic");
    }
}

#[tokio::test]
async fn panicking_reader_never_acknowledges_output_and_returns_capacity() {
    let fixture = Fixture::new();
    let expected = manifest(17, 70);
    assert_eq!(
        fixture.store.stage_chunk(&expected, 0, PanicReader).await,
        Err(ChunkStagingError::Unconfirmed)
    );
    let retained = snapshot(&fixture.path(&expected, 0));
    assert_eq!(retained.length, 0);
    let fresh = manifest(17, 71);
    fresh_stage_after_release(&fixture.store, &fresh).await;
    assert_eq!(
        fixture.store.verify_staged_chunk(&expected, 0).await,
        Err(ChunkStagingError::Unavailable)
    );
    assert_eq!(
        fixture
            .store
            .stage_chunk(&expected, 0, Pattern::new(0, 17))
            .await,
        Err(ChunkStagingError::Conflict)
    );
    assert_eq!(snapshot(&fixture.path(&expected, 0)), retained);
}
