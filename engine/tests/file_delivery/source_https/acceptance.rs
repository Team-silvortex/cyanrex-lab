use super::{fixture::CHOSEN_LEAF, support::*, *};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL and native OpenSSL loopback TLS"]
async fn postgres_openssl_controlled_delivery_saves_authorized_large_short_and_empty_selected_files(
) {
    let (observations, source_before, source_after, tls) = fixture::checked(|f| {
        Box::pin(async move {
            let short = f.source.sharing.base.manifest.clone();
            let large = publish(&f.source, 2, FILE_TRANSFER_CHUNK_BYTES + 17).await;
            let empty = publish(&f.source, 3, 0).await;
            let source_before = f.source.sharing.snapshot().await;
            let first_root = f.directory("https-client-short", 0o700);
            let peer = f.peer(&first_root);
            let mut observations = Vec::new();
            for (position, manifest, seed, mode) in [
                (0, short, 1, 0o750),
                (1, large, 2, 0o755),
                (2, empty, 3, 0o750),
            ] {
                let root = if position == 0 {
                    first_root.clone()
                } else {
                    f.directory(&format!("https-client-{position}"), 0o700)
                };
                let selected = f.directory(&format!("https-selected-{position}"), mode);
                peer.set_root(root.clone());
                let client = f.client(&peer, &root);
                let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
                let expected = FileVersionRef::from_manifest(&manifest);
                let root_before = directory_policy(&root);
                let selected_before = directory_policy(&selected);
                let (handle, cancellation) = FileDeliveryCancelHandle::channel();
                let handle = if manifest.byte_length() == 0 {
                    drop(handle);
                    None
                } else {
                    Some(handle)
                };
                let (observer, mut progress) = FileDeliveryObserver::channel();
                assert_eq!(progress.latest(), None);
                let result = tokio::time::timeout(
                    Duration::from_secs(30),
                    client.fetch_granted_and_save_controlled(
                        f.token(),
                        sharing_fixture::recipient_grant(seed),
                        publication_fixture::upload(seed),
                        &expected,
                        &target,
                        observer,
                        cancellation,
                    ),
                )
                .await
                .map(|result| result.map(|saved| saved.file().clone()));
                if let Some(handle) = handle {
                    handle.cancel();
                }
                let last = progress.latest();
                ended(&mut progress).await;
                drop((progress, target, client));
                let source_output =
                    observe_output(&root.join(name(&manifest, "f1", None)), &manifest);
                let output = observe_output(&selected.join(CHOSEN_LEAF), &manifest);
                let chunks = (0..manifest.chunk_sha256().len() as u32)
                    .map(|index| {
                        fs::File::open(root.join(name(&manifest, "v1", Some(index)))).map(
                            |mut file| {
                                verify_chunk(&manifest, index, &mut file)
                                    .map_err(|error| error.to_string())
                            },
                        )
                    })
                    .collect::<Vec<_>>();
                let requests = peer.take_requests();
                if matches!(&result, Ok(Ok(_))) {
                    let expected = std::iter::once(Kind::Manifest)
                        .chain((0..manifest.chunk_sha256().len() as u32).map(Kind::Chunk))
                        .collect::<Vec<_>>();
                    assert_requests(&requests, &manifest, seed, f.token(), &expected);
                }
                observations.push((
                    result,
                    last,
                    manifest,
                    seed,
                    mode,
                    requests,
                    peer.take_statuses(),
                    source_output,
                    output,
                    chunks,
                    bytes::tree(&root),
                    bytes::tree(&selected),
                    root_before,
                    directory_policy(&root),
                    selected_before,
                    directory_policy(&selected),
                ));
            }
            peer.wait_responded(6).await;
            let tls = peer.finish();
            let source_after = f.source.sharing.snapshot().await;
            (observations, source_before, source_after, tls)
        })
    })
    .await;
    // Actual Source/TLS process, auth-pool drain and schema/file cleanup all precede success assertions.
    assert_eq!(source_after, source_before);
    let mut expected_requests = 0;
    for (
        result,
        last,
        manifest,
        seed,
        mode,
        requests,
        statuses,
        source,
        output,
        chunks,
        source_entries,
        selected_entries,
        root_before,
        root_after,
        selected_before,
        selected_after,
    ) in observations
    {
        assert_eq!(result.expect("bounded genuine Source HTTPS delivery").expect(
            "actual active exact recipient authority and trusted TLS must allow checked selected saving"),
            FileVersionRef::from_manifest(&manifest));
        let count = manifest.chunk_sha256().len() as u32;
        let last = last.expect("actual final SavedFile ACK publishes Complete");
        assert_eq!(last.phase(), FileDeliveryPhase::Complete);
        assert_eq!(
            (
                last.staged_chunks(),
                last.staged_bytes(),
                last.total_bytes(),
                last.total_chunks()
            ),
            (
                count,
                manifest.byte_length(),
                manifest.byte_length(),
                Some(count)
            )
        );
        let expected = std::iter::once(Kind::Manifest)
            .chain((0..count).map(Kind::Chunk))
            .collect::<Vec<_>>();
        // The synthetic Session is secret even in a failing assertion; its value was checked inside.
        assert_eq!(requests.len(), expected.len());
        expected_requests += requests.len();
        for (request, kind) in requests.iter().zip(expected) {
            assert!(request.kind == kind);
        }
        assert_eq!(statuses, vec![axum::http::StatusCode::OK; requests.len()]);
        assert!(requests[0].staged_before_request.is_empty());
        if count > 0 {
            assert!(requests[1].staged_before_request.is_empty());
        }
        if count == 2 {
            assert_eq!(requests[2].staged_before_request.len(), 1);
            assert!(source_entries.contains(&requests[2].staged_before_request[0]));
            assert_ne!(
                manifest.chunk(0).unwrap().sha256,
                manifest.chunk(1).unwrap().sha256
            );
        }
        let source = source.expect("independent open of original acknowledged f1");
        let output = output.expect("independent open of host-selected output");
        assert_ne!((source.device, source.inode), (output.device, output.inode));
        assert_output(source, &manifest, root_before);
        assert_output(output, &manifest, selected_before);
        for chunk in chunks {
            chunk
                .expect("original staged chunk exists")
                .expect("independent chunk SHA and EOF");
        }
        assert_eq!(source_entries.len(), count as usize + 1);
        assert_eq!(selected_entries.len(), 1);
        assert_eq!(
            selected_entries[0].name,
            std::ffi::OsString::from(CHOSEN_LEAF)
        );
        assert_ne!(CHOSEN_LEAF, manifest.display_name());
        assert_eq!(root_after, root_before);
        assert_eq!(selected_after, selected_before);
        assert_eq!(root_before.4, 0o700);
        assert_eq!(selected_before.4, mode);
        assert_ne!(
            (root_before.0, root_before.1),
            (selected_before.0, selected_before.1)
        );
        assert!((1..=3).contains(&seed));
    }
    assert!(tls.received_bytes > 0);
    assert_eq!(tls.dispatched, expected_requests);
    assert_eq!(tls.responded, expected_requests);
    assert_eq!(tls.error, None);
}
