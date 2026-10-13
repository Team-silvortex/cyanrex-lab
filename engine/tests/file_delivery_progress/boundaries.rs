use crate::{fixture::*, support::*, *};
use std::sync::Arc;

#[tokio::test]
async fn held_manifest_headers_and_partial_json_report_only_expected_total_then_close_without_staging_or_save(
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
                let (observer, mut progress) = FileDeliveryObserver::channel();
                let mut pending = Box::pin(client.fetch_granted_and_save_observed(
                    TOKEN,
                    grant(),
                    upload(),
                    &expected,
                    &target,
                    observer,
                ));
                drive(f, 1, &mut pending).await;
                if partial {
                    tokio::select! {
                        _ = pending.as_mut() => panic!("held actual JSON tail must stay pending"),
                        _ = f.wait_pieces(Kind::Manifest, 1) => {}
                    }
                }
                let copied = snapshot(
                    f,
                    &progress,
                    FileDeliveryPhase::FetchingManifest,
                    0,
                    0,
                    None,
                );
                assert_eq!(within(progress.changed()).await, Some(copied));
                assert!(bytes::tree(&f.root).is_empty() && bytes::tree(&selected).is_empty());
                assert_eq!(f.request_count(), 1);
                drop(pending);
                closed(&mut progress, Some(copied)).await;
                gate.add_permits(1);
                f.clear_reply(Kind::Manifest);
                plain(f, &client, &target).await.unwrap();
                assert_output(f, &selected.join(CHOSEN_LEAF));
                assert_eq!(
                    progress.latest(),
                    Some(copied),
                    "the old unobserved API cannot change another invocation's closed view"
                );
                assert_eq!(indices(&f.take_requests()), [None, None, Some(0)]);
            })
        })
        .await;
    }
}

#[tokio::test]
async fn matched_manifest_and_later_chunk_headers_or_partial_body_report_only_real_ack_prefix_not_recovered_progress(
) {
    for partial in [false, true] {
        checked(Fixture::new().await, |f| Box::pin(async move {
            let selected = selected_directory(f);
            let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
            let client = f.client();
            let expected = FileVersionRef::from_manifest(&f.manifest);
            let first = Arc::new(tokio::sync::Semaphore::new(0));
            let later = Arc::new(tokio::sync::Semaphore::new(0));
            let mut first_reply = Reply::good(&f.manifest, Kind::Chunk(0));
            first_reply.header_gate = Some(first.clone());
            f.set_reply(Kind::Chunk(0), first_reply);
            let mut later_reply = Reply::good(&f.manifest, Kind::Chunk(1));
            if partial {
                let data = later_reply.pieces[0].data.clone();
                later_reply.pieces = vec![Piece::now(data.slice(..8)), Piece::held(data.slice(8..), later.clone())];
            } else { later_reply.header_gate = Some(later.clone()); }
            f.set_reply(Kind::Chunk(1), later_reply);
            let (observer, mut progress) = FileDeliveryObserver::channel();
            let mut pending = Box::pin(client.fetch_granted_and_save_observed(TOKEN, grant(), upload(), &expected, &target, observer));
            drive(f, 2, &mut pending).await;
            let matched = snapshot(f, &progress, FileDeliveryPhase::StagingChunks, 0, 0, Some(2));
            assert_eq!(within(progress.changed()).await, Some(matched), "watch may skip FetchingManifest before this real held chunk");
            assert!(bytes::tree(&f.root).is_empty());
            first.add_permits(1);
            drive(f, 3, &mut pending).await;
            if partial {
                tokio::select! {
                    _ = pending.as_mut() => panic!("held actual second-chunk tail must stay pending"),
                    _ = f.wait_pieces(Kind::Chunk(1), 1) => {}
                }
            }
            let prefix = snapshot(f, &progress, FileDeliveryPhase::StagingChunks, 1, FILE_TRANSFER_CHUNK_BYTES, Some(2));
            assert_eq!(within(progress.changed()).await, Some(prefix));
            let files = bytes::tree(&f.root);
            assert_eq!(files.len(), 1);
            verify_prefix(f, 0);
            assert!(!f.root.join(output_name(&f.manifest)).exists());
            assert!(bytes::tree(&selected).is_empty());
            drop(pending);
            closed(&mut progress, Some(prefix)).await;
            later.add_permits(1);
            f.clear_reply(Kind::Chunk(0));
            f.clear_reply(Kind::Chunk(1));
            let (retry_observer, mut retry_progress) = FileDeliveryObserver::channel();
            assert_eq!(observed(f, &client, &target, retry_observer).await.err(), Some(staging_conflict()));
            let retry = snapshot(f, &retry_progress, FileDeliveryPhase::StagingChunks, 0, 0, Some(2));
            closed(&mut retry_progress, Some(retry)).await;
            assert_eq!(bytes::tree(&f.root), files);
            assert!(bytes::tree(&selected).is_empty());
            assert_eq!(indices(&f.take_requests()), [None, Some(0), Some(1), None, Some(0)]);
            within(f.staging.verify_staged_chunk(&f.manifest, 0)).await.unwrap();
            assert_eq!(progress.latest(), Some(prefix));
        })).await;
    }
}
