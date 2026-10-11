use super::{bytes::*, control::*, fixture::*, support::*};
use cyanrex_engine::services::{
    file_transfer::staging::ChunkStagingError,
    file_upload_chunk_journal::{FileUploadChunkJournalError, PersistentChunkState},
    file_upload_staging::{
        FileUploadStaging, JournaledFileUploadStaging, JournaledUploadChunkError, UploadChunkError,
        UploadChunkState,
    },
};
use std::{sync::atomic::Ordering, time::Duration};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_cancelled_precommit_and_running_worker_preserve_exact_unknown_outcomes() {
    let fixture = Fixture::new(1, 1, 16).await;
    fixture.journal.install_empty_namespace().await.unwrap();
    for seed in 1..=2 {
        fixture
            .source
            .reserve(upload(seed), owner(1), account(seed), &manifest(17, seed))
            .await
            .unwrap();
    }
    let source = fixture.rows(false).await;
    let journal = fixture.rows(true).await;
    let source_oids = fixture.oids(false).await;
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
    let mut abandoned = fixture
        .journaled
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    let reader = Pattern::new(0, 17);
    let calls = reader.calls.clone();
    let mut pending = Box::pin(fixture.journaled.stage_chunk(&mut abandoned, reader));
    tokio::select! {result=&mut pending=>panic!("expected actual pre-ack SQL wait: {result:?}"),_=fixture.wait_blocked(true,pid)=>{}}
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(tree(&fixture.root).is_empty());
    assert!(hit(&fixture).await);
    drop(pending);
    assert_eq!(abandoned.state(), UploadChunkState::Unknown);
    holder.rollback().await.unwrap();
    drain(&fixture).await;
    fixture.pool_restored(true).await;
    assert_eq!(fixture.rows(true).await, journal);
    assert_eq!(fixture.rows(false).await, source);
    assert!(tree(&fixture.root).is_empty());
    remove_fault(&fixture).await;
    assert!(matches!(
        fixture
            .journaled
            .stage_chunk(&mut abandoned, Pattern::new(0, 17))
            .await,
        Err(JournaledUploadChunkError::Local(
            UploadChunkError::InvalidInput
        ))
    ));

    // A new prepare is a trusted call, not restoration, and does not inspect occupied coordinates.
    // Its subsequent stage can succeed here only because this fixture proved the prior rollback.
    let independent = JournaledFileUploadStaging::new(
        FileUploadStaging::new(fixture.source.clone(), fixture.staging.clone(), 1).unwrap(),
        fixture.journal.clone(),
    )
    .unwrap();
    let mut first = fixture
        .journaled
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    let mut waiting = fixture
        .journaled
        .prepare(upload(2), owner(1), account(2), 0)
        .await
        .unwrap();
    let mut same = independent
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    let mut other = independent
        .prepare(upload(2), owner(1), account(2), 0)
        .await
        .unwrap();
    assert!(matches!(
        independent
            .stage_chunk(&mut first, Pattern::new(0, 17))
            .await,
        Err(JournaledUploadChunkError::Local(
            UploadChunkError::InvalidInput
        ))
    ));
    assert_eq!(first.state(), UploadChunkState::Ready);
    let (reader, release, started, finished) = held(0, 17, 0);
    let mut pending = Box::pin(fixture.journaled.stage_chunk(&mut first, reader));
    tokio::select! {result=&mut pending=>panic!("expected actual blocking worker: {result:?}"),_=signal(started)=>{}}
    let partial = tree(&fixture.root);
    let acknowledged = fixture.rows(true).await;
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
    let reader = Pattern::new(0, 17);
    let calls = reader.calls.clone();
    assert!(matches!(
        fixture
            .journaled
            .clone()
            .stage_chunk(&mut waiting, reader)
            .await,
        Err(JournaledUploadChunkError::Local(
            UploadChunkError::CapacityExhausted
        ))
    ));
    assert_eq!(waiting.state(), UploadChunkState::Ready);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.rows(true).await, acknowledged);
    drop(pending);
    assert_eq!(first.state(), UploadChunkState::Unknown);
    let reader = Pattern::new(0, 17);
    let calls = reader.calls.clone();
    assert!(matches!(
        independent.stage_chunk(&mut same, reader).await,
        Err(JournaledUploadChunkError::Journal(
            FileUploadChunkJournalError::Conflict
        ))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(same.state(), UploadChunkState::Unknown);
    assert_eq!(fixture.rows(true).await, acknowledged);
    let reader = Pattern::new(0, 17);
    let calls = reader.calls.clone();
    assert!(matches!(
        independent.stage_chunk(&mut other, reader).await,
        Err(JournaledUploadChunkError::Local(
            UploadChunkError::CapacityExhausted
        ))
    ));
    assert_eq!(other.state(), UploadChunkState::Unknown);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.rows(true).await[1].len(), 2);
    assert_eq!(tree(&fixture.root), partial);
    assert_eq!(fixture.rows(false).await, source);
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
                result => panic!("unexpected legacy probe: {result:?}"),
            }
        }
    })
    .await
    .expect("cancelled actual worker never released its own permit");
    let retained = tree(&fixture.root);
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].length, 17);
    assert_eq!(retained[0].inode, partial[0].inode);
    let before = fixture.rows(true).await;
    let (reopened, pool) = reopen(&fixture).await;
    metadata(
        &reopened
            .get(upload(1), owner(1), account(1), 0)
            .await
            .unwrap()
            .unwrap(),
        0,
        &fixture.manifest,
        PersistentChunkState::Unknown,
    );
    assert_eq!(
        reopened
            .get(upload(2), owner(1), account(2), 0)
            .await
            .unwrap()
            .unwrap()
            .state(),
        PersistentChunkState::Unknown
    );
    assert_eq!(fixture.rows(true).await, before);
    assert_eq!(tree(&fixture.root), retained);
    pool.close().await;
    let mut fresh = fixture
        .journaled
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    assert!(matches!(
        fixture
            .journaled
            .stage_chunk(&mut fresh, Pattern::new(0, 17))
            .await,
        Err(JournaledUploadChunkError::Journal(
            FileUploadChunkJournalError::Conflict
        ))
    ));
    assert_eq!(fixture.rows(true).await, before);
    assert_eq!(tree(&fixture.root), retained);
    assert_eq!(fixture.oids(false).await, source_oids);
    healthy(&fixture, 3).await;
    fixture.pool_restored(false).await;
    fixture.pool_restored(true).await;
    fixture.cleanup().await;
}
