use crate::{fixture::*, support::*, *};

#[tokio::test]
async fn never_read_or_dropped_progress_receivers_do_not_backpressure_delivery_or_retain_old_flow_manifest_or_worker_capacity(
) {
    for drop_reader in [false, true] {
        checked(Fixture::new().await, |f| Box::pin(async move {
            let selected = selected_directory(f);
            let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
            let client = f.client();
            let (observer, progress) = FileDeliveryObserver::channel();
            let mut reader = Some(progress);
            if drop_reader { drop(reader.take()); }
            observed(f, &client, &target, observer).await.unwrap();
            assert_output(f, &selected.join(CHOSEN_LEAF));
            let copied = if let Some(progress) = reader.as_mut() {
                let last = snapshot(f, progress, FileDeliveryPhase::Complete, 2, f.manifest.byte_length(), Some(2));
                closed(progress, Some(last)).await;
                Some(last)
            } else { None };
            let source = bytes::tree(&f.root);
            let destination = bytes::tree(&selected);
            within(f.staging.verify_staged_chunk(&f.manifest, 0)).await.unwrap();
            assert_eq!(within(client.fetch_manifest(TOKEN, grant(), upload(), &FileVersionRef::from_manifest(&f.manifest)))
                .await.unwrap(), f.manifest);
            assert_eq!(within(client.fetch_granted_and_assemble(TOKEN, grant(), upload(), &FileVersionRef::from_manifest(&f.manifest)))
                .await.err(), Some(FileGrantedDownloadError::Download(FileDownloadError::Chunk {
                    index: 0, error: FileChunkClientError::Receive(FileChunkReceiveError::Staging(ChunkStagingError::Conflict)) })),
                "retaining progress/snapshot does not hold a whole flow or turn old chunks into adopted progress");
            if let Some(progress) = reader.as_ref() {
                assert_eq!(progress.latest(), copied, "old unobserved operations cannot rewrite this closed per-call slot");
            }
            assert_eq!(bytes::tree(&f.root), source);
            assert_eq!(bytes::tree(&selected), destination);
            assert_eq!(indices(&f.take_requests()), [None, Some(0), Some(1), None, None, Some(0)]);
        })).await;
    }
}
