use crate::{fixture::*, support::*, *};
use axum::body::Bytes;
use std::{fs, os::unix::fs::PermissionsExt, sync::Arc};

#[tokio::test]
async fn later_metadata_or_body_digest_fault_stops_without_retry_assembly_or_touching_confirmed_chunk(
) {
    for corrupt_body in [false, true] {
        checked(
            Fixture::new(2 * FILE_TRANSFER_CHUNK_BYTES + 17).await,
            |f| {
                Box::pin(async move {
                    let client = f.client(1);
                    let mut reply = Reply::good(&f.manifest, 1);
                    if corrupt_body {
                        let mut data = reply.pieces[0].data.to_vec();
                        data[7] ^= 1;
                        reply.pieces[0].data = Bytes::from(data);
                    } else {
                        reply
                            .headers
                            .insert("x-cyanrex-chunk-index", "0".parse().unwrap());
                    }
                    f.set_reply(1, reply);
                    let error = client
                        .fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest)
                        .await
                        .err()
                        .unwrap();
                    assert_eq!(error, invalid_response(1));
                    redacted(f, &error);
                    let requests = f.take_requests();
                    first_preserved(f, &requests);
                    assert!(!f.root.join(chunk_name(&f.manifest, 2)).exists());
                })
            },
        )
        .await;
    }
}

#[tokio::test]
async fn later_actual_header_timeout_preserves_first_chunk_and_reports_selected_index() {
    checked(
        Fixture::new(2 * FILE_TRANSFER_CHUNK_BYTES + 17).await,
        |f| {
            Box::pin(async move {
                let gate = Arc::new(tokio::sync::Semaphore::new(0));
                let mut reply = Reply::good(&f.manifest, 1);
                reply.header_gate = Some(gate);
                f.set_reply(1, reply);
                let client = f.client(1);
                let mut pending =
                    Box::pin(client.fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest));
                drive_to_request(f, 2, &mut pending).await;
                let error = pending.await.err().unwrap();
                assert_eq!(error, chunk_error(1, FileChunkClientError::TimedOut));
                redacted(f, &error);
                first_preserved(f, &f.take_requests());
                assert!(!f.root.join(chunk_name(&f.manifest, 2)).exists());
            })
        },
    )
    .await;
}

#[tokio::test]
async fn preexisting_first_chunk_and_completed_repeat_fail_at_first_chunk_without_adopting_bytes() {
    checked(Fixture::new(FILE_TRANSFER_CHUNK_BYTES + 17).await, |f| {
        Box::pin(async move {
            let spec = f.manifest.chunk(0).unwrap();
            f.staging
                .stage_chunk(
                    &f.manifest,
                    0,
                    bytes::Pattern::new(spec.offset, spec.byte_length),
                )
                .await
                .unwrap();
            let before = bytes::tree(&f.root);
            let client = f.client(1);
            assert_eq!(
                client
                    .fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest)
                    .await
                    .err(),
                Some(staging_error(0, ChunkStagingError::Conflict))
            );
            assert_eq!(request_indices(&f.take_requests()), [0]);
            assert_eq!(bytes::tree(&f.root), before);
            assert!(!f.root.join(output_name(&f.manifest)).exists());
        })
    })
    .await;
    checked(Fixture::new(FILE_TRANSFER_CHUNK_BYTES + 17).await, |f| {
        Box::pin(async move {
            let client = f.client(1);
            client
                .fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest)
                .await
                .unwrap();
            assert_eq!(request_indices(&f.take_requests()), [0, 1]);
            let before = bytes::tree(&f.root);
            assert_eq!(
                client
                    .clone()
                    .fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest)
                    .await
                    .err(),
                Some(staging_error(0, ChunkStagingError::Conflict))
            );
            assert_eq!(request_indices(&f.take_requests()), [0]);
            assert_eq!(
                bytes::tree(&f.root),
                before,
                "repeat never skips existing chunks or replaces the whole output"
            );
        })
    })
    .await;
}

#[tokio::test]
async fn correct_individual_http_chunks_with_wrong_whole_digest_fail_before_output_creation() {
    let manifest = changed(
        &bytes::manifest(FILE_TRANSFER_CHUNK_BYTES + 17, 1),
        "sha256",
        serde_json::json!("a".repeat(64)),
    );
    checked(Fixture::with_manifest(manifest).await, |f| {
        Box::pin(async move {
            let client = f.client(1);
            assert_eq!(
                client
                    .fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest)
                    .await
                    .err(),
                Some(FileDownloadError::Assembly(ChunkStagingError::Unavailable))
            );
            assert_eq!(request_indices(&f.take_requests()), [0, 1]);
            assert_eq!(bytes::tree(&f.root).len(), 2);
            for index in [0, 1] {
                let mut chunk =
                    fs::File::open(f.root.join(chunk_name(&f.manifest, index))).unwrap();
                verify_chunk(&f.manifest, index, &mut chunk).unwrap();
            }
            assert!(!f.root.join(output_name(&f.manifest)).exists());
        })
    })
    .await;
}

#[tokio::test]
async fn preexisting_whole_output_conflicts_after_all_chunk_requests_without_overwriting_output() {
    checked(Fixture::new(FILE_TRANSFER_CHUNK_BYTES + 17).await, |f| {
        Box::pin(async move {
            let output = f.root.join(output_name(&f.manifest));
            fs::write(&output, b"preexisting inert output").unwrap();
            fs::set_permissions(&output, fs::Permissions::from_mode(0o400)).unwrap();
            let original = bytes::snapshot(&output);
            let client = f.client(1);
            assert_eq!(
                client
                    .fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest)
                    .await
                    .err(),
                Some(FileDownloadError::Assembly(ChunkStagingError::Conflict))
            );
            assert_eq!(request_indices(&f.take_requests()), [0, 1]);
            assert_eq!(bytes::snapshot(&output), original);
            assert_eq!(bytes::tree(&f.root).len(), 3);
            for index in [0, 1] {
                let mut chunk =
                    fs::File::open(f.root.join(chunk_name(&f.manifest, index))).unwrap();
                verify_chunk(&f.manifest, index, &mut chunk).unwrap();
            }
        })
    })
    .await;
}

#[tokio::test]
async fn private_root_drift_reports_first_staging_failure_without_repair_or_later_request() {
    checked(Fixture::new(FILE_TRANSFER_CHUNK_BYTES + 17).await, |f| {
        Box::pin(async move {
            let client = f.client(1);
            fs::set_permissions(&f.root, fs::Permissions::from_mode(0o770)).unwrap();
            let drifted = bytes::snapshot(&f.root);
            assert_eq!(
                client
                    .fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest)
                    .await
                    .err(),
                Some(staging_error(0, ChunkStagingError::Unavailable))
            );
            assert_eq!(request_indices(&f.take_requests()), [0]);
            assert_eq!(bytes::snapshot(&f.root), drifted);
            assert!(bytes::tree(&f.root).is_empty());
        })
    })
    .await;
}
