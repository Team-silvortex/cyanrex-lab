use crate::{fixture::*, support::*, *};
use std::sync::Arc;

#[tokio::test]
async fn default_four_delivery_flows_share_old_entries_and_clones_with_pure_input_priority_and_independent_clients(
) {
    checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
        Box::pin(async move {
            let parent = selected_parent(f);
            let target = FileSaveTarget::open(&parent, CHOSEN_LEAF).unwrap();
            let same_root = FileSaveTarget::open(&f.root, "forbidden-same-root.bin").unwrap();
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, Kind::Manifest);
            reply.header_gate = Some(gate.clone());
            f.set_reply(Kind::Manifest, reply);
            let client = FileChunkClient::new_loopback(&f.origin, f.staging.clone()).unwrap();
            let mut held = Vec::new();
            for count in 1..=4 {
                let client = client.clone();
                let target = target.clone();
                let expected = FileVersionRef::from_manifest(&f.manifest);
                let mut pending = Box::pin(async move {
                    client
                        .fetch_granted_and_save(TOKEN, grant(), upload(), &expected, &target)
                        .await
                });
                drive(f, count, &mut pending).await;
                held.push(pending);
            }
            assert_eq!(
                deliver(f, &client.clone(), &target).await.unwrap_err(),
                FileGrantedSaveError::CapacityExhausted
            );
            assert_eq!(
                within(client.fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest))
                    .await
                    .err(),
                Some(FileDownloadError::CapacityExhausted)
            );
            let expected = FileVersionRef::from_manifest(&f.manifest);
            assert_eq!(
                within(client.fetch_granted_and_assemble(TOKEN, grant(), upload(), &expected))
                    .await
                    .err(),
                Some(FileGrantedDownloadError::CapacityExhausted)
            );
            assert_eq!(
                metadata(f, &client).await.err(),
                Some(FileManifestClientError::CapacityExhausted)
            );
            for token in [
                "bad-token",
                "00000000-0000-0000-0000-000000000000",
                "2BA67287-CB7C-4850-A551-966104F77B35",
            ] {
                assert_eq!(
                    within(client.fetch_granted_and_save(
                        token,
                        grant(),
                        upload(),
                        &expected,
                        &target
                    ))
                    .await
                    .err(),
                    Some(FileGrantedSaveError::InvalidInput)
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
                    within(client.fetch_granted_and_save(
                        TOKEN,
                        grant(),
                        upload(),
                        &FileVersionRef::from_manifest(&wrong),
                        &target
                    ))
                    .await
                    .err(),
                    Some(FileGrantedSaveError::InvalidInput)
                );
            }
            assert_eq!(
                deliver(f, &client, &same_root).await.unwrap_err(),
                FileGrantedSaveError::InvalidInput
            );
            assert_eq!(
                f.request_count(),
                4,
                "pure bad inputs precede full admission and cannot enter HTTP"
            );
            assert!(bytes::tree(&f.root).is_empty() && bytes::tree(&parent).is_empty());
            f.clear_reply(Kind::Manifest);
            let independent = f.client();
            let independent_target =
                FileSaveTarget::open(&parent, "independent-selected.bin").unwrap();
            deliver(f, &independent, &independent_target).await.unwrap();
            assert_eq!(
                f.request_count(),
                6,
                "independent construction adds its own whole/metadata capacity"
            );
            assert_output(f, &parent.join("independent-selected.bin"));
            assert_eq!(
                within(client.fetch_and_stage(TOKEN, grant(), upload(), &f.manifest, 0))
                    .await
                    .err(),
                Some(FileChunkClientError::Receive(
                    FileChunkReceiveError::Staging(ChunkStagingError::Conflict)
                )),
                "full whole/metadata gates do not consume the old chunk request gate"
            );
            assert_eq!(f.request_count(), 7);
            let source = bytes::tree(&f.root);
            let selected = bytes::tree(&parent);
            drop(held);
            gate.add_permits(4);
            assert_eq!(metadata(f, &client).await.unwrap(), f.manifest);
            assert_eq!(f.request_count(), 8);
            assert_eq!(bytes::tree(&f.root), source);
            assert_eq!(bytes::tree(&parent), selected);
            assert!(!parent.join(CHOSEN_LEAF).exists());
            assert_eq!(
                indices(&f.take_requests()),
                [None, None, None, None, None, Some(0), Some(0), None]
            );
        })
    })
    .await;
}

#[tokio::test]
async fn independently_full_metadata_gate_returns_nested_error_and_releases_admitted_whole_flow() {
    checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
        Box::pin(async move {
            let parent = selected_parent(f);
            let target = FileSaveTarget::open(&parent, CHOSEN_LEAF).unwrap();
            let client = f.client();
            let expected = FileVersionRef::from_manifest(&f.manifest);
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, Kind::Manifest);
            reply.header_gate = Some(gate.clone());
            f.set_reply(Kind::Manifest, reply);
            let mut pending = Box::pin(client.fetch_manifest(TOKEN, grant(), upload(), &expected));
            drive(f, 1, &mut pending).await;
            for _ in 0..2 {
                assert_eq!(
                    deliver(f, &client.clone(), &target).await.unwrap_err(),
                    metadata_error(FileManifestClientError::CapacityExhausted),
                    "metadata admission failure must return the newly acquired whole-flow slot"
                );
            }
            assert_eq!(f.request_count(), 1);
            assert!(bytes::tree(&f.root).is_empty() && bytes::tree(&parent).is_empty());
            drop(pending);
            gate.add_permits(1);
            f.clear_reply(Kind::Manifest);
            deliver(f, &client, &target).await.unwrap();
            assert_output(f, &parent.join(CHOSEN_LEAF));
            assert_eq!(indices(&f.take_requests()), [None, None, Some(0)]);
        })
    })
    .await;
}
