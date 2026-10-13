use crate::{fixture::*, support::*, *};

#[tokio::test]
async fn slow_or_dropped_progress_reader_is_not_a_cancel_signal_and_completed_observation_holds_no_worker_or_flow(
) {
    for drop_reader in [false, true] {
        checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
            Box::pin(async move {
                let selected = selected_directory(f);
                let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
                let client = f.client();
                let (handle, cancellation) = FileDeliveryCancelHandle::channel();
                let (observer, progress) = FileDeliveryObserver::channel();
                let mut reader = Some(progress);
                if drop_reader {
                    drop(reader.take());
                }
                controlled(f, &client, &target, observer, cancellation)
                    .await
                    .unwrap();
                drop(handle);
                let copy = if let Some(progress) = reader.as_mut() {
                    let last = snapshot(f, progress, FileDeliveryPhase::Complete, 1, 17, Some(1));
                    closed(progress, Some(last)).await;
                    Some(last)
                } else {
                    None
                };
                assert_output(f, &selected.join(CHOSEN_LEAF));
                let source = bytes::tree(&f.root);
                let output = bytes::tree(&selected);
                within(f.staging.verify_staged_chunk(&f.manifest, 0))
                    .await
                    .unwrap();
                assert_eq!(
                    within(client.fetch_manifest(
                        TOKEN,
                        grant(),
                        upload(),
                        &FileVersionRef::from_manifest(&f.manifest)
                    ))
                    .await
                    .unwrap(),
                    f.manifest
                );
                assert_eq!(
                    plain(f, &client, &target).await.err(),
                    Some(staging_conflict())
                );
                if let Some(progress) = reader.as_ref() {
                    assert_eq!(progress.latest(), copy);
                }
                assert_eq!(bytes::tree(&f.root), source);
                assert_eq!(bytes::tree(&selected), output);
                assert_eq!(
                    indices(&f.take_requests()),
                    [None, Some(0), None, None, Some(0)]
                );
            })
        })
        .await;
    }
}
