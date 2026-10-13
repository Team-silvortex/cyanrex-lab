use crate::{fixture::*, support::*, *};
use std::fs;

#[tokio::test]
async fn without_cancel_selected_metadata_chunk_assembly_and_save_failures_keep_exact_original_wrapped_error_and_last_phase(
) {
    checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
        Box::pin(async move {
            let selected = selected_directory(f);
            let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
            let mut reply = Reply::good(&f.manifest, Kind::Manifest);
            reply.status = axum::http::StatusCode::NOT_FOUND;
            f.set_reply(Kind::Manifest, reply);
            let (handle, cancellation) = FileDeliveryCancelHandle::channel();
            let (observer, mut progress) = FileDeliveryObserver::channel();
            assert_eq!(
                controlled(f, &f.client(), &target, observer, cancellation)
                    .await
                    .err(),
                Some(wrapped(FileGrantedSaveError::Download(
                    FileGrantedDownloadError::Manifest(FileManifestClientError::InvalidResponse)
                )))
            );
            handle.cancel();
            let last = snapshot(
                f,
                &progress,
                FileDeliveryPhase::FetchingManifest,
                0,
                0,
                None,
            );
            closed(&mut progress, Some(last)).await;
            assert_eq!(indices(&f.take_requests()), [None]);
            assert!(bytes::tree(&f.root).is_empty() && bytes::tree(&selected).is_empty());
        })
    })
    .await;
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            let selected = selected_directory(f);
            let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
            let mut reply = Reply::good(&f.manifest, Kind::Chunk(1));
            reply
                .headers
                .insert("x-cyanrex-chunk-index", "0".parse().unwrap());
            f.set_reply(Kind::Chunk(1), reply);
            let (handle, cancellation) = FileDeliveryCancelHandle::channel();
            let (observer, mut progress) = FileDeliveryObserver::channel();
            assert_eq!(
                controlled(f, &f.client(), &target, observer, cancellation)
                    .await
                    .err(),
                Some(wrapped(chunk_error(
                    1,
                    FileChunkClientError::Receive(FileChunkReceiveError::InvalidResponse)
                )))
            );
            drop(handle);
            let last = snapshot(
                f,
                &progress,
                FileDeliveryPhase::StagingChunks,
                1,
                FILE_TRANSFER_CHUNK_BYTES,
                Some(2),
            );
            closed(&mut progress, Some(last)).await;
            assert_eq!(indices(&f.take_requests()), [None, Some(0), Some(1)]);
            assert_eq!(bytes::tree(&f.root).len(), 1);
            verify_prefix(f, 0);
            assert!(bytes::tree(&selected).is_empty());
            assert!(!f.root.join(output_name(&f.manifest)).exists());
        })
    })
    .await;
    let manifest = changed(
        &bytes::manifest(FILE_TRANSFER_CHUNK_BYTES + 17, 1),
        "sha256",
        serde_json::json!("a".repeat(64)),
    );
    checked(Fixture::with_manifest(manifest).await, |f| {
        Box::pin(async move {
            let selected = selected_directory(f);
            let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
            let (handle, cancellation) = FileDeliveryCancelHandle::channel();
            let (observer, mut progress) = FileDeliveryObserver::channel();
            assert_eq!(
                controlled(f, &f.client(), &target, observer, cancellation)
                    .await
                    .err(),
                Some(wrapped(FileGrantedSaveError::Download(
                    FileGrantedDownloadError::Download(FileDownloadError::Assembly(
                        ChunkStagingError::Unavailable
                    ))
                )))
            );
            drop(handle);
            let last = snapshot(
                f,
                &progress,
                FileDeliveryPhase::Assembling,
                2,
                f.manifest.byte_length(),
                Some(2),
            );
            closed(&mut progress, Some(last)).await;
            assert_eq!(indices(&f.take_requests()), [None, Some(0), Some(1)]);
            assert_eq!(bytes::tree(&f.root).len(), 2);
            for index in [0, 1] {
                verify_prefix(f, index);
            }
            assert!(!f.root.join(output_name(&f.manifest)).exists());
            assert!(bytes::tree(&selected).is_empty());
        })
    })
    .await;
    checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
        Box::pin(async move {
            let selected = selected_directory(f);
            fs::write(selected.join(CHOSEN_LEAF), b"unchanged host output").unwrap();
            let before = bytes::tree(&selected);
            let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
            let (handle, cancellation) = FileDeliveryCancelHandle::channel();
            let (observer, mut progress) = FileDeliveryObserver::channel();
            assert_eq!(
                controlled(f, &f.client(), &target, observer, cancellation)
                    .await
                    .err(),
                Some(wrapped(FileGrantedSaveError::Save(FileSaveError::Conflict)))
            );
            drop(handle);
            let last = snapshot(f, &progress, FileDeliveryPhase::Saving, 1, 17, Some(1));
            closed(&mut progress, Some(last)).await;
            assert_eq!(indices(&f.take_requests()), [None, Some(0)]);
            assert_eq!(bytes::tree(&selected), before);
            assert_eq!(bytes::tree(&f.root).len(), 2);
            assert_output(f, &f.root.join(output_name(&f.manifest)));
        })
    })
    .await;
}
