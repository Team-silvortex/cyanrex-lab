//! Actual current-Session Source, one composed distribution router and host-selected output.
use axum::{body::Body, extract::State, http::Request, Router};
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    platform_http::{
        build_file_chunk_read_router, build_file_download_manifest_router, FileChunkReadHttpState,
        FileDownloadManifestHttpState,
    },
    services::{
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthSource, SessionBindCommand, SessionFileChunkReadRequest,
            SessionFileChunkReadWorkspace, SessionFileDownloadRequest,
            SessionFilePublisherGrantWorkspace, SessionFilePublishingWorkspace,
            SessionFileRecipientGrantIssue, SessionFileRecipientGrantWorkspace,
            SessionFileSharingWorkspace, SessionPolicyCommand,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, LegacyAccessPolicy, StoredLegacyAccessPolicy,
            StoredLegacyIdentity,
        },
        file_chunk_client::{
            FileChunkClient, FileChunkClientConfig, FileChunkClientError, FileChunkTransport,
            FileControlledSaveError, FileDeliveryCancelHandle, FileDeliveryObserver,
            FileDeliveryPhase, FileDeliveryProgress, FileDownloadError, FileGrantedDownloadError,
            FileGrantedSaveError, FileManifestClientError,
        },
        file_chunk_receiver::FileChunkReceiveError,
        file_publisher_grant_store::{FilePublisherGrantStore, StoredFilePublisherGrant},
        file_recipient_grant_store::FileRecipientGrantStore,
        file_transfer::{
            staging::{ChunkStaging, FileSaveTarget},
            verify_chunk, verify_file, FILE_TRANSFER_BUFFER_BYTES,
        },
    },
};
use futures_util::FutureExt;
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    panic::AssertUnwindSafe,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tower::ServiceExt;
use uuid::Uuid;

#[allow(dead_code)]
#[path = "../durable_auth_source/fixture.rs"]
mod source_fixture;
use source_fixture::{authority, id, otp, PASSWORD};
#[allow(dead_code)]
#[path = "../durable_collaboration/fixture.rs"]
mod identity_fixture;
use identity_fixture::scope;
mod fixture {
    pub use super::identity_fixture::scope;
}
#[allow(dead_code)]
#[path = "../file_upload_staging/bytes.rs"]
mod bytes;
#[allow(dead_code)]
#[path = "../session_file_chunk_read/fixture.rs"]
mod chunk_fixture;
#[allow(dead_code)]
#[path = "../session_file_publisher_grant/fixture.rs"]
mod grant_fixture;
#[allow(dead_code)]
#[path = "../session_file_publication/fixture.rs"]
mod publication_fixture;
#[allow(dead_code)]
#[path = "../session_file_sharing/fixture.rs"]
mod sharing_fixture;
#[path = "../file_manifest_client/source_support.rs"]
// Reuse its pure assertions; its old independent-router harness is unused only in this target.
#[allow(dead_code)]
mod support;
use support::*;
#[path = "distribution_bridge.rs"]
mod distribution_bridge;
use distribution_bridge::{DistributionBridge as Bridge, DistributionServer as Server};
#[path = "distribution_lifetime.rs"]
mod distribution_lifetime;
#[path = "source_https/mod.rs"]
mod source_https;

fn delivery_client(origin: &str, root: &PathBuf) -> FileChunkClient {
    FileChunkClient::with_config(
        origin,
        ChunkStaging::open(scope(), root, 1).unwrap(),
        FileChunkClientConfig {
            transport: FileChunkTransport::LoopbackHttp,
            capacity: 1,
            ..FileChunkClientConfig::default()
        },
    )
    .unwrap()
}
fn identity(path: &PathBuf) -> (u64, u64, u32, u32, u32) {
    let value = fs::metadata(path).unwrap();
    (
        value.dev(),
        value.ino(),
        value.uid(),
        value.mode(),
        value.gid(),
    )
}

