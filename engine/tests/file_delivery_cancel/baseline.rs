use crate::{fixture::*, *};
use std::{fs, time::Duration};

#[tokio::test]
async fn actual_uncancelled_capacity_one_cross_chunk_delivery_keeps_unique_handle_and_saves_owned_0755_file_with_complete_progress(
) {
    let (
        result,
        initial,
        latest,
        closure,
        manifest,
        requests,
        source,
        destination,
        chunks,
        source_entries,
        selected_entries,
        source_before,
        source_after,
        selected_before,
        selected_after,
    ) = checked(Fixture::new().await, |f| {
        Box::pin(async move {
            let selected = selected_directory(f);
            let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
            let manifest = f.manifest.clone();
            let expected = FileVersionRef::from_manifest(&manifest);
            let client = f.client();
            let source_before = directory_policy(&f.root);
            let selected_before = directory_policy(&selected);
            let (handle, cancellation) = FileDeliveryCancelHandle::channel();
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
            // The unique handle stays alive throughout the actual operation; no request is made.
            // Dropping it only after return cannot manufacture a cancellation or alter that result.
            drop(handle);
            let latest = progress.latest();
            let closure = tokio::time::timeout(Duration::from_secs(3), async {
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
            let source = observe_output(&f.root.join(output_name(&manifest)), &manifest);
            let destination = observe_output(&selected.join(CHOSEN_LEAF), &manifest);
            let chunks = [0, 1].map(|index| {
                fs::File::open(f.root.join(chunk_name(&manifest, index))).map(|mut file| {
                    verify_chunk(&manifest, index, &mut file).map_err(|error| error.to_string())
                })
            });
            (
                result,
                initial,
                latest,
                closure,
                manifest,
                f.take_requests(),
                source,
                destination,
                chunks,
                bytes::tree(&f.root),
                bytes::tree(&selected),
                source_before,
                directory_policy(&f.root),
                selected_before,
                directory_policy(&selected),
            )
        })
    })
    .await;
    let saved = result.expect("actual controlled delivery must finish within its real capacity-one IO lifetime")
        .expect("a live unused cancel handle must allow actual metadata, two chunks, original assembly and selected save, not a placeholder");
    assert_eq!(saved, FileVersionRef::from_manifest(&manifest));
    assert!(initial.is_none());
    let latest = latest.expect("actual selected-save ACK must publish completion");
    assert!(latest.phase() == FileDeliveryPhase::Complete);
    assert_eq!(latest.staged_chunks(), 2);
    assert_eq!(latest.staged_bytes(), FILE_TRANSFER_CHUNK_BYTES + 17);
    assert_eq!(latest.total_bytes(), manifest.byte_length());
    assert_eq!(latest.total_chunks(), Some(2));
    let (tail, closed) = closure.expect("returned operation must release its progress producer");
    assert!(
        closed,
        "closure is not a success receipt, but observation must end"
    );
    for snapshot in tail {
        assert!(
            snapshot == latest,
            "watch retains at most the final unseen slot"
        );
    }
    assert_eq!(manifest.byte_length(), FILE_TRANSFER_CHUNK_BYTES + 17);
    assert_eq!(manifest.chunk_sha256().len(), 2);
    assert_ne!(
        manifest.chunk(0).unwrap().sha256,
        manifest.chunk(1).unwrap().sha256
    );
    assert_ne!(CHOSEN_LEAF, manifest.display_name());
    assert_eq!(
        requests.len(),
        3,
        "control/observation adds no HTTP or retry"
    );
    for (request, index) in requests.iter().zip([None, Some(0), Some(1)]) {
        assert_request(request, &manifest, index);
    }
    assert!(requests[0].staged_before_request.is_empty());
    assert!(requests[1].staged_before_request.is_empty());
    assert_eq!(requests[2].staged_before_request.len(), 1);
    assert!(
        source_entries.contains(&requests[2].staged_before_request[0]),
        "original acknowledged chunk inode/bytes/snapshot survive later assembly/save"
    );
    assert_eq!(source_entries.len(), 3);
    for chunk in chunks {
        chunk
            .expect("independent reopen must find the real staged chunk")
            .expect("independent chunk SHA and exact EOF");
    }
    let source = source.expect("independent reopen must find the original private f1");
    let destination =
        destination.expect("independent reopen must find the explicitly selected actual file");
    for observed in [source, destination] {
        observed
            .verified
            .expect("independent ordered chunk SHA, whole SHA and exact EOF");
        assert!(observed.same_visible_inode && observed.pattern_matches);
        assert_eq!(observed.mode & 0o7777, 0o400);
        assert_eq!(observed.owner, source_before.2);
        assert_eq!(observed.links, 1);
        assert_eq!(observed.length, manifest.byte_length());
        assert_eq!(observed.observed_length, manifest.byte_length());
        assert_eq!(observed.digest, manifest.sha256().to_string());
    }
    assert_eq!(selected_entries.len(), 1);
    assert_eq!(
        selected_entries[0].name,
        std::ffi::OsString::from(CHOSEN_LEAF)
    );
    let source_file = source_entries
        .iter()
        .find(|entry| entry.name == std::ffi::OsString::from(output_name(&manifest)))
        .unwrap();
    assert_ne!(
        (source_file.device, source_file.inode),
        (selected_entries[0].device, selected_entries[0].inode)
    );
    assert_eq!(source_after, source_before);
    assert_eq!(selected_after, selected_before);
    assert_eq!(source_before.4, 0o700);
    assert_eq!(selected_before.4, 0o755);
    assert_ne!(
        (source_before.0, source_before.1),
        (selected_before.0, selected_before.1)
    );
}
