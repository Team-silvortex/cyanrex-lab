use crate::{fixture::*, *};
use std::{fs, time::Duration};

#[tokio::test]
async fn actual_observed_capacity_one_cross_chunk_delivery_saves_owned_0755_file_then_reports_complete_and_closes(
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
            let (observer, mut progress) = FileDeliveryObserver::channel();
            let initial = progress.latest();
            let result = tokio::time::timeout(
                Duration::from_secs(30),
                client.fetch_granted_and_save_observed(
                    TOKEN,
                    grant(),
                    upload(),
                    &expected,
                    &target,
                    observer,
                ),
            )
            .await
            .map(|result| result.map(|saved| saved.file().clone()));
            let latest = progress.latest();
            let closure = tokio::time::timeout(Duration::from_secs(3), async {
                let mut tail = Vec::new();
                // The producer has returned. A watch reader may still consume its last unseen value,
                // but there is no event queue or unbounded drain; closure itself is not a success ACK.
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
    let saved = result.expect("capacity-one actual delivery must finish within its real IO lifetime")
        .expect("metadata, chunks, original assembly and selected saving must run, not an observed-entry placeholder");
    assert_eq!(saved, FileVersionRef::from_manifest(&manifest));
    assert!(
        initial.is_none(),
        "a new observer pair has no fabricated transfer state"
    );
    let latest = latest.expect("successful real saving must publish a final observation");
    assert!(latest.phase() == FileDeliveryPhase::Complete);
    assert_eq!(latest.staged_chunks(), 2);
    assert_eq!(latest.staged_bytes(), FILE_TRANSFER_CHUNK_BYTES + 17);
    assert_eq!(latest.total_bytes(), manifest.byte_length());
    assert_eq!(latest.total_chunks(), Some(2));
    let (tail, closed) =
        closure.expect("completed call must close its observer without a retained job sender");
    assert!(
        closed,
        "None ends observation; only the actual save result establishes success"
    );
    for snapshot in tail {
        assert!(
            snapshot == latest,
            "the single watch slot may retain only its final unseen snapshot"
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
        "observation adds no HTTP, retry or save authorization query"
    );
    for (request, index) in requests.iter().zip([None, Some(0), Some(1)]) {
        assert_request(request, &manifest, index);
    }
    assert!(requests[0].staged_before_request.is_empty());
    assert!(requests[1].staged_before_request.is_empty());
    assert_eq!(requests[2].staged_before_request.len(), 1);
    assert!(
        source_entries.contains(&requests[2].staged_before_request[0]),
        "the original first chunk ACK/inode/snapshot survives later assembly and saving"
    );
    assert_eq!(source_entries.len(), 3);
    for chunk in chunks {
        chunk
            .expect("independent reopen must find a real chunk")
            .expect("independent chunk SHA and EOF must verify");
    }
    let source = source.expect("independent reopen must find this call's original private f1");
    let destination =
        destination.expect("independent reopen must find the actual host-selected new file");
    for observed in [source, destination] {
        observed
            .verified
            .expect("independent ordered chunk SHA, whole SHA and EOF");
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