async fn ended_observer(progress: &mut FileDeliveryProgress) {
    // No event queue: at most one final unseen snapshot precedes closure after the call returns.
    let tail = tokio::time::timeout(Duration::from_secs(3), progress.changed())
        .await
        .unwrap();
    if tail.is_some() {
        assert_eq!(tail, progress.latest());
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(3), progress.changed())
                .await
                .unwrap(),
            None
        );
    }
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL and loopback TCP"]
async fn postgres_delivery_fetches_authorized_large_and_empty_bytes_and_saves_new_selected_files() {
    checked(chunk_fixture::Fixture::ready(1).await, |f| {
        Box::pin(async move {
            let large = publish(f, 2, FILE_TRANSFER_CHUNK_BYTES + 17).await;
            let empty = publish(f, 3, 0).await;
            let downloads = roots(f, ["delivery-large", "delivery-large-user755", "delivery-large-user750",
                "delivery-empty", "delivery-empty-user755", "delivery-empty-user750"]);
            let outputs = roots(f, ["selected-delivery-large", "selected-large-user755", "selected-large-user750",
                "selected-delivery-empty", "selected-empty-user755", "selected-empty-user750"]);
            let bridge = Bridge::new(f, downloads[0].clone());
            let server = Server::start(bridge.clone()).await;
            let outcome = AssertUnwindSafe(async {
                let original_server = f.sharing.snapshot().await;
                for (position, manifest) in [&large, &empty].into_iter().enumerate() {
                  for (variant, bits) in [0o700, 0o755, 0o750].into_iter().enumerate() {
                    let slot = position * 3 + variant;
                    bridge.set_root(downloads[slot].clone());
                    fs::set_permissions(&outputs[slot], fs::Permissions::from_mode(bits)).unwrap();
                    let source_parent = identity(&downloads[slot]);
                    let selected_parent = identity(&outputs[slot]);
                    assert_eq!(source_parent.3 & 0o7777, 0o700);
                    assert_eq!(selected_parent.3 & 0o7777, bits);
                    let target = if variant == 0 {
                        FileSaveTarget::open(&outputs[slot], "chosen-delivery.bin").unwrap()
                    } else {
                        assert_eq!(FileSaveTarget::open(&outputs[slot], "chosen-delivery.bin").err(),
                            Some(cyanrex_engine::services::file_transfer::staging::FileSaveError::Unavailable));
                        FileSaveTarget::open_user_directory(&outputs[slot], "chosen-delivery.bin").unwrap()
                    };
                    let client = delivery_client(&server.origin, &downloads[slot]);
                    let seed = position as u128 + 2;
                    let (observer, mut progress) = FileDeliveryObserver::channel();
                    assert_eq!(progress.latest(), None);
                    let saved = tokio::time::timeout(
                        Duration::from_secs(20),
                        async {
                            let expected = FileVersionRef::from_manifest(manifest);
                            if variant == 0 {
                                drop(observer);
                                client.fetch_granted_and_save(
                                    &f.sharing.base.grants.auth.target_token,
                                    sharing_fixture::recipient_grant(seed),
                                    publication_fixture::upload(seed), &expected, &target,
                                ).await
                            } else if variant == 1 {
                                client.fetch_granted_and_save_observed(
                                    &f.sharing.base.grants.auth.target_token,
                                    sharing_fixture::recipient_grant(seed),
                                    publication_fixture::upload(seed), &expected, &target, observer,
                                ).await
                            } else {
                                let (handle, cancellation) = FileDeliveryCancelHandle::channel();
                                let result = client.fetch_granted_and_save_controlled(
                                    &f.sharing.base.grants.auth.target_token,
                                    sharing_fixture::recipient_grant(seed),
                                    publication_fixture::upload(seed), &expected, &target, observer, cancellation,
                                ).await;
                                // Late cancellation is a request, not revocation of a selected result.
                                handle.cancel();
                                result.map_err(|error| match error {
                                    FileControlledSaveError::Delivery(error) => error,
                                    FileControlledSaveError::Cancelled => panic!("unused controlled handle must not cancel actual Source delivery"),
                                })
                            }
                        },
                    )
                    .await
                    .unwrap()
                    .unwrap();
                    assert_eq!(saved.file(), &FileVersionRef::from_manifest(manifest));
                    if variant == 0 {
                        assert_eq!(progress.latest(), None, "the unconnected observer stays inert for the old-API control");
                    } else {
                        let observed = progress.latest().unwrap();
                        assert_eq!(observed.phase(), FileDeliveryPhase::Complete);
                        assert_eq!(observed.staged_chunks(), manifest.chunk_sha256().len() as u32);
                        assert_eq!(observed.staged_bytes(), manifest.byte_length());
                        assert_eq!(observed.total_bytes(), manifest.byte_length());
                        assert_eq!(observed.total_chunks(), Some(manifest.chunk_sha256().len() as u32));
                    }
                    ended_observer(&mut progress).await;
                    assert_private_whole(&downloads[slot], manifest);
                    assert_saved_copy(
                        &outputs[slot],
                        "chosen-delivery.bin",
                        &downloads[slot],
                        manifest,
                    );
                    assert_eq!(identity(&downloads[slot]), source_parent);
                    assert_eq!(identity(&outputs[slot]), selected_parent);
                    let expected = if position == 0 {
                        vec![None, Some(0), Some(1)]
                    } else {
                        vec![None]
                    };
                    let observed = bridge.take_requests();
                    assert_eq!(observed.iter().map(|r| r.0).collect::<Vec<_>>(), expected);
                    assert!(observed[0].1.is_empty());
                    if position == 0 {
                        assert!(observed[1].1.is_empty());
                        assert_eq!(observed[2].1.len(), 1);
                        assert!(bytes::tree(&downloads[slot]).contains(&observed[2].1[0]));
                    }
                    assert_eq!(
                        bridge.take_statuses(),
                        vec![axum::http::StatusCode::OK; expected.len()]
                    );
                    assert_eq!(f.sharing.snapshot().await, original_server);
                  }
                }
            })
            .catch_unwind()
            .await;
            server.stop().await;
            if let Err(panic) = outcome {
                std::panic::resume_unwind(panic);
            }
        })
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL and loopback TCP"]
async fn postgres_delivery_revocation_after_manifest_and_revoked_empty_metadata_never_save_targets()
{
    checked(chunk_fixture::Fixture::ready(1).await, |f| Box::pin(async move {
        let empty = publish(f, 3, 0).await;
        let downloads = roots(f, ["delivery-revoked-chunk", "delivery-revoked-empty"]);
        let outputs = roots(f, ["selected-revoked-chunk", "selected-revoked-empty"]);
        for (parent, bits) in outputs.iter().zip([0o755, 0o750]) {
            fs::set_permissions(parent, fs::Permissions::from_mode(bits)).unwrap();
        }
        let bridge = Bridge::new(f, downloads[0].clone());
        let pause = Arc::new(tokio::sync::Semaphore::new(0));
        *bridge.pause.lock().unwrap() = Some(pause.clone());
        let server = Server::start(bridge.clone()).await;
        let outcome = AssertUnwindSafe(async {
            let manifest = &f.sharing.base.manifest;
            let client = delivery_client(&server.origin, &downloads[0]);
            let target = FileSaveTarget::open_user_directory(&outputs[0], "not-created.bin").unwrap();
            let expected = FileVersionRef::from_manifest(manifest);
            let before = f.sharing.snapshot().await;
            let (observer, mut progress) = FileDeliveryObserver::channel();
            let (handle, cancellation) = FileDeliveryCancelHandle::channel();
            let future = client.fetch_granted_and_save_controlled(&f.sharing.base.grants.auth.target_token,
                sharing_fixture::recipient_grant(1), publication_fixture::upload(1), &expected, &target, observer, cancellation);
            tokio::pin!(future);
            tokio::select! {
                result = &mut future => panic!("completed before actual chunk request barrier: {:?}", result.err()),
                waited = tokio::time::timeout(Duration::from_secs(20), async {
                    loop {
                        if bridge.observed.lock().unwrap().len() == 2 { break; }
                        tokio::task::yield_now().await;
                    }
                }) => waited.unwrap(),
            }
            assert!(bytes::tree(&downloads[0]).is_empty());
            assert!(bytes::tree(&outputs[0]).is_empty());
            let pre_chunk = progress.latest().unwrap();
            assert_eq!(pre_chunk.phase(), FileDeliveryPhase::StagingChunks);
            assert_eq!((pre_chunk.staged_chunks(), pre_chunk.staged_bytes()), (0, 0));
            assert_eq!(pre_chunk.total_bytes(), manifest.byte_length());
            assert_eq!(pre_chunk.total_chunks(), Some(manifest.chunk_sha256().len() as u32));
            assert_eq!(f.sharing.snapshot().await, before);
            f.sharing.store.revoke(sharing_fixture::recipient_grant(1), f.sharing.actor(),
                sharing_fixture::revision(1)).await.unwrap();
            let revoked = f.sharing.snapshot().await;
            sharing_fixture::recipient_delta(&f.sharing.schema, &before, &revoked, 1, true);
            pause.add_permits(1);
            assert_eq!(tokio::time::timeout(Duration::from_secs(20), &mut future).await.unwrap().err(),
                Some(FileControlledSaveError::Delivery(FileGrantedSaveError::Download(FileGrantedDownloadError::Download(
                    FileDownloadError::Chunk { index: 0, error: FileChunkClientError::Receive(
                        FileChunkReceiveError::InvalidResponse) })))));
            drop(handle);
            assert_eq!(progress.latest(), Some(pre_chunk), "rejection emits no invented chunk/save ACK");
            ended_observer(&mut progress).await;
            assert_eq!(bridge.take_statuses(), [axum::http::StatusCode::OK, axum::http::StatusCode::NOT_FOUND]);
            assert_eq!(bridge.take_requests().iter().map(|r| r.0).collect::<Vec<_>>(), [None, Some(0)]);
            assert!(bytes::tree(&downloads[0]).is_empty());
            assert!(bytes::tree(&outputs[0]).is_empty());
            assert_eq!(f.sharing.snapshot().await, revoked);
            *bridge.pause.lock().unwrap() = None;
            bridge.set_root(downloads[1].clone());
            let before = f.sharing.snapshot().await;
            f.sharing.store.revoke(sharing_fixture::recipient_grant(3), f.sharing.actor(),
                sharing_fixture::revision(1)).await.unwrap();
            let revoked = f.sharing.snapshot().await;
            sharing_fixture::recipient_delta(&f.sharing.schema, &before, &revoked, 3, true);
            let client = delivery_client(&server.origin, &downloads[1]);
            let target = FileSaveTarget::open_user_directory(&outputs[1], "empty-not-created.bin").unwrap();
            let (observer, mut progress) = FileDeliveryObserver::channel();
            let (handle, cancellation) = FileDeliveryCancelHandle::channel();
            // Dropping an unused handle cannot turn real authorization rejection into Cancelled.
            drop(handle);
            assert_eq!(tokio::time::timeout(Duration::from_secs(20),
                client.fetch_granted_and_save_controlled(&f.sharing.base.grants.auth.target_token,
                    sharing_fixture::recipient_grant(3), publication_fixture::upload(3),
                    &FileVersionRef::from_manifest(&empty), &target, observer, cancellation)).await.unwrap().err(),
                Some(FileControlledSaveError::Delivery(FileGrantedSaveError::Download(FileGrantedDownloadError::Manifest(
                    FileManifestClientError::InvalidResponse)))));
            let rejected_empty = progress.latest().unwrap();
            assert_eq!(rejected_empty.phase(), FileDeliveryPhase::FetchingManifest);
            assert_eq!((rejected_empty.staged_chunks(), rejected_empty.staged_bytes(), rejected_empty.total_bytes()), (0, 0, 0));
            assert_eq!(rejected_empty.total_chunks(), None);
            ended_observer(&mut progress).await;
            assert_eq!(bridge.take_statuses(), [axum::http::StatusCode::NOT_FOUND]);
            assert_eq!(bridge.take_requests().iter().map(|r| r.0).collect::<Vec<_>>(), [None]);
            assert!(bytes::tree(&downloads[1]).is_empty());
            assert!(bytes::tree(&outputs[1]).is_empty());
            assert_eq!(f.sharing.snapshot().await, revoked);
        }).catch_unwind().await;
        pause.add_permits(8);
        server.stop().await;
        if let Err(panic) = outcome { std::panic::resume_unwind(panic); }
    })).await;
}
