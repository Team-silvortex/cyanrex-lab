use crate::{fixture::*, support::*, *};
use std::{fs, os::unix::fs::PermissionsExt};

#[tokio::test]
async fn metadata_or_later_chunk_first_failure_closes_with_only_confirmed_prefix_and_no_later_get_or_target(
) {
    checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
        Box::pin(async move {
            let selected = selected_directory(f);
            let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
            let mut reply = Reply::good(&f.manifest, Kind::Manifest);
            reply.status = axum::http::StatusCode::NOT_FOUND;
            f.set_reply(Kind::Manifest, reply);
            let (observer, mut progress) = FileDeliveryObserver::channel();
            assert_eq!(
                observed(f, &f.client(), &target, observer).await.err(),
                Some(FileGrantedSaveError::Download(
                    FileGrantedDownloadError::Manifest(FileManifestClientError::InvalidResponse)
                ))
            );
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
    checked(
        Fixture::with_manifest(bytes::manifest(2 * FILE_TRANSFER_CHUNK_BYTES + 17, 1)).await,
        |f| {
            Box::pin(async move {
                let selected = selected_directory(f);
                let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
                let mut reply = Reply::good(&f.manifest, Kind::Chunk(1));
                reply
                    .headers
                    .insert("x-cyanrex-chunk-index", "0".parse().unwrap());
                f.set_reply(Kind::Chunk(1), reply);
                let (observer, mut progress) = FileDeliveryObserver::channel();
                assert_eq!(
                    observed(f, &f.client(), &target, observer).await.err(),
                    Some(chunk_error(
                        1,
                        FileChunkClientError::Receive(FileChunkReceiveError::InvalidResponse)
                    ))
                );
                let last = snapshot(
                    f,
                    &progress,
                    FileDeliveryPhase::StagingChunks,
                    1,
                    FILE_TRANSFER_CHUNK_BYTES,
                    Some(3),
                );
                closed(&mut progress, Some(last)).await;
                let requests = f.take_requests();
                assert_eq!(
                    indices(&requests),
                    [None, Some(0), Some(1)],
                    "no retry/third chunk follows first failure"
                );
                assert_eq!(bytes::tree(&f.root), requests[2].staged_before_request);
                assert_eq!(bytes::tree(&f.root).len(), 1);
                verify_prefix(f, 0);
                assert!(!f.root.join(output_name(&f.manifest)).exists());
                assert!(bytes::tree(&selected).is_empty());
            })
        },
    )
    .await;
}

#[tokio::test]
async fn all_chunk_bytes_staged_with_wrong_whole_digest_close_as_assembling_not_complete_or_saving()
{
    let manifest = changed(
        &bytes::manifest(FILE_TRANSFER_CHUNK_BYTES + 17, 1),
        "sha256",
        serde_json::json!("a".repeat(64)),
    );
    checked(Fixture::with_manifest(manifest).await, |f| {
        Box::pin(async move {
            let selected = selected_directory(f);
            let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
            let (observer, mut progress) = FileDeliveryObserver::channel();
            assert_eq!(
                observed(f, &f.client(), &target, observer).await.err(),
                Some(FileGrantedSaveError::Download(
                    FileGrantedDownloadError::Download(FileDownloadError::Assembly(
                        ChunkStagingError::Unavailable
                    ))
                ))
            );
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
            assert!(
                bytes::tree(&selected).is_empty(),
                "100% staged bytes are not whole integrity or selected-save success"
            );
        })
    })
    .await;
}

#[tokio::test]
async fn fresh_save_conflict_after_real_assembly_closes_as_saving_without_complete_and_preserves_existing_output(
) {
    checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
        Box::pin(async move {
            let selected = selected_directory(f);
            let path = selected.join(CHOSEN_LEAF);
            fs::write(&path, b"host-selected existing bytes").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
            let before = bytes::tree(&selected);
            let parent = directory_policy(&selected);
            let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
            let (observer, mut progress) = FileDeliveryObserver::channel();
            assert_eq!(
                observed(f, &f.client(), &target, observer).await.err(),
                Some(FileGrantedSaveError::Save(FileSaveError::Conflict))
            );
            let last = snapshot(f, &progress, FileDeliveryPhase::Saving, 1, 17, Some(1));
            closed(&mut progress, Some(last)).await;
            assert_eq!(indices(&f.take_requests()), [None, Some(0)]);
            assert_eq!(bytes::tree(&f.root).len(), 2);
            assert_output(f, &f.root.join(output_name(&f.manifest)));
            assert_eq!(bytes::tree(&selected), before);
            assert_eq!(directory_policy(&selected), parent);
        })
    })
    .await;
}
