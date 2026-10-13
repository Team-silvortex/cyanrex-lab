use crate::{fixture::*, support::*, *};
use std::{fs, os::unix::fs::PermissionsExt, time::Duration};

#[tokio::test]
#[ignore = "requires explicit native OpenSSL loopback TLS fixture"]
async fn openssl_controlled_delivery_verifies_complete_large_empty_and_short_selected_outputs() {
    for (length, variant, mode) in [
        (FILE_TRANSFER_CHUNK_BYTES + 17, 1, 0o755),
        (0, 2, 0o755),
        (17, 3, 0o700),
    ] {
        let (
            result,
            initial,
            last,
            closure,
            manifest,
            requests,
            tls,
            source,
            selected,
            chunks,
            source_entries,
            selected_entries,
            source_before,
            source_after,
            selected_before,
            selected_after,
        ) = checked(Fixture::new(length, variant).await, move |f| {
            Box::pin(async move {
                fs::set_permissions(&f.selected, fs::Permissions::from_mode(mode)).unwrap();
                let target = if mode == 0o700 {
                    FileSaveTarget::open(&f.selected, CHOSEN_LEAF).unwrap()
                } else {
                    f.target().unwrap()
                };
                let client = f.client(true);
                let manifest = f.manifest.clone();
                let expected = FileVersionRef::from_manifest(&manifest);
                let source_before = directory_policy(&f.root);
                let selected_before = directory_policy(&f.selected);
                let (handle, cancellation) = FileDeliveryCancelHandle::channel();
                let mut handle = Some(handle);
                if length == 0 {
                    drop(handle.take());
                }
                let (observer, mut progress) = FileDeliveryObserver::channel();
                let initial = progress.latest();
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
                if let Some(handle) = handle.take() {
                    handle.cancel();
                }
                let last = progress.latest();
                let closure = tokio::time::timeout(Duration::from_secs(10), async {
                    let mut tail = Vec::new();
                    for _ in 0..2 {
                        match progress.changed().await {
                            Some(snapshot) => tail.push(snapshot),
                            None => return (tail, true),
                        }
                    }
                    (tail, false)
                })
                .await;
                drop(progress);
                drop(target);
                drop(client);
                // Collect fallible readback observations before cleanup; only assert success afterwards.
                let source = observe_output(&f.root.join(output_name(&manifest)), &manifest);
                let selected = observe_output(&f.selected.join(CHOSEN_LEAF), &manifest);
                let chunks = (0..manifest.chunk_sha256().len() as u32)
                    .map(|index| {
                        fs::File::open(f.root.join(chunk_name(&manifest, index))).map(|mut file| {
                            verify_chunk(&manifest, index, &mut file)
                                .map_err(|error| error.to_string())
                        })
                    })
                    .collect::<Vec<_>>();
                (
                    result,
                    initial,
                    last,
                    closure,
                    manifest,
                    f.take_requests(),
                    f.finish_server(),
                    source,
                    selected,
                    chunks,
                    bytes::tree(&f.root),
                    bytes::tree(&f.selected),
                    source_before,
                    directory_policy(&f.root),
                    selected_before,
                    directory_policy(&f.selected),
                )
            })
        })
        .await;
        assert_eq!(result.expect("bounded actual HTTPS delivery must finish").expect(
            "trusted CA/exact IP SAN must allow real metadata, chunks, original assembly and saving",
        ), FileVersionRef::from_manifest(&manifest));
        assert_eq!(initial, None);
        let last = last.expect("real selected save must produce Complete");
        assert_eq!(last.phase(), FileDeliveryPhase::Complete);
        assert_eq!(last.staged_chunks(), manifest.chunk_sha256().len() as u32);
        assert_eq!(last.staged_bytes(), length);
        assert_eq!(last.total_bytes(), length);
        assert_eq!(
            last.total_chunks(),
            Some(manifest.chunk_sha256().len() as u32)
        );
        let (tail, closed) = closure.expect("bounded final observation closure");
        assert!(closed, "one latest slot, not a retained event queue");
        for snapshot in tail {
            assert_eq!(snapshot, last);
        }
        let expected = std::iter::once(None)
            .chain((0..manifest.chunk_sha256().len() as u32).map(Some))
            .collect::<Vec<_>>();
        assert_eq!(requests.len(), expected.len());
        for (request, index) in requests.iter().zip(expected) {
            assert_request(request, &manifest, index);
        }
        assert!(requests[0].staged_before_request.is_empty());
        if length != 0 {
            assert!(requests[1].staged_before_request.is_empty());
        }
        if manifest.chunk_sha256().len() == 2 {
            assert_eq!(requests[2].staged_before_request.len(), 1);
            assert!(
                source_entries.contains(&requests[2].staged_before_request[0]),
                "the first actual acknowledged original chunk survives later whole copy/save"
            );
            assert_ne!(
                manifest.chunk(0).unwrap().sha256,
                manifest.chunk(1).unwrap().sha256
            );
        }
        assert!(
            tls.received_bytes > 0,
            "the verified TLS peer actually received decrypted HTTP"
        );
        assert_eq!(tls.responded, requests.len());
        assert_eq!(tls.error, None);
        assert_eq!(source_entries.len(), manifest.chunk_sha256().len() + 1);
        assert_eq!(chunks.len(), manifest.chunk_sha256().len());
        for chunk in chunks {
            chunk
                .expect("real chunk exists")
                .expect("independent chunk SHA and EOF");
        }
        let source = source.expect("original private f1 exists");
        let selected = selected.expect("actual host-selected new output exists");
        for (output, parent) in [(source, source_before), (selected, selected_before)] {
            output
                .verified
                .expect("independent ordered chunk/whole SHA and EOF");
            assert!(output.same_visible_inode && output.pattern_matches);
            assert_eq!(output.mode & 0o7777, 0o400);
            assert_eq!(output.owner, parent.2);
            assert_eq!(output.gid, parent.3);
            assert_eq!(output.links, 1);
            assert_eq!(output.length, length);
            assert_eq!(output.observed_length, length);
            assert_eq!(output.digest, manifest.sha256().to_string());
        }
        assert_ne!(CHOSEN_LEAF, manifest.display_name());
        assert_eq!(selected_entries.len(), 1);
        assert_eq!(
            selected_entries[0].name,
            std::ffi::OsString::from(CHOSEN_LEAF)
        );
        let original = source_entries
            .iter()
            .find(|entry| entry.name == std::ffi::OsString::from(output_name(&manifest)))
            .unwrap();
        assert_ne!(
            (original.device, original.inode),
            (selected_entries[0].device, selected_entries[0].inode)
        );
        assert_eq!(source_before, source_after);
        assert_eq!(selected_before, selected_after);
        assert_eq!(source_before.4, 0o700);
        assert_eq!(selected_before.4, mode);
        assert_ne!(
            (source_before.0, source_before.1),
            (selected_before.0, selected_before.1)
        );
    }
}
