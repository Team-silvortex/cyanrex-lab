use crate::{
    fixture::{directory_policy, within},
    network_fixture::*,
    support::changed,
    *,
};
use std::sync::Arc;

#[tokio::test]
async fn actual_capacity_one_loopback_delivery_saves_large_and_empty_files_to_owned_0755_targets_without_extra_http(
) {
    for length in [FILE_TRANSFER_CHUNK_BYTES + 17, 0] {
        checked(Fixture::with_manifest(manifest(length)).await, |f| {
            Box::pin(async move {
                assert_eq!(f.manifest.workspace(), crate::legacy_fixture::scope());
                let selected = selected_directory(f);
                let target =
                    FileSaveTarget::open_user_directory(&selected, "host-network-selected.bin")
                        .unwrap();
                let before = directory_policy(&selected);
                let source_before = directory_policy(&f.root);
                assert_eq!(
                    deliver(f, &f.client(), &target).await.unwrap(),
                    FileVersionRef::from_manifest(&f.manifest)
                );
                let requests = f.take_requests();
                let expected = if length == 0 {
                    vec![None]
                } else {
                    vec![None, Some(0), Some(1)]
                };
                assert_eq!(indices(&requests), expected);
                for (request, index) in requests.iter().zip(expected) {
                    assert_request(request, &f.manifest, index);
                }
                assert!(requests[0].staged_before_request.is_empty());
                let source_entries = bytes::tree(&f.root);
                if length != 0 {
                    assert!(requests[1].staged_before_request.is_empty());
                    assert_eq!(requests[2].staged_before_request.len(), 1);
                    assert!(
                        source_entries.contains(&requests[2].staged_before_request[0]),
                        "the first actual chunk ACK and original inode survive whole delivery/save"
                    );
                }
                assert_eq!(source_entries.len(), if length == 0 { 1 } else { 3 });
                let source = assert_whole(f, &f.root.join(output_name(&f.manifest)));
                let saved = assert_whole(f, &selected.join("host-network-selected.bin"));
                assert_ne!(source, saved);
                assert_eq!(directory_policy(&f.root), source_before);
                assert_eq!(source_before.4, 0o700);
                assert_eq!(directory_policy(&selected), before);
                assert_eq!(before.4, 0o755);
                let entries = bytes::tree(&selected);
                assert_eq!(entries.len(), 1);
                assert_eq!(
                    entries[0].name,
                    std::ffi::OsString::from("host-network-selected.bin")
                );
                assert!(!selected.join(f.manifest.display_name()).exists());
            })
        })
        .await;
    }
}

#[tokio::test]
async fn new_user_target_same_source_and_bad_scope_precede_full_delivery_admission_and_cancelled_metadata_never_saves(
) {
    for partial in [false, true] {
        checked(Fixture::with_manifest(manifest(17)).await, |f| Box::pin(async move {
            let selected = selected_directory(f);
            let target = FileSaveTarget::open_user_directory(&selected, "selected-after-cancel.bin").unwrap();
            let same_root = FileSaveTarget::open_user_directory(&f.root, "forbidden-source-target.bin").unwrap();
            let client = f.client();
            let expected = FileVersionRef::from_manifest(&f.manifest);
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, Kind::Manifest);
            if partial {
                let data = reply.pieces[0].data.clone();
                reply.pieces = vec![Piece::now(data.slice(..8)), Piece::held(data.slice(8..), gate.clone())];
            } else { reply.header_gate = Some(gate.clone()); }
            f.set_reply(Kind::Manifest, reply);
            let parent = directory_policy(&selected);
            let mut pending = Box::pin(client.fetch_granted_and_save(TOKEN, grant(), upload(), &expected, &target));
            drive(f, 1, &mut pending).await;
            if partial {
                tokio::select! {
                    _ = pending.as_mut() => panic!("held actual JSON tail must stay pending"),
                    _ = f.wait_pieces(Kind::Manifest, 1) => {}
                }
            }
            assert_eq!(deliver(f, &client.clone(), &target).await.err(), Some(FileGrantedSaveError::CapacityExhausted));
            assert_eq!(deliver(f, &client, &same_root).await.err(), Some(FileGrantedSaveError::InvalidInput));
            for token in ["bad-token", "00000000-0000-0000-0000-000000000000"] {
                assert_eq!(within(client.fetch_granted_and_save(token, grant(), upload(), &expected, &target))
                    .await.err(), Some(FileGrantedSaveError::InvalidInput));
            }
            for authority in [false, true] {
                let mut wrong = f.manifest.workspace();
                if authority { wrong.authority_id = Uuid::from_u128(0xab).try_into().unwrap(); }
                else { wrong.workspace_id = Uuid::from_u128(0xab).try_into().unwrap(); }
                let manifest = changed(&f.manifest, "workspace", serde_json::json!(wrong));
                assert_eq!(within(client.fetch_granted_and_save(TOKEN, grant(), upload(),
                    &FileVersionRef::from_manifest(&manifest), &target)).await.err(), Some(FileGrantedSaveError::InvalidInput));
            }
            assert_eq!(f.request_count(), 1, "same cached source root and bad inputs never enter HTTP despite the full shared flow");
            assert!(bytes::tree(&f.root).is_empty() && bytes::tree(&selected).is_empty());
            drop(pending);
            gate.add_permits(1);
            f.clear_reply(Kind::Manifest);
            deliver(f, &client, &target).await.unwrap();
            assert_eq!(indices(&f.take_requests()), [None, None, Some(0)]);
            assert_whole(f, &selected.join("selected-after-cancel.bin"));
            assert_eq!(directory_policy(&selected), parent);
            assert_eq!(directory_policy(&f.root).4, 0o700);
        })).await;
    }
}
