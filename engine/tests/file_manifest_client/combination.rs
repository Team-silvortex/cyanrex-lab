use crate::{fixture::*, support::*, *};
use axum::http::StatusCode;
use std::{fs, os::unix::fs::MetadataExt, sync::Arc};

#[tokio::test]
async fn granted_empty_download_still_makes_one_metadata_get_and_creates_private_empty_output_not_resume(
) {
    checked(Fixture::with_manifest(bytes::manifest(0, 1)).await, |f| {
        Box::pin(async move {
            let client = f.client();
            let expected = FileVersionRef::from_manifest(&f.manifest);
            assert_eq!(
                within(client.fetch_granted_and_assemble(
                    "bad-token",
                    grant(),
                    upload(),
                    &expected
                ))
                .await
                .err(),
                Some(FileGrantedDownloadError::InvalidInput)
            );
            assert_eq!(f.request_count(), 0);
            let root = fs::metadata(&f.root).unwrap();
            let identity = (root.dev(), root.ino(), root.uid(), root.mode());
            let assembled = combined(f, &client).await.unwrap();
            assert_eq!(assembled.file(), &expected);
            let requests = f.take_requests();
            assert_eq!(
                indices(&requests),
                [None],
                "new empty flow observes metadata, never invents a chunk GET"
            );
            assert_request(&requests[0], &f.manifest, None);
            assert!(requests[0].staged_before_request.is_empty());
            let output =
                observe_output(&f.root.join(output_name(&f.manifest)), &f.manifest).unwrap();
            output.verified.unwrap();
            assert_eq!((output.length, output.observed_length), (0, 0));
            assert_eq!(output.mode & 0o7777, 0o400);
            assert_eq!(output.owner, identity.2);
            assert_eq!(output.links, 1);
            assert!(output.same_visible_inode && output.pattern_matches);
            assert_eq!(output.digest, f.manifest.sha256().to_string());
            let confirmed = bytes::tree(&f.root);
            assert_eq!(confirmed.len(), 1);
            assert_eq!(
                combined(f, &client).await.err(),
                Some(FileGrantedDownloadError::Download(
                    FileDownloadError::Assembly(ChunkStagingError::Conflict)
                ))
            );
            assert_eq!(
                indices(&f.take_requests()),
                [None],
                "a repeated empty flow still fetches metadata before local conflict"
            );
            assert_eq!(
                within(client.fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest))
                    .await
                    .err(),
                Some(FileDownloadError::Assembly(ChunkStagingError::Conflict))
            );
            assert_eq!(
                f.request_count(),
                0,
                "old supplied-manifest empty entry retains its zero-HTTP behavior"
            );
            assert_eq!(bytes::tree(&f.root), confirmed);
            let final_root = fs::metadata(&f.root).unwrap();
            assert_eq!(
                (
                    final_root.dev(),
                    final_root.ino(),
                    final_root.uid(),
                    final_root.mode()
                ),
                identity
            );
        })
    })
    .await;
}

#[tokio::test]
async fn granted_metadata_admission_contract_tuple_and_timeout_failures_never_request_chunks_or_dispatch_files(
) {
    checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
        Box::pin(async move {
            let client = f.client_with(config(1));
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, Kind::Manifest);
            reply.header_gate = Some(gate.clone());
            f.set_reply(Kind::Manifest, reply);
            let expected = FileVersionRef::from_manifest(&f.manifest);
            let mut held = Box::pin(client.fetch_manifest(TOKEN, grant(), upload(), &expected));
            drive_to_request(f, 1, &mut held).await;
            for _ in 0..2 {
                assert_eq!(
                    combined(f, &client).await.err(),
                    Some(FileGrantedDownloadError::Manifest(
                        FileManifestClientError::CapacityExhausted
                    )),
                    "metadata denial returns the admitted whole-flow slot before the next attempt"
                );
            }
            assert_eq!(f.request_count(), 1);
            drop(held);
            gate.add_permits(1);
            healthy(f, &client).await;
            let mut replies = Vec::new();
            let mut json = Reply::good(&f.manifest, Kind::Manifest);
            json.json_bytes(b"{}".to_vec());
            replies.push(json);
            let wrong = changed(
                &f.manifest,
                "version_id",
                serde_json::json!(Uuid::from_u128(0xdd)),
            );
            replies.push(Reply::good(&wrong, Kind::Manifest));
            let mut wrong_scope = scope();
            wrong_scope.authority_id = Uuid::from_u128(0xee).try_into().unwrap();
            let wrong = changed(&f.manifest, "workspace", serde_json::json!(wrong_scope));
            replies.push(Reply::good(&wrong, Kind::Manifest));
            let mut headers = Reply::good(&f.manifest, Kind::Manifest);
            headers
                .headers
                .insert("content-type", "application/octet-stream".parse().unwrap());
            replies.push(headers);
            let mut status = Reply::good(&f.manifest, Kind::Manifest);
            status.status = StatusCode::SERVICE_UNAVAILABLE;
            replies.push(status);
            let count = replies.len();
            for reply in replies {
                f.set_reply(Kind::Manifest, reply);
                let error = combined(f, &client).await.err().unwrap();
                assert_eq!(
                    error,
                    FileGrantedDownloadError::Manifest(FileManifestClientError::InvalidResponse)
                );
                redacted(f, &error);
                assert!(bytes::tree(&f.root).is_empty());
            }
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, Kind::Manifest);
            reply.header_gate = Some(gate.clone());
            f.set_reply(Kind::Manifest, reply);
            assert_eq!(
                combined(f, &client).await.err(),
                Some(FileGrantedDownloadError::Manifest(
                    FileManifestClientError::TimedOut
                ))
            );
            gate.add_permits(1);
            assert_eq!(f.request_count(), count + 3);
            assert_eq!(
                indices(&f.take_requests()),
                vec![None; count + 3],
                "every failed flow stops at metadata with no HTTP retry or chunk request"
            );
            assert!(bytes::tree(&f.root).is_empty());
            healthy(f, &client).await;
        })
    })
    .await;
}

