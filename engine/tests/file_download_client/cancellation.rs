use crate::{fixture::*, support::*, *};
use std::sync::Arc;

async fn cancel_later_chunk(partial_body: bool) {
    checked(Fixture::new(FILE_TRANSFER_CHUNK_BYTES + 17).await, |f| {
        Box::pin(async move {
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, 1);
            if partial_body {
                let data = reply.pieces[0].data.clone();
                reply.pieces = vec![
                    Piece {
                        data: data.slice(..8),
                        gate: None,
                    },
                    Piece {
                        data: data.slice(8..),
                        gate: Some(gate.clone()),
                    },
                ];
            } else {
                reply.header_gate = Some(gate.clone());
            }
            f.set_reply(1, reply);
            let client = f.client(1);
            let mut pending =
                Box::pin(client.fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest));
            drive_to_request(f, 2, &mut pending).await;
            if partial_body {
                // An actual yielded prefix with held tail is pre-staging cancellation evidence.
                tokio::select! {
                    _ = pending.as_mut() => panic!("held actual partial body must not complete"),
                    _ = f.wait_pieces(1, 1) => {}
                }
            }
            let before = bytes::tree(&f.root);
            assert_eq!(before.len(), 1);
            assert_eq!(
                client
                    .clone()
                    .fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest)
                    .await
                    .err(),
                Some(FileDownloadError::CapacityExhausted)
            );
            assert_eq!(f.request_count(), 2);
            drop(pending);
            f.clear_reply(1);
            gate.add_permits(1);
            assert_eq!(
                client
                    .fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest)
                    .await
                    .err(),
                Some(staging_error(0, ChunkStagingError::Conflict)),
                "released flow can start again, but never adopts the original first chunk"
            );
            assert_eq!(request_indices(&f.take_requests()), [0, 1, 0]);
            assert_eq!(bytes::tree(&f.root), before);
            assert!(!f.root.join(chunk_name(&f.manifest, 1)).exists());
            assert!(!f.root.join(output_name(&f.manifest)).exists());
        })
    })
    .await;
}

#[tokio::test]
async fn cancelled_later_header_keeps_confirmed_first_chunk_and_releases_flow_slot() {
    cancel_later_chunk(false).await;
}

#[tokio::test]
async fn cancelled_later_partial_body_keeps_confirmed_first_chunk_and_releases_flow_slot() {
    cancel_later_chunk(true).await;
}
