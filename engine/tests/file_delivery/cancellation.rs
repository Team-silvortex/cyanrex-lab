use crate::{fixture::*, support::*, *};
use std::{fs, sync::Arc};

#[tokio::test]
async fn cancelled_real_metadata_headers_or_partial_json_do_not_stage_or_save_and_restore_delivery_admission(
) {
    for partial in [false, true] {
        checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
            Box::pin(async move {
                let parent = selected_parent(f);
                let target = FileSaveTarget::open(&parent, CHOSEN_LEAF).unwrap();
                let client = f.client();
                let expected = FileVersionRef::from_manifest(&f.manifest);
                let gate = Arc::new(tokio::sync::Semaphore::new(0));
                let mut reply = Reply::good(&f.manifest, Kind::Manifest);
                if partial {
                    let data = reply.pieces[0].data.clone();
                    reply.pieces = vec![
                        Piece::now(data.slice(..8)),
                        Piece::held(data.slice(8..), gate.clone()),
                    ];
                } else {
                    reply.header_gate = Some(gate.clone());
                }
                f.set_reply(Kind::Manifest, reply);
                let mut pending = Box::pin(client.fetch_granted_and_save(
                    TOKEN,
                    grant(),
                    upload(),
                    &expected,
                    &target,
                ));
                drive(f, 1, &mut pending).await;
                if partial {
                    tokio::select! {
                        _ = pending.as_mut() => panic!("held real JSON tail must not complete"),
                        _ = f.wait_pieces(Kind::Manifest, 1) => {}
                    }
                }
                assert_eq!(
                    deliver(f, &client.clone(), &target).await.unwrap_err(),
                    FileGrantedSaveError::CapacityExhausted
                );
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
}

#[tokio::test]
async fn cancelled_later_chunk_headers_or_partial_body_keep_confirmed_prefix_without_assembly_save_or_resume(
) {
    for partial in [false, true] {
        checked(Fixture::new().await, |f| {
            Box::pin(async move {
                let parent = selected_parent(f);
                let target = FileSaveTarget::open(&parent, CHOSEN_LEAF).unwrap();
                let client = f.client();
                let expected = FileVersionRef::from_manifest(&f.manifest);
                let gate = Arc::new(tokio::sync::Semaphore::new(0));
                let mut reply = Reply::good(&f.manifest, Kind::Chunk(1));
                if partial {
                    let data = reply.pieces[0].data.clone();
                    reply.pieces = vec![Piece::now(data.slice(..8)), Piece::held(data.slice(8..), gate.clone())];
                } else { reply.header_gate = Some(gate.clone()); }
                f.set_reply(Kind::Chunk(1), reply);
                let mut pending = Box::pin(client.fetch_granted_and_save(TOKEN, grant(), upload(), &expected, &target));
                drive(f, 3, &mut pending).await;
                if partial {
                    tokio::select! {
                        _ = pending.as_mut() => panic!("held real second-chunk tail must not complete"),
                        _ = f.wait_pieces(Kind::Chunk(1), 1) => {}
                    }
                }
                let prefix = bytes::tree(&f.root);
                assert_eq!(prefix.len(), 1, "confirmed first chunk has ACKed before the second GET");
                assert_eq!(deliver(f, &client.clone(), &target).await.unwrap_err(), FileGrantedSaveError::CapacityExhausted);
                assert_eq!(f.request_count(), 3);
                drop(pending);
                gate.add_permits(1);
                f.clear_reply(Kind::Chunk(1));
                assert_eq!(deliver(f, &client, &target).await.unwrap_err(), staging_error(0, ChunkStagingError::Conflict),
                    "returned flow capacity permits a new attempt, not adoption or resumed save");
                assert_eq!(indices(&f.take_requests()), [None, Some(0), Some(1), None, Some(0)]);
                assert_eq!(bytes::tree(&f.root), prefix);
                verify_chunk(&f.manifest, 0, &mut fs::File::open(f.root.join(chunk_name(&f.manifest, 0))).unwrap()).unwrap();
                assert!(!f.root.join(output_name(&f.manifest)).exists());
                assert!(bytes::tree(&parent).is_empty());
            })
        }).await;
    }
}