#[tokio::test]
async fn granted_later_chunk_fault_retains_confirmed_prefix_and_repeats_conflict_without_adoption_or_overwrite(
) {
    for corrupt_body in [false, true] {
        checked(Fixture::with_manifest(bytes::manifest(2 * FILE_TRANSFER_CHUNK_BYTES + 17, 1)).await, |f| {
            Box::pin(async move {
                let client = f.client();
                let mut reply = Reply::good(&f.manifest, Kind::Chunk(1));
                if corrupt_body {
                    let mut data = reply.pieces[0].data.to_vec();
                    data[7] ^= 1;
                    reply.pieces[0].data = data.into();
                } else { reply.headers.insert("x-cyanrex-chunk-index", "0".parse().unwrap()); }
                f.set_reply(Kind::Chunk(1), reply);
                let error = combined(f, &client).await.err().unwrap();
                assert_eq!(error, FileGrantedDownloadError::Download(FileDownloadError::Chunk {
                    index: 1, error: FileChunkClientError::Receive(FileChunkReceiveError::InvalidResponse),
                }));
                redacted(f, &error);
                let requests = f.take_requests();
                assert_eq!(indices(&requests), [None, Some(0), Some(1)]);
                assert!(requests[0].staged_before_request.is_empty());
                assert!(requests[1].staged_before_request.is_empty());
                assert_eq!(requests[2].staged_before_request.len(), 1);
                let confirmed = bytes::tree(&f.root);
                assert_eq!(confirmed, requests[2].staged_before_request, "later failure preserves the original first-chunk ACK inode, digest and timestamps");
                assert_eq!(confirmed[0].mode & 0o7777, 0o400);
                assert_eq!(confirmed[0].links, 1);
                let mut first = fs::File::open(f.root.join(chunk_name(&f.manifest, 0))).unwrap();
                verify_chunk(&f.manifest, 0, &mut first).unwrap();
                assert!(!f.root.join(chunk_name(&f.manifest, 1)).exists());
                assert!(!f.root.join(chunk_name(&f.manifest, 2)).exists());
                assert!(!f.root.join(output_name(&f.manifest)).exists());
                f.clear_reply(Kind::Chunk(1));
                assert_eq!(combined(f, &client).await.err(), Some(staging_error(0, ChunkStagingError::Conflict)));
                assert_eq!(indices(&f.take_requests()), [None, Some(0)], "retry fetches metadata but never skips or adopts the existing chunk");
                assert_eq!(bytes::tree(&f.root), confirmed);
            })
        }).await;
    }
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            let client = f.client();
            let assembled = combined(f, &client).await.unwrap();
            assert_eq!(
                assembled.file(),
                &FileVersionRef::from_manifest(&f.manifest)
            );
            assert_eq!(indices(&f.take_requests()), [None, Some(0), Some(1)]);
            let output =
                observe_output(&f.root.join(output_name(&f.manifest)), &f.manifest).unwrap();
            output.verified.unwrap();
            assert!(output.pattern_matches && output.same_visible_inode);
            let confirmed = bytes::tree(&f.root);
            assert_eq!(confirmed.len(), 3);
            assert_eq!(
                combined(f, &client.clone()).await.err(),
                Some(staging_error(0, ChunkStagingError::Conflict))
            );
            assert_eq!(indices(&f.take_requests()), [None, Some(0)]);
            assert_eq!(
                bytes::tree(&f.root),
                confirmed,
                "completed repeat changes neither existing chunks nor verified whole output"
            );
        })
    })
    .await;
}
