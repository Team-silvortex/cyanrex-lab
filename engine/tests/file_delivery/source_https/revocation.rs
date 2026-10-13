use super::{fixture::CHOSEN_LEAF, support::*, *};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL and native OpenSSL loopback TLS"]
async fn postgres_openssl_revocation_after_manifest_and_first_chunk_ack_refuses_later_chunk_without_saving(
) {
    fixture::checked(|f| Box::pin(async move {
        let manifest = publish(&f.source, 2, FILE_TRANSFER_CHUNK_BYTES + 17).await;
        let empty = publish(&f.source, 3, 0).await;
        let root = f.directory("https-revoked-prefix", 0o700);
        let selected = f.directory("https-revoked-selected", 0o755);
        let peer = f.peer(&root);
        let hold: Hold = peer.hold(Kind::Chunk(1), HoldAt::BeforeDispatch);
        let client = f.client(&peer, &root);
        let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
        let expected = FileVersionRef::from_manifest(&manifest);
        let parents = (directory_policy(&root), directory_policy(&selected));
        let before = f.source.sharing.snapshot().await;
        let (handle, cancellation) = FileDeliveryCancelHandle::channel();
        let (observer, mut progress) = FileDeliveryObserver::channel();
        let mut pending = Box::pin(client.fetch_granted_and_save_controlled(f.token(),
            sharing_fixture::recipient_grant(2), publication_fixture::upload(2), &expected, &target, observer, cancellation));
        tokio::select! {
            result = &mut pending => panic!("actual later decrypted request must reach pre-Source barrier: {:?}", result.err()),
            _ = hold.wait_reached() => {},
        }
        assert_progress(&progress, FileDeliveryPhase::StagingChunks, &manifest, 1, FILE_TRANSFER_CHUNK_BYTES, Some(2));
        let last = progress.latest();
        let original = bytes::tree(&root);
        assert_eq!(original.len(), 1);
        verify_chunk(&manifest, 0, &mut fs::File::open(root.join(name(&manifest, "v1", Some(0)))).unwrap()).unwrap();
        assert!(bytes::tree(&selected).is_empty());
        assert!(!root.join(name(&manifest, "f1", None)).exists());
        assert_eq!(f.source.sharing.snapshot().await, before);
        f.source.sharing.store.revoke(sharing_fixture::recipient_grant(2), f.source.sharing.actor(),
            sharing_fixture::revision(1)).await.unwrap();
        let revoked = f.source.sharing.snapshot().await;
        sharing_fixture::recipient_delta(&f.source.sharing.schema, &before, &revoked, 2, true);
        hold.release();
        let result = within(&mut pending).await;
        drop(pending); drop(handle);
        assert_eq!(result.err(), Some(FileControlledSaveError::Delivery(FileGrantedSaveError::Download(
            FileGrantedDownloadError::Download(FileDownloadError::Chunk { index: 1,
                error: FileChunkClientError::Receive(FileChunkReceiveError::InvalidResponse) })))));
        ended(&mut progress).await;
        assert_eq!(progress.latest(), last);
        let requests = peer.take_requests();
        assert_requests(&requests, &manifest, 2, f.token(), &[Kind::Manifest, Kind::Chunk(0), Kind::Chunk(1)]);
        assert!(requests[0].staged_before_request.is_empty() && requests[1].staged_before_request.is_empty());
        assert_eq!(requests[2].staged_before_request, original);
        assert_eq!(peer.take_statuses(), [axum::http::StatusCode::OK, axum::http::StatusCode::OK,
            axum::http::StatusCode::NOT_FOUND]);
        let tls = peer.finish();
        assert!(tls.received_bytes > 0);
        assert_eq!(tls.dispatched, 3);
        assert_eq!(tls.error, None);
        assert_eq!(bytes::tree(&root), original);
        assert!(!root.join(name(&manifest, "v1", Some(1))).exists());
        assert!(!root.join(name(&manifest, "f1", None)).exists());
        assert!(bytes::tree(&selected).is_empty());
        assert_eq!((directory_policy(&root), directory_policy(&selected)), parents);
        assert_eq!(f.source.sharing.snapshot().await, revoked);
        drop((progress, target, client, hold, peer));

        let before = f.source.sharing.snapshot().await;
        f.source.sharing.store.revoke(sharing_fixture::recipient_grant(3), f.source.sharing.actor(),
            sharing_fixture::revision(1)).await.unwrap();
        let revoked_empty = f.source.sharing.snapshot().await;
        sharing_fixture::recipient_delta(&f.source.sharing.schema, &before, &revoked_empty, 3, true);
        let root = f.directory("https-revoked-empty", 0o700);
        let selected = f.directory("https-revoked-empty-selected", 0o750);
        let peer = f.peer(&root);
        let client = f.client(&peer, &root);
        let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
        let (handle, cancellation) = FileDeliveryCancelHandle::channel();
        drop(handle);
        let (observer, mut progress) = FileDeliveryObserver::channel();
        assert_eq!(within(client.fetch_granted_and_save_controlled(f.token(), sharing_fixture::recipient_grant(3),
            publication_fixture::upload(3), &FileVersionRef::from_manifest(&empty), &target, observer, cancellation)).await.err(),
            Some(FileControlledSaveError::Delivery(FileGrantedSaveError::Download(
                FileGrantedDownloadError::Manifest(FileManifestClientError::InvalidResponse)))));
        assert_progress(&progress, FileDeliveryPhase::FetchingManifest, &empty, 0, 0, None);
        ended(&mut progress).await;
        assert_requests(&peer.take_requests(), &empty, 3, f.token(), &[Kind::Manifest]);
        assert_eq!(peer.take_statuses(), [axum::http::StatusCode::NOT_FOUND]);
        let tls = peer.finish();
        assert!(tls.received_bytes > 0); assert_eq!(tls.dispatched, 1); assert_eq!(tls.error, None);
        assert!(bytes::tree(&root).is_empty() && bytes::tree(&selected).is_empty());
        assert_eq!(f.source.sharing.snapshot().await, revoked_empty);
    })).await;
}
