//! The one real current-Source bridge; nested pure fixtures retain their own fixed scope.
use axum::{body::Body, http::Request, Router};
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    platform_http::{build_file_chunk_read_router, FileChunkReadHttpState},
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
        file_chunk_client::{FileChunkClient, FileChunkClientError},
        file_chunk_receiver::FileChunkReceiveError,
        file_publisher_grant_store::{FilePublisherGrantStore, StoredFilePublisherGrant},
        file_recipient_grant_store::FileRecipientGrantStore,
        file_transfer::{staging::ChunkStaging, verify_chunk},
    },
};
use futures_util::FutureExt;
use sha2::{Digest, Sha256};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use std::{
    fs,
    io::Read,
    os::unix::fs::{DirBuilderExt, MetadataExt},
    panic::AssertUnwindSafe,
};
use uuid::Uuid;

#[allow(dead_code)]
#[path = "../durable_auth_source/fixture.rs"]
mod source_fixture;
use source_fixture::{authority, id, otp, PASSWORD};
#[allow(dead_code)]
#[path = "../durable_collaboration/fixture.rs"]
mod identity_fixture;
use identity_fixture::scope;
#[allow(dead_code)]
#[path = "../file_upload_staging/bytes.rs"]
mod bytes;
#[allow(dead_code)]
#[path = "../session_file_chunk_read/fixture.rs"]
mod chunk_fixture;
#[allow(dead_code)]
#[path = "../file_chunk_read_http/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../session_file_publisher_grant/fixture.rs"]
mod grant_fixture;
#[allow(dead_code)]
#[path = "../session_file_publication/fixture.rs"]
mod publication_fixture;
#[allow(dead_code)]
#[path = "../session_file_sharing/fixture.rs"]
mod sharing_fixture;

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL and loopback TCP"]
async fn postgres_client_fetches_actual_authorized_http_bytes_and_stages_without_server_mutations()
{
    let f = fixture::Fixture::ready(1).await;
    let mut server = None;
    let outcome = AssertUnwindSafe(async {
        let root = f.sharing.base.directory.join("network-client-root");
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let staging = ChunkStaging::open(scope(), &root, 1).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let app = fixture::app(&f);
        server = Some(tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        }));
        let client = FileChunkClient::new_loopback(&origin, staging.clone()).unwrap();
        let manifest = &f.sharing.base.manifest;
        let token = &f.sharing.base.grants.auth.target_token;
        let before = f.sharing.snapshot().await;
        let received = client
            .fetch_and_stage(
                token,
                sharing_fixture::recipient_grant(1),
                publication_fixture::upload(1),
                manifest,
                0,
            )
            .await
            .unwrap();
        assert_eq!(received.file(), &FileVersionRef::from_manifest(manifest));
        assert_eq!(received.byte_length(), 17);
        assert_eq!(received.chunk_index(), 0);
        assert_eq!(received.chunk_sha256(), &manifest.chunk(0).unwrap().sha256);
        let path = root.join(format!(
            "v1-{:x}-0",
            Sha256::digest(serde_json::to_vec(manifest).unwrap())
        ));
        let mut file = fs::File::open(&path).unwrap();
        let metadata = file.metadata().unwrap();
        assert_eq!(metadata.mode() & 0o777, 0o400);
        assert_eq!(metadata.len(), 17);
        assert_eq!(metadata.nlink(), 1);
        verify_chunk(manifest, 0, &mut file).unwrap();
        let mut observed = Vec::new();
        fs::File::open(&path)
            .unwrap()
            .take(18)
            .read_to_end(&mut observed)
            .unwrap();
        assert_eq!(observed, (0..17).map(bytes::byte).collect::<Vec<_>>());
        assert_eq!(f.sharing.snapshot().await, before);
        let client_before = bytes::tree(&root);
        f.sharing
            .store
            .revoke(
                sharing_fixture::recipient_grant(1),
                f.sharing.actor(),
                sharing_fixture::revision(1),
            )
            .await
            .unwrap();
        let revoked = f.sharing.snapshot().await;
        assert_eq!(
            client
                .fetch_and_stage(
                    token,
                    sharing_fixture::recipient_grant(1),
                    publication_fixture::upload(1),
                    manifest,
                    0
                )
                .await
                .err(),
            Some(FileChunkClientError::Receive(
                FileChunkReceiveError::InvalidResponse
            ))
        );
        assert_eq!(f.sharing.snapshot().await, revoked);
        assert_eq!(bytes::tree(&root), client_before);
        staging.verify_staged_chunk(manifest, 0).await.unwrap();
    })
    .catch_unwind()
    .await;
    if let Some(task) = server {
        task.abort();
        let _ = task.await;
    }
    f.cleanup().await;
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}
