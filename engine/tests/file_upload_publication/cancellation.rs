use super::{control::*, fixture::*, support::*};
use cyanrex_engine::services::file_upload_staging::{
    FileUploadPublicationError, FileUploadPublicationState,
};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_actual_sql_cancellation_is_terminal_but_does_not_burn_uninserted_coordinate() {
    let f = Fixture::ready(17).await;
    stage_all(&f).await;
    let mut attempt = f
        .publisher
        .prepare(upload(1), owner(1), account(1))
        .await
        .unwrap();
    let before = snapshot_all(&f).await;
    drop(f.publisher.publish(&mut attempt));
    assert_eq!(attempt.state(), FileUploadPublicationState::Ready);
    assert_eq!(snapshot_all(&f).await, before);
    let (holder, pid) = advisory(&f).await;
    fault(
        &f,
        &format!(
            "PERFORM pg_advisory_xact_lock(hashtext('{}'),877);",
            f.publication_schema
        ),
        true,
    )
    .await;
    let mut pending = Box::pin(f.publisher.publish(&mut attempt));
    tokio::select! {result=&mut pending=>panic!("publication missed actual deferred SQL wait: {result:?}"),_=f.wait_blocked(Store::Publication,pid)=>{}}
    assert!(hit(&f).await);
    assert_eq!(snapshot_all(&f).await, before);
    assert!(f
        .publication
        .get(upload(1), owner(1), account(1))
        .await
        .unwrap()
        .is_none());
    drop(pending);
    holder.rollback().await.unwrap();
    let (drain, _) = lock(&f, Store::Publication).await;
    drain.rollback().await.unwrap();
    for store in [Store::Source, Store::Journal, Store::Publication] {
        f.pool_restored(store).await;
    }
    assert_eq!(attempt.state(), FileUploadPublicationState::Unknown);
    assert_eq!(snapshot_all(&f).await, before);
    remove_fault(&f).await;
    assert_eq!(
        f.publisher.clone().publish(&mut attempt).await.unwrap_err(),
        FileUploadPublicationError::InvalidInput
    );
    assert_eq!(snapshot_all(&f).await, before);
    publish(&f, &f.publisher.clone(), 1, &f.manifest).await;
    f.cleanup().await;

    // Actual old staging worker lifetime, not a timing estimate, governs publication's byte job.
    let mut f = Fixture::new(1, 1, 16, 8).await;
    f.journal.install_empty_namespace().await.unwrap();
    f.publication.install_empty_namespace().await.unwrap();
    f.manifest = manifest(17, 1);
    f.source
        .reserve(upload(1), owner(1), account(1), &f.manifest)
        .await
        .unwrap();
    stage_all(&f).await;
    let mut next = f
        .publisher
        .prepare(upload(1), owner(1), account(1))
        .await
        .unwrap();
    let (reader, release, started, finished) = held(0, 17, 0);
    let mut worker = Box::pin(f.staging.stage_chunk(&f.manifest, 0, reader));
    tokio::select! {result=&mut worker=>panic!("legacy worker missed actual read barrier: {result:?}"),_=signal(started)=>{}}
    drop(worker);
    let partial = snapshot_all(&f).await;
    assert_eq!(
        f.publisher.publish(&mut next).await.unwrap_err(),
        FileUploadPublicationError::CapacityExhausted
    );
    assert_eq!(next.state(), FileUploadPublicationState::Unknown);
    assert_eq!(snapshot_all(&f).await, partial);
    drop(release);
    signal(finished).await;
    tokio::time::timeout(std::time::Duration::from_secs(5),async{loop{
        match f.staging.verify_staged_chunk(&f.manifest,0).await {
            Err(cyanrex_engine::services::file_transfer::staging::ChunkStagingError::CapacityExhausted)=>tokio::task::yield_now().await,
            result=>{result.unwrap();break;}
        }
    }}).await.expect("dropped caller's actual worker did not restore its lifetime admission");
    publish(&f, &f.publisher, 1, &f.manifest).await;
    f.cleanup().await;
}
