use crate::{fixture::*, *};
use std::{fs, time::Duration};

#[tokio::test]
async fn real_granted_delivery_fetches_two_chunks_assembles_original_and_saves_explicit_new_file_with_capacity_one(
) {
    let (
        result,
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
            let selected = selected_parent(f);
            let target = FileSaveTarget::open(&selected, CHOSEN_LEAF).unwrap();
            let source_before = root_identity(&f.root);
            let selected_before = root_identity(&selected);
            let manifest = f.manifest.clone();
            let expected = FileVersionRef::from_manifest(&manifest);
            let client = f.client();
            let result = tokio::time::timeout(
                Duration::from_secs(30),
                client.fetch_granted_and_save(TOKEN, grant(), upload(), &expected, &target),
            )
            .await
            .map(|result| result.map(|saved| saved.file().clone()));
            // The borrowed target and client holders must drop before fixture cleanup or any
            // success assertion; a stub failure must still remove its own parent/listener.
            drop(target);
            drop(client);
            let source = observe_output(&f.root.join(output_name(&manifest)), &manifest);
            let destination = observe_output(&selected.join(CHOSEN_LEAF), &manifest);
            let chunks = [0, 1].map(|index| {
                fs::File::open(f.root.join(chunk_name(&manifest, index))).map(|mut file| {
                    verify_chunk(&manifest, index, &mut file).map_err(|error| error.to_string())
                })
            });
            let source_entries = bytes::tree(&f.root);
            let selected_entries = bytes::tree(&selected);
            let source_after = root_identity(&f.root);
            let selected_after = root_identity(&selected);
            (
                result,
                manifest,
                f.take_requests(),
                source,
                destination,
                chunks,
                source_entries,
                selected_entries,
                source_before,
                source_after,
                selected_before,
                selected_after,
            )
        })
    })
    .await;
    let saved = result.expect("capacity-one delivery must finish within the real transport/file-job bound")
        .expect("actual metadata, chunks, original assembly and selected save must succeed, not a placeholder");
    assert_eq!(saved, FileVersionRef::from_manifest(&manifest));
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
        "one metadata GET and exactly one GET per chunk; save adds no HTTP"
    );
    for (request, index) in requests.iter().zip([None, Some(0), Some(1)]) {
        assert_request(request, &manifest, index);
    }
    assert!(matches!(
        requests[0].selected.as_ref(),
        Ok(Selected::Manifest(_))
    ));
    assert!(requests[0].staged_before_request.is_empty());
    assert!(requests[1].staged_before_request.is_empty());
    assert_eq!(
        requests[2].staged_before_request.len(),
        1,
        "first actual chunk ACK precedes the later GET"
    );
    assert!(
        source_entries.contains(&requests[2].staged_before_request[0]),
        "the original confirmed first-chunk inode, bytes and timestamps survive assembly/save"
    );
    assert_eq!(
        source_entries.len(),
        3,
        "two confirmed chunks and the retained private original f1"
    );
    for (index, verified) in chunks.into_iter().enumerate() {
        verified
            .expect("independent reopen must find the real staged chunk")
            .expect("independent chunk SHA and exact EOF");
        let entry = source_entries
            .iter()
            .find(|entry| {
                entry.name == std::ffi::OsString::from(chunk_name(&manifest, index as u32))
            })
            .unwrap();
        assert_eq!(entry.mode & 0o7777, 0o400);
        assert_eq!(entry.links, 1);
        assert_eq!(
            entry.length,
            manifest.chunk(index as u32).unwrap().byte_length
        );
    }
    let source =
        source.expect("independent reopen must find this flow's actual private original f1");
    let destination = destination.expect("independent reopen must find the host-selected new file");
    let source_entry = source_entries
        .iter()
        .find(|entry| entry.name == std::ffi::OsString::from(output_name(&manifest)))
        .unwrap();
    assert_eq!(selected_entries.len(), 1);
    assert_eq!(
        selected_entries[0].name,
        std::ffi::OsString::from(CHOSEN_LEAF)
    );
    assert_ne!(
        (source_entry.device, source_entry.inode),
        (selected_entries[0].device, selected_entries[0].inode),
        "selected file is not a rename or hardlink"
    );
    for observed in [source, destination] {
        observed
            .verified
            .expect("independent ordered chunk/whole SHA and exact EOF");
        assert!(observed.same_visible_inode && observed.pattern_matches);
        assert_eq!(observed.mode & 0o7777, 0o400);
        assert_eq!(observed.owner, source_before.2);
        assert_eq!(observed.links, 1);
        assert_eq!(observed.length, manifest.byte_length());
        assert_eq!(observed.observed_length, FILE_TRANSFER_CHUNK_BYTES + 17);
        assert_eq!(observed.digest, manifest.sha256().to_string());
    }
    assert_eq!(source_after, source_before);
    assert_eq!(selected_after, selected_before);
    assert_eq!(source_before.3 & 0o7777, 0o700);
    assert_eq!(selected_before.3 & 0o7777, 0o700);
    assert_ne!(
        (source_before.0, source_before.1),
        (selected_before.0, selected_before.1)
    );
}
