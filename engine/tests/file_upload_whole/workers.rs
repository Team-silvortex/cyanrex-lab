use super::{bytes::*, control::*, fixture::*, pool_gate::captured_pool, support::*};
use cyanrex_engine::services::{
    file_transfer::staging::{ChunkStaging, ChunkStagingError},
    file_upload_staging::{FileUploadStaging, UploadChunkError, UploadFileVerificationError},
};
use std::time::Duration;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_shared_controls_and_existing_staging_worker_admission_fail_closed() {
    let fixture = Fixture::new(1, 2).await;
    fixture
        .source
        .reserve(upload(1), owner(1), account(1), &fixture.manifest)
        .await
        .unwrap();
    stage_all(&fixture).await;
    let rows = fixture.rows().await;
    let oids = fixture.oids().await;
    let files = tree(&fixture.root);
    let (adapter, pool, release, started) = captured_pool(&fixture, 1).await;
    let clone = adapter.clone();
    let mut pending = Box::pin(adapter.verify_file(upload(1), owner(1), account(1)));
    tokio::select! {result=&mut pending=>panic!("initial capture-return barrier absent: {result:?}"),_=signal(started)=>{}}
    assert_eq!(
        clone
            .verify_file(upload(1), owner(1), account(1))
            .await
            .unwrap_err(),
        UploadFileVerificationError::CapacityExhausted
    );
    assert!(matches!(
        clone.prepare(upload(1), owner(1), account(1), 0).await,
        Err(UploadChunkError::CapacityExhausted)
    ));
    let independent =
        FileUploadStaging::new(fixture.source.clone(), fixture.staging.clone(), 1).unwrap();
    good(&fixture, &independent).await;
    assert_eq!(fixture.rows().await, rows);
    assert_eq!(fixture.oids().await, oids);
    assert_eq!(tree(&fixture.root), files);
    drop(pending);
    drop(release);
    good(&fixture, &adapter).await;
    pool.close().await;
    fixture.pool_restored().await;
    fixture.cleanup().await;

    let fixture = Fixture::new(4, 1).await;
    fixture
        .source
        .reserve(upload(1), owner(1), account(1), &fixture.manifest)
        .await
        .unwrap();
    stage_all(&fixture).await;
    let rows = fixture.rows().await;
    let oids = fixture.oids().await;
    let original = tree(&fixture.root);
    let legacy_manifest = manifest(17, 900);
    let (reader, release, started, finished) = held(0, 17, 0);
    let mut pending = Box::pin(fixture.staging.stage_chunk(&legacy_manifest, 0, reader));
    tokio::select! {result=&mut pending=>panic!("actual old staging reader did not block: {result:?}"),_=signal(started)=>{}}
    let partial = tree(&fixture.root);
    assert_eq!(partial.len(), 2);
    assert!(partial.contains(&original[0]));
    for adapter in [
        fixture.adapter.clone(),
        FileUploadStaging::new(fixture.source.clone(), fixture.staging.clone(), 4).unwrap(),
    ] {
        assert_eq!(
            adapter
                .verify_file(upload(1), owner(1), account(1))
                .await
                .unwrap_err(),
            UploadFileVerificationError::CapacityExhausted
        );
        assert_eq!(fixture.rows().await, rows);
        assert_eq!(fixture.oids().await, oids);
        assert_eq!(tree(&fixture.root), partial);
    }
    let separate = FileUploadStaging::new(
        fixture.source.clone(),
        ChunkStaging::open(scope(), &fixture.root, 1).unwrap(),
        4,
    )
    .unwrap();
    good(&fixture, &separate).await;
    drop(pending);
    assert_eq!(
        fixture
            .adapter
            .verify_file(upload(1), owner(1), account(1))
            .await
            .unwrap_err(),
        UploadFileVerificationError::CapacityExhausted
    );
    assert_eq!(fixture.rows().await, rows);
    assert_eq!(tree(&fixture.root), partial);
    drop(release);
    signal(finished).await;
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match fixture
                .staging
                .verify_staged_chunk(&manifest(17, 901), 0)
                .await
            {
                Err(ChunkStagingError::CapacityExhausted) => tokio::task::yield_now().await,
                Err(ChunkStagingError::Unavailable) => break,
                other => panic!("unexpected release probe: {other:?}"),
            }
        }
    })
    .await
    .expect("cancelled actual legacy worker did not release its slot");
    let retained = tree(&fixture.root);
    assert!(retained.contains(&original[0]));
    assert!(retained.iter().all(|entry| entry.length == 17));
    good(&fixture, &fixture.adapter).await;
    assert_eq!(fixture.rows().await, rows);
    assert_eq!(fixture.oids().await, oids);
    assert_eq!(tree(&fixture.root), retained);
    fixture.pool_restored().await;
    fixture.cleanup().await;
}
