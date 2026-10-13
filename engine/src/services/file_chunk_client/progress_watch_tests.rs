use super::*;

#[tokio::test]
async fn latest_copy_coalesces_and_preserves_final_unseen_value_after_channel_closes() {
    let (observer, mut progress) = FileDeliveryObserver::channel();
    assert_eq!(progress.latest(), None);
    observer.report(FileDeliveryPhase::FetchingManifest, 0, 0, 17, None);
    let copied = progress.latest().unwrap();
    // Holding a copied value cannot hold a watch Ref/read lock or stall later reports.
    observer.report(FileDeliveryPhase::StagingChunks, 1, 17, 17, Some(1));
    observer.report(FileDeliveryPhase::Complete, 1, 17, 17, Some(1));
    assert_eq!(copied.phase(), FileDeliveryPhase::FetchingManifest);
    drop(observer);
    assert_eq!(
        progress.changed().await.unwrap().phase(),
        FileDeliveryPhase::Complete
    );
    assert_eq!(progress.changed().await, None);
    assert_eq!(progress.latest().unwrap().staged_bytes(), 17);
    let (observer, progress) = FileDeliveryObserver::channel();
    drop(progress);
    observer.report(FileDeliveryPhase::FetchingManifest, 0, 0, 0, None);
    // This is watch/type behavior only, not evidence of an actual stage or saved receipt.
}
