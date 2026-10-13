//! Current Source over actual loopback HTTP; nested fixtures keep their own trusted scope.
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
            FileChunkClient, FileChunkClientError, FileDownloadError, FileGrantedDownloadError,
            FileManifestClientError,
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
    os::unix::fs::{DirBuilderExt, MetadataExt},
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
#[path = "source_support.rs"]
mod support;
use support::*;

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL and loopback TCP"]
async fn postgres_granted_client_fetches_real_manifest_and_assembles_published_large_and_empty_files(
) {
    checked(chunk_fixture::Fixture::ready(1).await, |f| {
        Box::pin(async move {
            let large = publish(f, 2, FILE_TRANSFER_CHUNK_BYTES + 17).await;
            let empty = publish(f, 3, 0).await;
            let roots = roots(f, ["manifest-large", "manifest-empty"]);
            let saved_roots = support::roots(f, ["selected-large", "selected-empty"]);
            let bridge = Bridge::new(f, roots[0].clone());
            let server = Server::start(bridge.clone()).await;
            let outcome = AssertUnwindSafe(async {
                let before = f.sharing.snapshot().await;
                for (position, manifest) in [&large, &empty].into_iter().enumerate() {
                    bridge.set_root(roots[position].clone());
                    let original = fs::metadata(&roots[position]).unwrap();
                    let staging = ChunkStaging::open(scope(), &roots[position], 1).unwrap();
                    let client =
                        FileChunkClient::new_loopback(&server.origin, staging.clone()).unwrap();
                    let seed = position as u128 + 2;
                    let result = tokio::time::timeout(
                        Duration::from_secs(20),
                        client.fetch_granted_and_assemble(
                            &f.sharing.base.grants.auth.target_token,
                            sharing_fixture::recipient_grant(seed),
                            publication_fixture::upload(seed),
                            &FileVersionRef::from_manifest(manifest),
                        ),
                    )
                    .await
                    .unwrap();
                    let assembled = result.unwrap();
                    assert_eq!(assembled.file(), &FileVersionRef::from_manifest(manifest));
                    assert_private_whole(&roots[position], manifest);
                    let local_before = bytes::tree(&roots[position]);
                    let source_root_before = bytes::snapshot(&roots[position]);
                    let selected_parent = fs::metadata(&saved_roots[position]).unwrap();
                    let target =
                        FileSaveTarget::open(&saved_roots[position], "host-selected.bin").unwrap();
                    let saved = tokio::time::timeout(
                        Duration::from_secs(20),
                        staging.save_assembled_file(manifest, &assembled, &target),
                    )
                    .await
                    .unwrap()
                    .unwrap();
                    assert_eq!(saved.file(), &FileVersionRef::from_manifest(manifest));
                    assert_saved_copy(
                        &saved_roots[position],
                        "host-selected.bin",
                        &roots[position],
                        manifest,
                    );
                    assert_eq!(bytes::tree(&roots[position]), local_before);
                    assert_eq!(bytes::snapshot(&roots[position]), source_root_before);
                    let selected_after = fs::metadata(&saved_roots[position]).unwrap();
                    assert_eq!(
                        (
                            selected_parent.dev(),
                            selected_parent.ino(),
                            selected_parent.uid(),
                            selected_parent.mode()
                        ),
                        (
                            selected_after.dev(),
                            selected_after.ino(),
                            selected_after.uid(),
                            selected_after.mode()
                        )
                    );
                    let current = fs::metadata(&roots[position]).unwrap();
                    assert_eq!(
                        (
                            original.dev(),
                            original.ino(),
                            original.uid(),
                            original.mode()
                        ),
                        (current.dev(), current.ino(), current.uid(), current.mode())
                    );
                    let requests = bridge.take_requests();
                    let expected = if position == 0 {
                        vec![None, Some(0), Some(1)]
                    } else {
                        vec![None]
                    };
                    assert_eq!(requests.iter().map(|r| r.0).collect::<Vec<_>>(), expected);
                    assert!(requests[0].1.is_empty());
                    if position == 0 {
                        assert!(requests[1].1.is_empty());
                        assert_eq!(requests[2].1.len(), 1);
                        assert!(bytes::tree(&roots[position]).contains(&requests[2].1[0]));
                    }
                    assert_eq!(
                        bridge.take_statuses(),
                        vec![axum::http::StatusCode::OK; expected.len()]
                    );
                    assert_eq!(f.sharing.snapshot().await, before);
                    drop(client);
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
async fn postgres_granted_client_refuses_revocation_between_manifest_and_chunk_and_revoked_empty_metadata(
) {
    checked(chunk_fixture::Fixture::ready(1).await, |f| Box::pin(async move {
        let empty = publish(f, 3, 0).await;
        let roots = roots(f, ["manifest-revoked-chunk", "manifest-revoked-empty"]);
        let bridge = Bridge::new(f, roots[0].clone());
        let pause = Arc::new(tokio::sync::Semaphore::new(0));
        *bridge.pause.lock().unwrap() = Some(pause.clone());
        let server = Server::start(bridge.clone()).await;
        let outcome = AssertUnwindSafe(async {
            let manifest = &f.sharing.base.manifest;
            let before = f.sharing.snapshot().await;
            let client = FileChunkClient::new_loopback(&server.origin,
                ChunkStaging::open(scope(), &roots[0], 1).unwrap()).unwrap();
            let expected_file = FileVersionRef::from_manifest(manifest);
            let future = client.fetch_granted_and_assemble(&f.sharing.base.grants.auth.target_token,
                sharing_fixture::recipient_grant(1), publication_fixture::upload(1),
                &expected_file);
            tokio::pin!(future);
            tokio::select! {
                result = &mut future => panic!("completed before the actual chunk request barrier: {:?}", result.err()),
                waited = tokio::time::timeout(Duration::from_secs(20), async {
                    loop {
                        if bridge.observed.lock().unwrap().len() == 2 { break; }
                        tokio::task::yield_now().await;
                    }
                }) => waited.unwrap(),
            }
            assert!(bytes::tree(&roots[0]).is_empty());
            assert_eq!(f.sharing.snapshot().await, before);
            f.sharing.store.revoke(sharing_fixture::recipient_grant(1), f.sharing.actor(),
                sharing_fixture::revision(1)).await.unwrap();
            let revoked = f.sharing.snapshot().await;
            sharing_fixture::recipient_delta(&f.sharing.schema, &before, &revoked, 1, true);
            pause.add_permits(1);
            assert_eq!(tokio::time::timeout(Duration::from_secs(20), &mut future).await.unwrap().err(),
                Some(FileGrantedDownloadError::Download(FileDownloadError::Chunk { index: 0,
                    error: FileChunkClientError::Receive(FileChunkReceiveError::InvalidResponse) })));
            assert_eq!(bridge.take_statuses(), [axum::http::StatusCode::OK, axum::http::StatusCode::NOT_FOUND]);
            assert_eq!(bridge.take_requests().iter().map(|r| r.0).collect::<Vec<_>>(), [None, Some(0)]);
            assert!(bytes::tree(&roots[0]).is_empty());
            assert_eq!(f.sharing.snapshot().await, revoked);

            *bridge.pause.lock().unwrap() = None;
            bridge.set_root(roots[1].clone());
            let before = f.sharing.snapshot().await;
            f.sharing.store.revoke(sharing_fixture::recipient_grant(3), f.sharing.actor(),
                sharing_fixture::revision(1)).await.unwrap();
            let revoked = f.sharing.snapshot().await;
            sharing_fixture::recipient_delta(&f.sharing.schema, &before, &revoked, 3, true);
            let client = FileChunkClient::new_loopback(&server.origin,
                ChunkStaging::open(scope(), &roots[1], 1).unwrap()).unwrap();
            assert_eq!(tokio::time::timeout(Duration::from_secs(20), client.fetch_granted_and_assemble(
                &f.sharing.base.grants.auth.target_token, sharing_fixture::recipient_grant(3),
                publication_fixture::upload(3), &FileVersionRef::from_manifest(&empty)
            )).await.unwrap().err(), Some(FileGrantedDownloadError::Manifest(FileManifestClientError::InvalidResponse)));
            assert_eq!(bridge.take_statuses(), [axum::http::StatusCode::NOT_FOUND]);
            assert_eq!(bridge.take_requests().iter().map(|r| r.0).collect::<Vec<_>>(), [None]);
            assert!(bytes::tree(&roots[1]).is_empty(), "denied empty metadata must not create an empty assembly");
            assert_eq!(f.sharing.snapshot().await, revoked);
        }).catch_unwind().await;
        pause.add_permits(16);
        server.stop().await;
        if let Err(panic) = outcome { std::panic::resume_unwind(panic); }
    })).await;
}
