use super::{fixture::CHOSEN_LEAF, support::*, *};
use cyanrex_engine::services::auth_service::durable_source::SessionFileChunkReadError;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL and native OpenSSL loopback TLS"]
async fn postgres_openssl_explicit_tls_body_cancellation_closes_only_waiter_and_preserves_real_ack_prefix(
) {
    let (controls, before, after) = fixture::checked(|f| Box::pin(async move {
        let short = f.source.sharing.base.manifest.clone();
        let large = publish(&f.source, 2, FILE_TRANSFER_CHUNK_BYTES + 17).await;
        let before = f.source.sharing.snapshot().await;
        let mut controls = Vec::new();
        for (position, manifest, seed, kind, at) in [
            (0, short.clone(), 1, Kind::Manifest, HoldAt::Headers),
            (1, short.clone(), 1, Kind::Manifest, HoldAt::BodyPrefix(8)),
            (2, large, 2, Kind::Chunk(1), HoldAt::BodyPrefix(8)),
        ] {
            let root = f.directory(&format!("https-cancelled-{position}"), 0o700);
            let selected = f.directory(&format!("https-cancelled-selected-{position}"), 0o755);
            let parents = (directory_policy(&root), directory_policy(&selected));
            let (peer, route) = f.peer_with_router(&root);
            let hold: Hold = peer.hold(kind, at);
            let client = f.client(&peer, &root);
            let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
            let expected = FileVersionRef::from_manifest(&manifest);
            let (handle, cancellation) = FileDeliveryCancelHandle::channel();
            let (observer, mut progress) = FileDeliveryObserver::channel();
            let mut pending = Box::pin(client.fetch_granted_and_save_controlled(f.token(),
                sharing_fixture::recipient_grant(seed), publication_fixture::upload(seed),
                &expected, &target, observer, cancellation));
            tokio::select! {
                result = &mut pending => panic!("actual authorized Source frame must reach TLS hold first: {:?}", result.err()),
                _ = hold.wait_reached() => {},
            }
            let (phase, chunks, staged, total) = if kind == Kind::Manifest {
                (FileDeliveryPhase::FetchingManifest, 0, 0, None)
            } else { (FileDeliveryPhase::StagingChunks, 1, FILE_TRANSFER_CHUNK_BYTES, Some(2)) };
            assert_progress(&progress, phase, &manifest, chunks, staged, total);
            let last = progress.latest();
            let original = bytes::tree(&root);
            assert_eq!(original.len(), chunks as usize);
            if matches!(at, HoldAt::Headers) {
                metadata_capacity(&route, f, &manifest, seed).await;
            }
            if chunks == 1 {
                verify_chunk(&manifest, 0, &mut fs::File::open(root.join(name(&manifest, "v1", Some(0)))).unwrap()).unwrap();
                // The actual original Source Bytes owner survives the native pipe's partial write.
                assert_eq!(within(f.source.sharing.base.grants.auth.source.read_session_file_chunk(
                    f.token(), &f.source.workspace, &f.source.request())).await.err(),
                    Some(SessionFileChunkReadError::CapacityExhausted));
            }
            assert!(bytes::tree(&selected).is_empty());
            assert!(!root.join(name(&manifest, "f1", None)).exists());
            assert_eq!(f.source.sharing.snapshot().await, before);
            handle.cancel();
            assert_eq!(within(&mut pending).await.err(), Some(FileControlledSaveError::Cancelled));
            drop(pending);
            ended(&mut progress).await;
            assert_eq!(progress.latest(), last);
            assert_eq!(bytes::tree(&root), original);
            assert!(bytes::tree(&selected).is_empty());
            let requests = peer.take_requests();
            let expected = if chunks == 0 { vec![Kind::Manifest] }
                else { vec![Kind::Manifest, Kind::Chunk(0), Kind::Chunk(1)] };
            assert_requests(&requests, &manifest, seed, f.token(), &expected);
            assert_eq!(peer.take_statuses(), vec![axum::http::StatusCode::OK; expected.len()],
                "Source authorization already returned; cancelling this wait does not revoke it");
            if chunks == 1 { assert_eq!(requests[2].staged_before_request, original); }
            // BodyPrefix records pipe acceptance of its bytes, not client consumption or OS-task kill.
            // Stop this connection before releasing anything toward a fresh TLS connection.
            let tls = peer.finish();
            hold.release();
            assert!(tls.received_bytes > 0);
            assert_eq!(tls.dispatched, expected.len());
            assert!(tls.responded < tls.dispatched);
            assert_eq!(tls.error, None);
            if chunks == 1 {
                let source_ack = within(f.source.sharing.base.grants.auth.source.read_session_file_chunk(
                    f.token(), &f.source.workspace, &f.source.request())).await
                    .expect("stopped native peer drops the last real Source byte owner");
                assert_eq!(source_ack.bytes(), &(0..17).map(bytes::byte).collect::<Vec<_>>());
                drop(source_ack);
                assert!(!root.join(name(&manifest, "v1", Some(1))).exists());
            }
            assert_eq!((directory_policy(&root), directory_policy(&selected)), parents);
            assert_eq!(f.source.sharing.snapshot().await, before);
            drop((client, target, hold, peer));

            let fresh_root = f.directory(&format!("https-fresh-control-{position}"), 0o700);
            let fresh_selected = f.directory(&format!("https-fresh-selected-{position}"), 0o750);
            let healthy = f.peer(&fresh_root);
            let fresh_parents = (directory_policy(&fresh_root), directory_policy(&fresh_selected));
            let result = controlled(f, &healthy, &fresh_root, &fresh_selected, &short, 1, true).await;
            let output = observe_output(&fresh_selected.join(CHOSEN_LEAF), &short);
            let source = observe_output(&fresh_root.join(name(&short, "f1", None)), &short);
            assert_requests(&healthy.take_requests(), &short, 1, f.token(), &[Kind::Manifest, Kind::Chunk(0)]);
            assert_eq!(healthy.take_statuses(), [axum::http::StatusCode::OK; 2]);
            healthy.wait_responded(2).await;
            let tls = healthy.finish();
            assert_eq!(progress.latest(), last, "fresh successful calls cannot reopen the cancelled view");
            assert_eq!(bytes::tree(&root), original);
            assert!(bytes::tree(&selected).is_empty());
            assert_eq!((directory_policy(&fresh_root), directory_policy(&fresh_selected)), fresh_parents);
            controls.push((result, output, source, short.clone(), fresh_parents, tls));
        }
        let after = f.source.sharing.snapshot().await;
        (controls, before, after)
    })).await;
    assert_eq!(after, before);
    for (result, output, source, manifest, parents, tls) in controls {
        assert_eq!(
            result.expect("fresh root/peer is healthy without adopting any cancelled prefix"),
            FileVersionRef::from_manifest(&manifest)
        );
        let output = output.expect("fresh independently selected real output");
        let source = source.expect("fresh original acknowledged f1");
        assert_ne!((output.device, output.inode), (source.device, source.inode));
        assert_output(output, &manifest, parents.1);
        assert_output(source, &manifest, parents.0);
        assert!(tls.received_bytes > 0);
        assert_eq!(tls.dispatched, 2);
        assert_eq!(tls.responded, 2);
        assert_eq!(tls.error, None);
    }
}
