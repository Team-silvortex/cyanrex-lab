use crate::{fixture::*, support::*, *};
use std::sync::Arc;

#[tokio::test]
async fn cancelled_metadata_header_and_actual_partial_body_release_shared_slots_without_staging() {
    for partial_body in [false, true] {
        checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
            Box::pin(async move {
                let gate = Arc::new(tokio::sync::Semaphore::new(0));
                let mut reply = Reply::good(&f.manifest, Kind::Manifest);
                if partial_body {
                    let data = reply.pieces[0].data.clone();
                    reply.pieces = vec![
                        Piece::now(data.slice(..8)),
                        Piece::held(data.slice(8..), gate.clone()),
                    ];
                } else {
                    reply.header_gate = Some(gate.clone());
                }
                f.set_reply(Kind::Manifest, reply);
                let client = f.client();
                let expected = FileVersionRef::from_manifest(&f.manifest);
                let mut pending =
                    Box::pin(client.fetch_manifest(TOKEN, grant(), upload(), &expected));
                drive_to_request(f, 1, &mut pending).await;
                if partial_body {
                    tokio::select! {
                        _ = pending.as_mut() => panic!("held real metadata tail must not complete"),
                        _ = f.wait_pieces(Kind::Manifest, 1) => {}
                    }
                }
                assert_eq!(
                    metadata(f, &client.clone()).await.err(),
                    Some(FileManifestClientError::CapacityExhausted)
                );
                assert_eq!(f.request_count(), 1);
                assert!(bytes::tree(&f.root).is_empty());
                drop(pending);
                gate.add_permits(1);
                healthy(f, &client).await;
                assert_eq!(
                    f.request_count(),
                    2,
                    "cancellation returns admission for an actual healthy GET"
                );
                assert!(bytes::tree(&f.root).is_empty());
            })
        })
        .await;
    }
}

#[tokio::test]
async fn default_four_metadata_slots_are_clone_shared_independent_clients_add_and_old_chunk_gate_is_separate(
) {
    checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
        Box::pin(async move {
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, Kind::Manifest);
            reply.header_gate = Some(gate.clone());
            f.set_reply(Kind::Manifest, reply);
            let client = FileChunkClient::new_loopback(&f.origin, f.staging.clone()).unwrap();
            let mut held = Vec::new();
            for count in 1..=4 {
                let client = client.clone();
                let expected = FileVersionRef::from_manifest(&f.manifest);
                let mut pending = Box::pin(async move {
                    client
                        .fetch_manifest(TOKEN, grant(), upload(), &expected)
                        .await
                });
                drive_to_request(f, count, &mut pending).await;
                held.push(pending);
            }
            assert_eq!(
                metadata(f, &client.clone()).await.err(),
                Some(FileManifestClientError::CapacityExhausted)
            );
            let expected = FileVersionRef::from_manifest(&f.manifest);
            for token in [
                "bad-token",
                "00000000-0000-0000-0000-000000000000",
                "2BA67287-CB7C-4850-A551-966104F77B35",
                "{2ba67287-cb7c-4850-a551-966104f77b35}",
            ] {
                assert_eq!(
                    within(client.fetch_manifest(token, grant(), upload(), &expected))
                        .await
                        .err(),
                    Some(FileManifestClientError::InvalidInput)
                );
            }
            for authority in [false, true] {
                let mut wrong_scope = scope();
                if authority {
                    wrong_scope.authority_id = Uuid::from_u128(0xab).try_into().unwrap();
                } else {
                    wrong_scope.workspace_id = Uuid::from_u128(0xab).try_into().unwrap();
                }
                let wrong = changed(&f.manifest, "workspace", serde_json::json!(wrong_scope));
                assert_eq!(
                    within(client.fetch_manifest(
                        TOKEN,
                        grant(),
                        upload(),
                        &FileVersionRef::from_manifest(&wrong)
                    ))
                    .await
                    .err(),
                    Some(FileManifestClientError::InvalidInput)
                );
            }
            assert_eq!(
                f.request_count(),
                4,
                "cheap invalid inputs and the fifth clone never enter HTTP"
            );
            assert!(bytes::tree(&f.root).is_empty());
            f.clear_reply(Kind::Manifest);
            let independent = f.client();
            let retained = metadata(f, &independent).await.unwrap();
            assert_eq!(
                f.request_count(),
                5,
                "independent construction has additive metadata admission"
            );
            let received = within(client.fetch_and_stage(TOKEN, grant(), upload(), &f.manifest, 0))
                .await
                .unwrap();
            assert_eq!(received.file(), &expected);
            assert_eq!(
                f.request_count(),
                6,
                "full metadata gate does not consume the old chunk request gate"
            );
            let confirmed = bytes::tree(&f.root);
            assert_eq!(confirmed.len(), 1);
            drop(held);
            gate.add_permits(4);
            healthy(f, &client).await;
            assert_eq!(f.request_count(), 7);
            assert_eq!(
                retained, f.manifest,
                "retaining parsed metadata is not a capacity lease"
            );
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, Kind::Chunk(0));
            reply.header_gate = Some(gate.clone());
            f.set_reply(Kind::Chunk(0), reply);
            let mut held_chunks = Vec::new();
            for count in 8..=11 {
                let client = client.clone();
                let manifest = f.manifest.clone();
                let mut pending = Box::pin(async move {
                    client
                        .fetch_and_stage(TOKEN, grant(), upload(), &manifest, 0)
                        .await
                });
                drive_to_request(f, count, &mut pending).await;
                held_chunks.push(pending);
            }
            assert_eq!(
                within(client.fetch_and_stage(TOKEN, grant(), upload(), &f.manifest, 0))
                    .await
                    .err(),
                Some(FileChunkClientError::CapacityExhausted)
            );
            assert_eq!(
                metadata(f, &client).await.unwrap(),
                f.manifest,
                "the independently full old chunk gate cannot consume metadata slots"
            );
            assert_eq!(f.request_count(), 12);
            drop(held_chunks);
            gate.add_permits(4);
            assert_eq!(
                bytes::tree(&f.root),
                confirmed,
                "cancelled header requests never adopt or alter the confirmed chunk"
            );
            let requests = f.take_requests();
            assert_eq!(
                indices(&requests),
                [
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(0),
                    None,
                    Some(0),
                    Some(0),
                    Some(0),
                    Some(0),
                    None
                ]
            );
        })
    })
    .await;
}

