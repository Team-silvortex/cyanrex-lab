use crate::{fixture::*, support::*, *};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
};

#[tokio::test]
async fn fresh_download_with_preexisting_selected_target_fails_at_save_and_preserves_completed_private_source(
) {
    checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
        Box::pin(async move {
            let parent = selected_parent(f);
            fs::write(parent.join(CHOSEN_LEAF), b"untouched host-owned output").unwrap();
            fs::set_permissions(parent.join(CHOSEN_LEAF), fs::Permissions::from_mode(0o400))
                .unwrap();
            let before = bytes::tree(&parent);
            let target = FileSaveTarget::open(&parent, CHOSEN_LEAF).unwrap();
            let error = deliver(f, &f.client(), &target).await.unwrap_err();
            assert_eq!(error, FileGrantedSaveError::Save(FileSaveError::Conflict));
            redacted(f, &error);
            assert_eq!(
                indices(&f.take_requests()),
                [None, Some(0)],
                "a selected-path conflict cannot skip current metadata/chunk authorization"
            );
            assert_eq!(bytes::tree(&parent), before);
            assert_eq!(bytes::tree(&f.root).len(), 2);
            assert_output(f, &f.root.join(output_name(&f.manifest)));
        })
    })
    .await;
}

#[tokio::test]
async fn metadata_chunk_and_whole_first_failures_stop_before_later_http_or_selected_save_without_retry(
) {
    for preexisting_target in [false, true] {
        checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
            Box::pin(async move {
                let parent = selected_parent(f);
                if preexisting_target {
                    fs::write(parent.join(CHOSEN_LEAF), b"unchanged").unwrap();
                }
                let before = bytes::tree(&parent);
                let target = FileSaveTarget::open(&parent, CHOSEN_LEAF).unwrap();
                let mut reply = Reply::good(&f.manifest, Kind::Manifest);
                reply.status = axum::http::StatusCode::NOT_FOUND;
                f.set_reply(Kind::Manifest, reply);
                let error = deliver(f, &f.client(), &target).await.unwrap_err();
                assert_eq!(
                    error,
                    metadata_error(FileManifestClientError::InvalidResponse)
                );
                redacted(f, &error);
                assert_eq!(indices(&f.take_requests()), [None]);
                assert!(bytes::tree(&f.root).is_empty());
                assert_eq!(
                    bytes::tree(&parent),
                    before,
                    "early grant/metadata failure must not be masked by target existence"
                );
            })
        })
        .await;
    }
    checked(
        Fixture::with_manifest(bytes::manifest(2 * FILE_TRANSFER_CHUNK_BYTES + 17, 1)).await,
        |f| {
            Box::pin(async move {
                let parent = selected_parent(f);
                let target = FileSaveTarget::open(&parent, CHOSEN_LEAF).unwrap();
                let mut reply = Reply::good(&f.manifest, Kind::Chunk(1));
                reply
                    .headers
                    .insert("x-cyanrex-chunk-index", "0".parse().unwrap());
                f.set_reply(Kind::Chunk(1), reply);
                let error = deliver(f, &f.client(), &target).await.unwrap_err();
                assert_eq!(
                    error,
                    chunk_error(
                        1,
                        FileChunkClientError::Receive(FileChunkReceiveError::InvalidResponse)
                    )
                );
                redacted(f, &error);
                let requests = f.take_requests();
                assert_eq!(
                    indices(&requests),
                    [None, Some(0), Some(1)],
                    "no retry or third chunk is fetched after the first failing chunk"
                );
                let prefix = bytes::tree(&f.root);
                assert_eq!(prefix, requests[2].staged_before_request);
                assert_eq!(prefix.len(), 1);
                verify_chunk(
                    &f.manifest,
                    0,
                    &mut fs::File::open(f.root.join(chunk_name(&f.manifest, 0))).unwrap(),
                )
                .unwrap();
                assert!(!f.root.join(output_name(&f.manifest)).exists());
                assert!(bytes::tree(&parent).is_empty());
            })
        },
    )
    .await;
    let manifest = changed(
        &bytes::manifest(FILE_TRANSFER_CHUNK_BYTES + 17, 1),
        "sha256",
        serde_json::json!("a".repeat(64)),
    );
    checked(Fixture::with_manifest(manifest).await, |f| {
        Box::pin(async move {
            let parent = selected_parent(f);
            let target = FileSaveTarget::open(&parent, CHOSEN_LEAF).unwrap();
            let error = deliver(f, &f.client(), &target).await.unwrap_err();
            assert_eq!(error, assembly_error(ChunkStagingError::Unavailable));
            redacted(f, &error);
            assert_eq!(indices(&f.take_requests()), [None, Some(0), Some(1)]);
            assert_eq!(bytes::tree(&f.root).len(), 2);
            for index in [0, 1] {
                verify_chunk(
                    &f.manifest,
                    index,
                    &mut fs::File::open(f.root.join(chunk_name(&f.manifest, index))).unwrap(),
                )
                .unwrap();
            }
            assert!(!f.root.join(output_name(&f.manifest)).exists());
            assert!(bytes::tree(&parent).is_empty());
        })
    })
    .await;
}

#[tokio::test]
async fn actual_selected_parent_drift_is_save_unavailable_after_download_and_never_repairs_or_writes_either_parent(
) {
    for replaced in [false, true] {
        checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
            Box::pin(async move {
                let parent = selected_parent(f);
                let target = FileSaveTarget::open(&parent, CHOSEN_LEAF).unwrap();
                let retained = parent.with_file_name("retained-selected-parent");
                if replaced {
                    fs::rename(&parent, &retained).unwrap();
                    fs::DirBuilder::new().mode(0o700).create(&parent).unwrap();
                } else {
                    fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
                }
                let current = bytes::snapshot(&parent);
                let old = replaced.then(|| bytes::snapshot(&retained));
                let error = deliver(f, &f.client(), &target).await.unwrap_err();
                assert_eq!(
                    error,
                    FileGrantedSaveError::Save(FileSaveError::Unavailable),
                    "actual filesystem drift remains a late save error, not cheap invalid input"
                );
                redacted(f, &error);
                assert_eq!(indices(&f.take_requests()), [None, Some(0)]);
                assert_eq!(bytes::tree(&f.root).len(), 2);
                assert_output(f, &f.root.join(output_name(&f.manifest)));
                assert_eq!(bytes::snapshot(&parent), current);
                assert!(bytes::tree(&parent).is_empty());
                if let Some(old) = old {
                    assert_eq!(bytes::snapshot(&retained), old);
                    assert!(bytes::tree(&retained).is_empty());
                }
            })
        })
        .await;
    }
}
