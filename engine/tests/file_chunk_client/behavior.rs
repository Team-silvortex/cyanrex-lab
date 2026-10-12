use crate::{fixture::*, support::*, *};
use cyanrex_engine::services::{
    file_chunk_receiver::FileChunkReceiveError, file_transfer::staging::ChunkStagingError,
};
use std::{fs, os::unix::fs::PermissionsExt};

#[tokio::test]
async fn client_real_cross_chunk_requests_save_only_exact_selected_private_chunks() {
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            let client = client(f, 1);
            let manifest = bytes::manifest(FILE_TRANSFER_CHUNK_BYTES + 17, 2);
            for index in [1, 0] {
                f.set_reply(Reply::good(&manifest, index));
                let received = client
                    .fetch_and_stage(TOKEN, grant(), upload(), &manifest, index)
                    .await
                    .unwrap();
                assert_eq!(received.file(), &FileVersionRef::from_manifest(&manifest));
                assert_eq!(received.chunk_index(), index);
                assert_eq!(
                    received.byte_length(),
                    manifest.chunk(index).unwrap().byte_length
                );
                assert_eq!(
                    received.chunk_sha256(),
                    &manifest.chunk(index).unwrap().sha256
                );
                let saved =
                    saved_at(&f.root.join(name_at(&manifest, index)), &manifest, index).unwrap();
                saved.verified.unwrap();
                let start = index as u64 * FILE_TRANSFER_CHUNK_BYTES;
                assert_eq!(
                    saved.bytes,
                    (start..start + manifest.chunk(index).unwrap().byte_length)
                        .map(bytes::byte)
                        .collect::<Vec<_>>()
                );
                assert_eq!(saved.mode & 0o777, 0o400);
                assert_eq!(saved.links, 1);
                if index == 1 {
                    assert_eq!(bytes::tree(&f.root).len(), 1);
                    assert!(!f.root.join(name_at(&manifest, 0)).exists());
                }
            }
            assert_eq!(f.observed().len(), 2);
            assert!(f.observed()[0]
                .uri
                .query()
                .unwrap()
                .split('&')
                .any(|pair| pair == "chunk_index=1"));
            assert_eq!(bytes::tree(&f.root).len(), 2);
            healthy(f, &client, 3).await;
        })
    })
    .await;
}

#[tokio::test]
async fn client_existing_output_and_private_root_drift_are_not_adopted_or_repaired() {
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            let client = client(f, 1);
            healthy(f, &client, 1).await;
            let old = bytes::tree(&f.root);
            f.set_reply(Reply::good(&f.manifest, 0));
            denied(
                f,
                &client,
                &f.manifest,
                0,
                FileChunkClientError::Receive(FileChunkReceiveError::Staging(
                    ChunkStagingError::Conflict,
                )),
            )
            .await;
            assert_eq!(bytes::tree(&f.root), old);
            let manifest = bytes::manifest(17, 2);
            f.set_reply(Reply::good(&manifest, 0));
            fs::set_permissions(&f.root, fs::Permissions::from_mode(0o770)).unwrap();
            let injected = bytes::snapshot(&f.root);
            denied(
                f,
                &client,
                &manifest,
                0,
                FileChunkClientError::Receive(FileChunkReceiveError::Staging(
                    ChunkStagingError::Unavailable,
                )),
            )
            .await;
            assert_eq!(bytes::snapshot(&f.root), injected);
            assert_eq!(bytes::tree(&f.root), old);
            fs::set_permissions(&f.root, fs::Permissions::from_mode(0o700)).unwrap();
            healthy(f, &client, 2).await;
        })
    })
    .await;
}
