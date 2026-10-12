use crate::{fixture::*, local::*, *};
use cyanrex_engine::services::file_transfer::staging::ChunkStagingError;
use http_body_util::BodyExt;
use std::{fs, os::unix::fs::PermissionsExt};

async fn actual(f: &Fixture, request: &SessionFileChunkReadRequest) -> axum::response::Response {
    http_fixture::app(&f.server)
        .oneshot(http_fixture::request(
            &f.server.sharing.base.grants.auth.target_token,
            &http_fixture::path(request),
        ))
        .await
        .unwrap()
}
async fn accept(
    f: &Fixture,
    receiver: &FileChunkReceiver,
    manifest: &FileTransferManifest,
    request: &SessionFileChunkReadRequest,
) {
    let before = f.server.sharing.snapshot().await;
    let received = receiver
        .receive_and_stage(manifest, request.chunk_index, actual(f, request).await)
        .await
        .unwrap();
    assert_eq!(received.file(), &request.download.expected_file);
    assert_eq!(received.chunk_index(), request.chunk_index);
    assert_eq!(
        received.byte_length(),
        manifest.chunk(request.chunk_index).unwrap().byte_length
    );
    assert_eq!(
        received.chunk_sha256(),
        &manifest.chunk(request.chunk_index).unwrap().sha256
    );
    let observed = observe_chunk(
        &f.client_root
            .join(legacy_name(manifest, request.chunk_index)),
        manifest,
        request.chunk_index,
    )
    .unwrap();
    observed.verified.unwrap();
    assert_eq!(observed.bytes, data(manifest, request.chunk_index));
    assert_eq!(observed.mode & 0o777, 0o400);
    assert_eq!(observed.links, 1);
    assert_eq!(f.server.sharing.snapshot().await, before);
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_receiver_stages_both_real_cross_chunk_responses_with_exact_client_manifest_names()
{
    checked_sql(Fixture::ready().await, |f| {
        Box::pin(async move {
            let actor = f.server.sharing.actor();
            let manifest = f
                .server
                .sharing
                .base
                .stage_additional(2, FILE_TRANSFER_CHUNK_BYTES + 17, actor)
                .await;
            let mut attempt = f
                .server
                .sharing
                .base
                .publisher
                .prepare(
                    publication_fixture::upload(2),
                    actor.principal(),
                    actor.account_id(),
                )
                .await
                .unwrap();
            let auth = &f.server.sharing.base.grants.auth;
            drop(
                auth.source
                    .publish_session_file_upload(
                        &auth.learner_token,
                        &f.server.sharing.base.workspace,
                        grant_fixture::grant(1),
                        &mut attempt,
                    )
                    .await
                    .unwrap(),
            );
            let mut command = f.server.sharing.command(2);
            command.upload_id = publication_fixture::upload(2);
            command.expected_file = FileVersionRef::from_manifest(&manifest);
            auth.source
                .issue_session_file_recipient_grant(
                    &auth.learner_token,
                    &f.server.sharing.workspace,
                    &command,
                )
                .await
                .unwrap();
            let mut request = SessionFileChunkReadRequest {
                download: SessionFileDownloadRequest {
                    grant_id: command.grant_id,
                    upload_id: command.upload_id,
                    expected_file: command.expected_file,
                },
                chunk_index: 0,
            };
            let receiver = FileChunkReceiver::new(f.client_staging.clone());
            accept(f, &receiver, &manifest, &request).await;
            request.chunk_index = 1;
            accept(f, &receiver, &manifest, &request).await;
            let entries = bytes::tree(&f.client_root);
            assert_eq!(entries.len(), 2);
            for index in 0..2 {
                assert!(entries
                    .iter()
                    .any(|e| e.name == std::ffi::OsString::from(legacy_name(&manifest, index))));
            }
            assert!(
                entries
                    .iter()
                    .all(|e| e.name.to_string_lossy().starts_with("v1-")),
                "client files are not server u1 bindings"
            );
            // A fresh request still obtains the real Source lease after both response bodies were consumed.
            let before = f.server.sharing.snapshot().await;
            let r = actual(f, &request).await;
            assert_eq!(r.status(), StatusCode::OK);
            let frame = r.into_body().collect().await.unwrap().to_bytes();
            assert_eq!(frame, data(&manifest, 1));
            drop(frame);
            assert_eq!(f.server.sharing.snapshot().await, before);
        })
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_receiver_refuses_real_denied_or_corrupt_http_and_existing_client_chunk_without_retaining_server_leases(
) {
    checked_sql(Fixture::ready().await, |f| {
        Box::pin(async move {
            let manifest = f.server.sharing.base.manifest.clone();
            let receiver = FileChunkReceiver::new(f.client_staging.clone());
            let request = f.server.request();
            let mut denied = request.clone();
            denied.download.grant_id = sharing_fixture::recipient_grant(999);
            let before = f.server.sharing.snapshot().await;
            let r = actual(f, &denied).await;
            assert_eq!(r.status(), StatusCode::NOT_FOUND);
            assert_eq!(
                receiver.receive_and_stage(&manifest, 0, r).await.err(),
                Some(FileChunkReceiveError::InvalidResponse)
            );
            assert!(bytes::tree(&f.client_root).is_empty());
            assert_eq!(f.server.sharing.snapshot().await, before);
            let server_path = f.server.sharing.base.root.join(
                bytes::tree(&f.server.sharing.base.root)
                    .into_iter()
                    .find(|e| e.name.to_string_lossy().ends_with("-0"))
                    .unwrap()
                    .name,
            );
            let original = fs::read(&server_path).unwrap();
            fs::set_permissions(&server_path, fs::Permissions::from_mode(0o600)).unwrap();
            let mut corrupt = original.clone();
            corrupt[3] ^= 1;
            fs::write(&server_path, corrupt).unwrap();
            fs::set_permissions(&server_path, fs::Permissions::from_mode(0o400)).unwrap();
            let injected = f.server.sharing.snapshot().await;
            let r = actual(f, &request).await;
            assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(
                receiver.receive_and_stage(&manifest, 0, r).await.err(),
                Some(FileChunkReceiveError::InvalidResponse)
            );
            assert!(bytes::tree(&f.client_root).is_empty());
            assert_eq!(f.server.sharing.snapshot().await, injected);
            fs::set_permissions(&server_path, fs::Permissions::from_mode(0o600)).unwrap();
            fs::write(&server_path, original).unwrap();
            fs::set_permissions(&server_path, fs::Permissions::from_mode(0o400)).unwrap();
            accept(f, &receiver, &manifest, &request).await;
            let before = f.server.sharing.snapshot().await;
            let client_before = bytes::tree(&f.client_root);
            let r = actual(f, &request).await;
            assert_eq!(r.status(), StatusCode::OK);
            assert_eq!(
                receiver.receive_and_stage(&manifest, 0, r).await.err(),
                Some(FileChunkReceiveError::Staging(ChunkStagingError::Conflict))
            );
            assert_eq!(bytes::tree(&f.client_root), client_before);
            assert_eq!(f.server.sharing.snapshot().await, before);
            f.client_staging
                .verify_staged_chunk(&manifest, 0)
                .await
                .unwrap();
            // Rejected input and output conflict must drop the actual response owner, not strand Source capacity.
            let r = actual(f, &request).await;
            assert_eq!(r.status(), StatusCode::OK);
            let frame = r.into_body().collect().await.unwrap().to_bytes();
            assert_eq!(frame, data(&manifest, 0));
            drop(frame);
            assert_eq!(f.server.sharing.snapshot().await, before);
            assert_eq!(bytes::tree(&f.client_root), client_before);
        })
    })
    .await;
}
