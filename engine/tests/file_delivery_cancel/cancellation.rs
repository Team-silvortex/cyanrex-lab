use crate::{fixture::*, support::*, *};
use std::sync::Arc;

#[tokio::test]
async fn selected_explicit_cancel_during_real_manifest_headers_or_partial_json_closes_fetching_view_without_files_and_restores_admission(
) {
    for partial in [false, true] {
        checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
            Box::pin(async move {
                let selected = selected_directory(f);
                let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
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
                let (handle, cancellation) = FileDeliveryCancelHandle::channel();
                let (observer, mut progress) = FileDeliveryObserver::channel();
                let mut pending = Box::pin(client.fetch_granted_and_save_controlled(
                    TOKEN,
                    grant(),
                    upload(),
                    &expected,
                    &target,
                    observer,
                    cancellation,
                ));
                drive(f, 1, &mut pending).await;
                if partial {
                    tokio::select! {
                        _ = pending.as_mut() => panic!("held actual JSON tail must remain pending"),
                        _ = f.wait_pieces(Kind::Manifest, 1) => {}
                    }
                }
                let last = snapshot(
                    f,
                    &progress,
                    FileDeliveryPhase::FetchingManifest,
                    0,
                    0,
                    None,
                );
                handle.cancel();
                assert_cancelled(
                    within(pending)
                        .await
                        .err()
                        .expect("selected explicit request must return Cancelled"),
                );
                closed(&mut progress, Some(last)).await;
                assert_eq!(f.request_count(), 1);
                assert!(bytes::tree(&f.root).is_empty() && bytes::tree(&selected).is_empty());
                gate.add_permits(1);
                f.clear_reply(Kind::Manifest);
                let (handle, cancellation) = FileDeliveryCancelHandle::channel();
                let (observer, mut fresh) = FileDeliveryObserver::channel();
                controlled(f, &client, &target, observer, cancellation)
                    .await
                    .unwrap();
                drop(handle);
                let complete = snapshot(f, &fresh, FileDeliveryPhase::Complete, 1, 17, Some(1));
                closed(&mut fresh, Some(complete)).await;
                assert_eq!(progress.latest(), Some(last));
                assert_output(f, &selected.join(CHOSEN_LEAF));
                assert_eq!(indices(&f.take_requests()), [None, None, Some(0)]);
            })
        })
        .await;
    }
}

#[tokio::test]
async fn selected_explicit_cancel_at_later_chunk_headers_or_partial_body_preserves_only_ack_prefix_and_never_resumes_existing_chunk(
) {
    for partial in [false, true] {
        checked(Fixture::new().await, |f| Box::pin(async move {
            let selected = selected_directory(f);
            let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
            let client = f.client();
            let expected = FileVersionRef::from_manifest(&f.manifest);
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, Kind::Chunk(1));
            if partial {
                let data = reply.pieces[0].data.clone();
                reply.pieces = vec![Piece::now(data.slice(..8)), Piece::held(data.slice(8..), gate.clone())];
            } else { reply.header_gate = Some(gate.clone()); }
            f.set_reply(Kind::Chunk(1), reply);
            let (handle, cancellation) = FileDeliveryCancelHandle::channel();
            let (observer, mut progress) = FileDeliveryObserver::channel();
            let mut pending = Box::pin(client.fetch_granted_and_save_controlled(TOKEN, grant(), upload(), &expected, &target, observer, cancellation));
            drive(f, 3, &mut pending).await;
            if partial {
                tokio::select! {
                    _ = pending.as_mut() => panic!("held actual later chunk tail must remain pending"),
                    _ = f.wait_pieces(Kind::Chunk(1), 1) => {}
                }
            }
            let last = snapshot(f, &progress, FileDeliveryPhase::StagingChunks, 1, FILE_TRANSFER_CHUNK_BYTES, Some(2));
            let prefix = bytes::tree(&f.root);
            assert_eq!(prefix.len(), 1);
            verify_prefix(f, 0);
            handle.cancel();
            assert_cancelled(within(pending).await.err().expect("selected explicit request must return Cancelled"));
            closed(&mut progress, Some(last)).await;
            assert_eq!(bytes::tree(&f.root), prefix);
            assert!(!f.root.join(output_name(&f.manifest)).exists());
            assert!(bytes::tree(&selected).is_empty());
            assert_eq!(f.request_count(), 3, "after cancellation is selected no new continuation is polled");
            gate.add_permits(1);
            f.clear_reply(Kind::Chunk(1));
            let (handle, cancellation) = FileDeliveryCancelHandle::channel();
            let (observer, mut fresh) = FileDeliveryObserver::channel();
            assert_eq!(controlled(f, &client, &target, observer, cancellation).await.err(), Some(wrapped(staging_conflict())));
            drop(handle);
            let retry = snapshot(f, &fresh, FileDeliveryPhase::StagingChunks, 0, 0, Some(2));
            closed(&mut fresh, Some(retry)).await;
            assert_eq!(bytes::tree(&f.root), prefix);
            assert!(bytes::tree(&selected).is_empty());
            assert_eq!(indices(&f.take_requests()), [None, Some(0), Some(1), None, Some(0)]);
            within(f.staging.verify_staged_chunk(&f.manifest, 0)).await.unwrap();
        })).await;
    }
}
