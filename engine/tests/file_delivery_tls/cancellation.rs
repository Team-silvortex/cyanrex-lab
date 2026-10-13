use crate::{fixture::*, support::*, *};
use std::{fs, time::Duration};

#[tokio::test]
#[ignore = "requires explicit native OpenSSL loopback TLS fixture"]
async fn openssl_explicit_cancellation_at_tls_manifest_and_later_chunk_boundaries_preserves_only_original_ack_prefix(
) {
    for (length, variant, kind, boundary) in [
        (17, 7, Kind::Manifest, HoldAt::Body(8)),
        (
            FILE_TRANSFER_CHUNK_BYTES + 17,
            8,
            Kind::Chunk(1),
            HoldAt::Headers,
        ),
        (
            FILE_TRANSFER_CHUNK_BYTES + 17,
            9,
            Kind::Chunk(1),
            HoldAt::Body(8),
        ),
    ] {
        checked(Fixture::new(length, variant).await, move |f| Box::pin(async move {
            let target = f.target().unwrap();
            let client = f.client(true);
            let expected = FileVersionRef::from_manifest(&f.manifest);
            let parents = (directory_policy(&f.root), directory_policy(&f.selected));
            let hold = f.hold(kind, boundary);
            let (handle, cancellation) = FileDeliveryCancelHandle::channel();
            let (observer, mut progress) = FileDeliveryObserver::channel();
            let mut pending = Box::pin(client.fetch_granted_and_save_controlled(
                TOKEN, grant(), upload(), &expected, &target, observer, cancellation,
            ));
            tokio::time::timeout(Duration::from_secs(10), async {
                tokio::select! {
                    result = &mut pending => panic!("actual held TLS response must stay pending: {:?}", result.err()),
                    _ = hold.wait_reached() => (),
                }
            }).await.expect("actual TLS pipe reaches the selected header/partial-body barrier");
            let (phase, staged_chunks, staged_bytes, total_chunks) = match kind {
                Kind::Manifest => (FileDeliveryPhase::FetchingManifest, 0, 0, None),
                Kind::Chunk(1) => (FileDeliveryPhase::StagingChunks, 1, FILE_TRANSFER_CHUNK_BYTES, Some(2)),
                _ => unreachable!(),
            };
            let last = snapshot(f, &progress, phase, staged_chunks, staged_bytes, total_chunks);
            let original = bytes::tree(&f.root);
            assert_eq!(original.len(), staged_chunks as usize);
            if staged_chunks == 1 {
                verify_chunk(&f.manifest, 0, &mut fs::File::open(f.root.join(chunk_name(&f.manifest, 0))).unwrap()).unwrap();
            }
            assert!(!f.root.join(output_name(&f.manifest)).exists());
            assert!(bytes::tree(&f.selected).is_empty());
            assert!(f.received_bytes() > 0, "trusted TLS actually decrypted this original request");
            handle.cancel();
            assert_eq!(tokio::time::timeout(Duration::from_secs(30), &mut pending).await.unwrap().err(),
                Some(FileControlledSaveError::Cancelled));
            drop(pending);
            closed(&mut progress, Some(last)).await;
            assert_eq!(bytes::tree(&f.root), original);
            assert!(bytes::tree(&f.selected).is_empty());
            assert_eq!((directory_policy(&f.root), directory_policy(&f.selected)), parents);
            // Stop the old TLS pipe before a new connection: cancelled response tails are never
            // released onto a reused OpenSSL stdio connection. This is fixture hygiene, not rollback.
            let stopped = f.finish_server();
            assert_eq!(stopped.error, None);
            hold.release();
            let requests = f.take_requests();
            let expected_requests = if staged_chunks == 0 { vec![None] } else { vec![None, Some(0), Some(1)] };
            assert_eq!(requests.len(), expected_requests.len());
            for (request, index) in requests.iter().zip(expected_requests) {
                assert_request(request, &f.manifest, index);
            }
            if staged_chunks == 1 {
                assert_eq!(requests[2].staged_before_request, original,
                    "the actual later request sees only the acknowledged original prefix");
                // Fresh TLS server/CA but original local root: a new call cannot adopt its prefix.
                let original_staging = f.staging.clone();
                let retry_target = target.clone();
                let retry_expected = expected.clone();
                let (result, retry_manifest, retry_requests, retry_tls, retry_source) =
                    checked(Fixture::new(length, variant).await, move |retry| Box::pin(async move {
                        let retry_client = FileChunkClient::with_config(&retry.origin, original_staging, FileChunkClientConfig {
                            capacity: 1,
                            trusted_root_pem: Some(retry.ca.clone()),
                            ..FileChunkClientConfig::default()
                        }).unwrap();
                        let (handle, cancellation) = FileDeliveryCancelHandle::channel();
                        let (observer, mut retry_progress) = FileDeliveryObserver::channel();
                        let result = tokio::time::timeout(Duration::from_secs(30), retry_client.fetch_granted_and_save_controlled(
                            TOKEN, grant(), upload(), &retry_expected, &retry_target, observer, cancellation,
                        )).await.map(|result| result.map(|saved| saved.file().clone()));
                        drop(handle);
                        let value = snapshot(retry, &retry_progress, FileDeliveryPhase::StagingChunks, 0, 0, Some(2));
                        closed(&mut retry_progress, Some(value)).await;
                        // This new server's spy root is empty and is NOT claimed as the old root.
                        (result, retry.manifest.clone(), retry.take_requests(), retry.finish_server(), bytes::tree(&retry.root))
                    })).await;
                assert_eq!(retry_manifest, f.manifest);
                assert_eq!(result.expect("bounded real retry refusal").err(),
                    Some(FileControlledSaveError::Delivery(FileGrantedSaveError::Download(
                        FileGrantedDownloadError::Download(FileDownloadError::Chunk {
                            index: 0,
                            error: FileChunkClientError::Receive(FileChunkReceiveError::Staging(ChunkStagingError::Conflict)),
                        }),
                    ))));
                assert_eq!(retry_requests.len(), 2);
                for (request, index) in retry_requests.iter().zip([None, Some(0)]) {
                    assert_request(request, &f.manifest, index);
                }
                assert!(retry_tls.received_bytes > 0);
                assert_eq!(retry_tls.error, None);
                assert_eq!(retry_tls.responded, 2);
                assert!(retry_source.is_empty());
                assert_eq!(bytes::tree(&f.root), original);
                assert!(bytes::tree(&f.selected).is_empty());
                verify_chunk(&f.manifest, 0, &mut fs::File::open(f.root.join(chunk_name(&f.manifest, 0))).unwrap()).unwrap();
            }
            assert_eq!(progress.latest(), Some(last), "orphan/independent work cannot publish Complete in this closed view");
            assert_eq!((directory_policy(&f.root), directory_policy(&f.selected)), parents);
        })).await;
    }

    // Independent healthy origin, root and target after cancellations, not adoption or recovery.
    let (result, output, manifest, requests, tls, last) =
        checked(Fixture::new(17, 10).await, |f| {
            Box::pin(async move {
                let target = f.target().unwrap();
                let client = f.client(true);
                let expected = FileVersionRef::from_manifest(&f.manifest);
                let (handle, cancellation) = FileDeliveryCancelHandle::channel();
                drop(handle);
                let (observer, mut progress) = FileDeliveryObserver::channel();
                let result = tokio::time::timeout(
                    Duration::from_secs(30),
                    client.fetch_granted_and_save_controlled(
                        TOKEN,
                        grant(),
                        upload(),
                        &expected,
                        &target,
                        observer,
                        cancellation,
                    ),
                )
                .await
                .map(|result| result.map(|saved| saved.file().clone()));
                let last = progress.latest();
                closed(&mut progress, last).await;
                (
                    result,
                    observe_output(&f.selected.join(CHOSEN_LEAF), &f.manifest),
                    f.manifest.clone(),
                    f.take_requests(),
                    f.finish_server(),
                    last,
                )
            })
        })
        .await;
    assert_eq!(
        result
            .expect("bounded independent TLS control")
            .expect("a dropped handle does not cancel healthy delivery"),
        FileVersionRef::from_manifest(&manifest)
    );
    let output = output.expect("independent actual selected output exists");
    output
        .verified
        .expect("actual selected bytes/whole digest and EOF");
    assert!(output.pattern_matches && output.same_visible_inode);
    assert_eq!(output.mode & 0o7777, 0o400);
    assert_eq!(output.links, 1);
    assert_eq!(output.length, 17);
    assert_eq!(last.unwrap().phase(), FileDeliveryPhase::Complete);
    assert_eq!(requests.len(), 2);
    for (request, index) in requests.iter().zip([None, Some(0)]) {
        assert_request(request, &manifest, index);
    }
    assert!(tls.received_bytes > 0);
    assert_eq!(tls.error, None);
    assert_eq!(tls.responded, 2);
}
