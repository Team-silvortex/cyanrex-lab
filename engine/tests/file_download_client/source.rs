//! Real Source authorization per chunk; pure nested fixtures keep their separate fixed scope.
use axum::{
    body::Body,
    extract::{Query, State},
    http::Request,
    Router,
};
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
        file_chunk_client::{FileChunkClient, FileChunkClientError, FileDownloadError},
        file_chunk_receiver::FileChunkReceiveError,
        file_publisher_grant_store::{FilePublisherGrantStore, StoredFilePublisherGrant},
        file_recipient_grant_store::FileRecipientGrantStore,
        file_transfer::{
            staging::ChunkStaging, verify_chunk, verify_file, FILE_TRANSFER_BUFFER_BYTES,
        },
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
    path::{Path, PathBuf},
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

#[derive(Clone)]
struct Bridge {
    source: Router,
    statuses: Arc<Mutex<Vec<axum::http::StatusCode>>>,
    observed: Arc<Mutex<Vec<(super::fixture::RequestQuery, Vec<bytes::Entry>)>>>,
    root: Arc<Mutex<PathBuf>>,
    pause: Arc<Mutex<Option<Arc<tokio::sync::Semaphore>>>>,
}
async fn forward(State(bridge): State<Bridge>, request: Request<Body>) -> axum::response::Response {
    let selected = Query::<super::fixture::RequestQuery>::try_from_uri(request.uri())
        .unwrap()
        .0;
    assert_eq!(request.method(), "GET");
    assert_eq!(request.uri().path(), fixture::ENDPOINT);
    assert!(!request.headers().contains_key("origin"));
    let pause = if selected.chunk_index == 1 {
        bridge.pause.lock().unwrap().clone()
    } else {
        None
    };
    let tree = bytes::tree(&bridge.root.lock().unwrap());
    bridge.observed.lock().unwrap().push((selected, tree));
    if let Some(pause) = pause {
        pause.acquire().await.unwrap().forget();
    }
    let response = bridge.source.oneshot(request).await.unwrap();
    bridge.statuses.lock().unwrap().push(response.status());
    response
}
fn chunk_name(manifest: &FileTransferManifest, index: u32) -> String {
    format!(
        "v1-{:x}-{index}",
        Sha256::digest(serde_json::to_vec(manifest).unwrap())
    )
}
fn output_name(manifest: &FileTransferManifest) -> String {
    format!(
        "f1-{:x}",
        Sha256::digest(serde_json::to_vec(manifest).unwrap())
    )
}
fn assert_private_whole(root: &Path, manifest: &FileTransferManifest) {
    let path = root.join(output_name(manifest));
    let mut file = fs::File::open(&path).unwrap();
    let opened = file.metadata().unwrap();
    let visible = fs::symlink_metadata(&path).unwrap();
    assert_eq!((opened.dev(), opened.ino()), (visible.dev(), visible.ino()));
    assert_eq!(opened.uid(), fs::metadata(root).unwrap().uid());
    assert_eq!(opened.mode() & 0o7777, 0o400);
    assert_eq!(opened.nlink(), 1);
    assert_eq!(opened.len(), manifest.byte_length());
    verify_file(manifest, &mut file).unwrap();
    let mut file = fs::File::open(&path).unwrap();
    let mut buffer = [0; FILE_TRANSFER_BUFFER_BYTES];
    let mut offset = 0;
    let mut hash = Sha256::new();
    loop {
        let amount = file.read(&mut buffer).unwrap();
        if amount == 0 {
            break;
        }
        for (index, value) in buffer[..amount].iter().enumerate() {
            assert_eq!(*value, bytes::byte(offset + index as u64));
        }
        hash.update(&buffer[..amount]);
        offset += amount as u64;
    }
    assert_eq!(offset, manifest.byte_length());
    assert_eq!(
        format!("{:x}", hash.finalize()),
        manifest.sha256().to_string()
    );
    for index in 0..2 {
        let mut file = fs::File::open(root.join(chunk_name(manifest, index))).unwrap();
        let metadata = file.metadata().unwrap();
        assert_eq!(metadata.mode() & 0o7777, 0o400);
        assert_eq!(metadata.nlink(), 1);
        assert_eq!(metadata.uid(), opened.uid());
        verify_chunk(manifest, index, &mut file).unwrap();
    }
    assert_eq!(bytes::tree(root).len(), 3);
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL and loopback TCP"]
async fn postgres_download_assembles_real_published_bytes_and_stops_at_later_revoked_grant() {
    let f = fixture::Fixture::ready(1).await;
    let mut server = None;
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let pause = Arc::new(tokio::sync::Semaphore::new(0));
    let outcome = AssertUnwindSafe(async {
        let actor = f.sharing.actor();
        let manifest = f.sharing.base.stage_additional(2, FILE_TRANSFER_CHUNK_BYTES + 17, actor).await;
        let mut attempt = f.sharing.base.publisher.prepare(publication_fixture::upload(2),
            actor.principal(), actor.account_id()).await.unwrap();
        let source = &f.sharing.base.grants.auth.source;
        let publisher_token = &f.sharing.base.grants.auth.learner_token;
        source.publish_session_file_upload(publisher_token, &f.sharing.base.workspace,
            grant_fixture::grant(1), &mut attempt).await.unwrap();
        source.issue_session_file_recipient_grant(publisher_token, &f.sharing.workspace,
            &SessionFileRecipientGrantIssue {
                grant_id: sharing_fixture::recipient_grant(2),
                publisher_grant_id: grant_fixture::grant(1), upload_id: publication_fixture::upload(2),
                expected_file: FileVersionRef::from_manifest(&manifest), recipient: f.sharing.recipient,
                validity: grant_fixture::validity(),
            }).await.unwrap();
        let roots = ["download-success", "download-revoked"].map(|name| f.sharing.base.directory.join(name));
        for root in &roots { fs::DirBuilder::new().mode(0o700).create(root).unwrap(); }
        let bridge = Bridge {
            source: fixture::app(&f), observed: Arc::new(Mutex::new(Vec::new())),
            statuses: Arc::new(Mutex::new(Vec::new())),
            root: Arc::new(Mutex::new(roots[0].clone())), pause: Arc::new(Mutex::new(None)),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new().fallback(forward).with_state(bridge.clone());
        server = Some(tokio::spawn(async move {
            axum::serve(listener, app).with_graceful_shutdown(async { let _ = stopped.await; }).await.unwrap();
        }));
        let token = &f.sharing.base.grants.auth.target_token;
        let before = f.sharing.snapshot().await;
        let staging = ChunkStaging::open(scope(), &roots[0], 1).unwrap();
        let root_metadata = fs::metadata(&roots[0]).unwrap();
        let client = FileChunkClient::new_loopback(&origin, staging.clone()).unwrap();
        let assembled = tokio::time::timeout(Duration::from_secs(20), client.fetch_and_assemble(
            token, sharing_fixture::recipient_grant(2), publication_fixture::upload(2), &manifest
        )).await.unwrap().unwrap();
        assert_eq!(assembled.file(), &FileVersionRef::from_manifest(&manifest));
        assert_private_whole(&roots[0], &manifest);
        let current_root = fs::metadata(&roots[0]).unwrap();
        assert_eq!((root_metadata.dev(), root_metadata.ino(), root_metadata.uid(), root_metadata.mode()),
            (current_root.dev(), current_root.ino(), current_root.uid(), current_root.mode()));
        let requests = std::mem::take(&mut *bridge.observed.lock().unwrap());
        assert_eq!(requests.iter().map(|r| r.0.chunk_index).collect::<Vec<_>>(), [0,1]);
        assert!(requests[0].1.is_empty());
        assert_eq!(requests[1].1.len(), 1);
        assert!(bytes::tree(&roots[0]).contains(&requests[1].1[0]));
        assert_eq!(std::mem::take(&mut *bridge.statuses.lock().unwrap()),
            [axum::http::StatusCode::OK, axum::http::StatusCode::OK]);
        assert_eq!(f.sharing.snapshot().await, before);
        drop(client);

        *bridge.root.lock().unwrap() = roots[1].clone();
        *bridge.pause.lock().unwrap() = Some(pause.clone());
        let staging = ChunkStaging::open(scope(), &roots[1], 1).unwrap();
        let client = FileChunkClient::new_loopback(&origin, staging.clone()).unwrap();
        let future = client.fetch_and_assemble(token, sharing_fixture::recipient_grant(2),
            publication_fixture::upload(2), &manifest);
        tokio::pin!(future);
        let wait_second = async {
            loop {
                if bridge.observed.lock().unwrap().len() == 2 { break; }
                tokio::task::yield_now().await;
            }
        };
        tokio::select! {
            result = &mut future => panic!("download completed before the real second-request barrier: {:?}", result.err()),
            waited = tokio::time::timeout(Duration::from_secs(20), wait_second) => waited.unwrap(),
        }
        let saved = bytes::tree(&roots[1]);
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].name, std::ffi::OsString::from(chunk_name(&manifest, 0)));
        staging.verify_staged_chunk(&manifest, 0).await.unwrap();
        assert_eq!(f.sharing.snapshot().await, before);
        // This explicit trusted test-admin revoke is not a new remote mutation or issuer API.
        f.sharing.store.revoke(sharing_fixture::recipient_grant(2), actor,
            sharing_fixture::revision(1)).await.unwrap();
        let revoked = f.sharing.snapshot().await;
        sharing_fixture::recipient_delta(&f.sharing.schema, &before, &revoked, 2, true);
        pause.add_permits(1);
        assert_eq!(tokio::time::timeout(Duration::from_secs(20), &mut future).await.unwrap().err(),
            Some(FileDownloadError::Chunk { index: 1,
                error: FileChunkClientError::Receive(FileChunkReceiveError::InvalidResponse) }));
        assert_eq!(bytes::tree(&roots[1]), saved);
        assert!(!roots[1].join(output_name(&manifest)).exists());
        let requests = std::mem::take(&mut *bridge.observed.lock().unwrap());
        assert_eq!(requests.iter().map(|r| r.0.chunk_index).collect::<Vec<_>>(), [0,1]);
        assert!(requests[0].1.is_empty());
        assert_eq!(requests[1].1, saved);
        assert_eq!(std::mem::take(&mut *bridge.statuses.lock().unwrap()),
            [axum::http::StatusCode::OK, axum::http::StatusCode::NOT_FOUND],
            "actual revoked authorization must be private 404, not generic service failure");
        assert_eq!(f.sharing.snapshot().await, revoked);
        staging.verify_staged_chunk(&manifest, 0).await.unwrap();
    }).catch_unwind().await;
    pause.add_permits(16);
    let _ = stop.send(());
    let joined = if let Some(mut task) = server {
        let joined = tokio::time::timeout(Duration::from_secs(5), &mut task).await;
        if joined.is_err() {
            task.abort();
            let _ = task.await;
        }
        Some(joined)
    } else {
        None
    };
    f.cleanup().await;
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
    joined
        .expect("the actual Source bridge must have started")
        .expect("drain and await this fixture's own server")
        .expect("fixture server must exit without task panic");
}