#[tokio::test]
async fn granted_download_and_old_entry_share_flow_gate_while_metadata_slot_returns_before_chunk_ack(
) {
    checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
        Box::pin(async move {
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, Kind::Manifest);
            reply.header_gate = Some(gate.clone());
            f.set_reply(Kind::Manifest, reply);
            let client = FileChunkClient::new_loopback(&f.origin, f.staging.clone()).unwrap();
            let mut held = Vec::new();
            for count in 1..=4 {
                let client = client.clone();
                let expected = FileVersionRef::from_manifest(&f.manifest);
                let mut pending = Box::pin(async move { client.fetch_granted_and_assemble(TOKEN, grant(), upload(), &expected).await });
                drive_to_request(f, count, &mut pending).await;
                held.push(pending);
            }
            assert_eq!(combined(f, &client.clone()).await.err(), Some(FileGrantedDownloadError::CapacityExhausted));
            assert_eq!(within(client.fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest)).await.err(), Some(FileDownloadError::CapacityExhausted));
            assert_eq!(metadata(f, &client).await.err(), Some(FileManifestClientError::CapacityExhausted));
            let expected = FileVersionRef::from_manifest(&f.manifest);
            assert_eq!(within(client.fetch_granted_and_assemble("bad-token", grant(), upload(), &expected)).await.err(), Some(FileGrantedDownloadError::InvalidInput));
            let mut other = scope();
            other.workspace_id = Uuid::from_u128(0xbc).try_into().unwrap();
            let wrong = changed(&f.manifest, "workspace", serde_json::json!(other));
            assert_eq!(within(client.fetch_granted_and_assemble(TOKEN, grant(), upload(), &FileVersionRef::from_manifest(&wrong))).await.err(), Some(FileGrantedDownloadError::InvalidInput));
            assert_eq!(f.request_count(), 4);
            assert!(bytes::tree(&f.root).is_empty());
            drop(held);
            gate.add_permits(4);
            f.clear_reply(Kind::Manifest);
            let one = f.client();
            let chunk_gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, Kind::Chunk(0));
            reply.header_gate = Some(chunk_gate.clone());
            f.set_reply(Kind::Chunk(0), reply);
            let mut pending = Box::pin(one.fetch_granted_and_assemble(TOKEN, grant(), upload(), &expected));
            drive_to_request(f, 6, &mut pending).await;
            assert_eq!(metadata(f, &one).await.unwrap(), f.manifest, "JSON EOF releases metadata admission while the same whole flow awaits an actual chunk");
            assert_eq!(combined(f, &one.clone()).await.err(), Some(FileGrantedDownloadError::CapacityExhausted));
            assert_eq!(within(one.fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest)).await.err(), Some(FileDownloadError::CapacityExhausted));
            assert_eq!(f.request_count(), 7);
            assert!(bytes::tree(&f.root).is_empty());
            drop(pending);
            chunk_gate.add_permits(1);
            f.clear_reply(Kind::Chunk(0));
            let assembled = combined(f, &one).await.unwrap();
            assert_eq!(assembled.file(), &expected);
            assert_eq!(indices(&f.take_requests()), [None, None, None, None, None, Some(0), None, None, Some(0)]);
            let output = observe_output(&f.root.join(output_name(&f.manifest)), &f.manifest).unwrap();
            output.verified.unwrap();
            assert!(output.pattern_matches && output.same_visible_inode);
            assert_eq!(bytes::tree(&f.root).len(), 2);
        })
    }).await;
}
