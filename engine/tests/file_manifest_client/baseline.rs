use crate::{fixture::*, *};
use std::{fs, os::unix::fs::MetadataExt, time::Duration};

fn root_identity(root: &fs::Metadata) -> (u64, u64, u32, u32) {
    (root.dev(), root.ino(), root.uid(), root.mode())
}

#[tokio::test]
async fn manifest_client_makes_actual_six_selector_json_request_returns_complete_version_and_leaves_files_untouched(
) {
    let (result, expected, requests, before, after, original_root, final_root) =
        checked(Fixture::new().await, |f| {
            Box::pin(async move {
                let expected = f.manifest.clone();
                let original_root = root_identity(&fs::metadata(&f.root).unwrap());
                let before = bytes::tree(&f.root);
                let client = f.client();
                let result = tokio::time::timeout(
                    Duration::from_secs(20),
                    client.fetch_manifest(
                        TOKEN,
                        grant(),
                        upload(),
                        &FileVersionRef::from_manifest(&expected),
                    ),
                )
                .await;
                let requests = f.take_requests();
                let after = bytes::tree(&f.root);
                let final_root = root_identity(&fs::metadata(&f.root).unwrap());
                drop(client);
                (
                    result,
                    expected,
                    requests,
                    before,
                    after,
                    original_root,
                    final_root,
                )
            })
        })
        .await;
    let manifest = result
        .expect("actual metadata transport must finish within the baseline bound")
        .expect("actual HTTP response must yield the complete checked manifest, not a placeholder");
    assert_eq!(manifest, expected);
    assert_eq!(
        FileVersionRef::from_manifest(&manifest),
        FileVersionRef::from_manifest(&expected)
    );
    assert_eq!(manifest.chunk_sha256().len(), 2);
    assert_ne!(
        manifest.chunk(0).unwrap().sha256,
        manifest.chunk(1).unwrap().sha256
    );
    assert_eq!(manifest.byte_length(), FILE_TRANSFER_CHUNK_BYTES + 17);
    assert_eq!(manifest.display_name(), "真实作业.bin");
    assert_eq!(
        requests.len(),
        1,
        "metadata fetching must not retry or request chunks"
    );
    assert_request(&requests[0], &expected, None);
    assert!(requests[0].staged_before_request.is_empty());
    assert!(before.is_empty());
    assert_eq!(
        after, before,
        "metadata fetching must not stage or assemble any file"
    );
    assert_eq!(final_root, original_root);
    assert_eq!(final_root.3 & 0o7777, 0o700);
}

#[tokio::test]
async fn granted_download_fetches_manifest_then_ordered_chunks_and_assembles_actual_private_file_with_capacity_one(
) {
    let (result, manifest, requests, entries, observed, original_root, final_root, chunks) =
        checked(Fixture::new().await, |f| {
            Box::pin(async move {
                let manifest = f.manifest.clone();
                let original_root = root_identity(&fs::metadata(&f.root).unwrap());
                let client = f.client();
                let result = tokio::time::timeout(
                    Duration::from_secs(20),
                    client.fetch_granted_and_assemble(
                        TOKEN,
                        grant(),
                        upload(),
                        &FileVersionRef::from_manifest(&manifest),
                    ),
                )
                .await;
                let observed = observe_output(&f.root.join(output_name(&manifest)), &manifest);
                let chunks = [0, 1].map(|index| {
                    fs::File::open(f.root.join(chunk_name(&manifest, index))).map(|mut file| {
                        verify_chunk(&manifest, index, &mut file).map_err(|error| error.to_string())
                    })
                });
                let entries = bytes::tree(&f.root);
                let requests = f.take_requests();
                let final_root = root_identity(&fs::metadata(&f.root).unwrap());
                drop(client);
                (
                    result,
                    manifest,
                    requests,
                    entries,
                    observed,
                    original_root,
                    final_root,
                    chunks,
                )
            })
        })
        .await;
    let assembled = result
        .expect("capacity one must not self-deadlock across metadata and chunks")
        .expect("actual metadata/chunk HTTP must produce a private whole-file acknowledgement");
    assert_eq!(assembled.file(), &FileVersionRef::from_manifest(&manifest));
    assert_eq!(
        requests.len(),
        3,
        "one metadata request then one per chunk, with no retry"
    );
    for (request, index) in requests.iter().zip([None, Some(0), Some(1)]) {
        assert_request(request, &manifest, index);
    }
    assert!(requests[0].staged_before_request.is_empty());
    assert!(requests[1].staged_before_request.is_empty());
    assert_eq!(
        requests[2].staged_before_request.len(),
        1,
        "first chunk ACK must precede the next actual request"
    );
    let first = &requests[2].staged_before_request[0];
    assert_eq!(
        first.name,
        std::ffi::OsString::from(chunk_name(&manifest, 0))
    );
    assert!(
        entries.contains(first),
        "assembly preserves the original confirmed first chunk inode and bytes"
    );
    assert_eq!(entries.len(), 3);
    for (index, verified) in chunks.into_iter().enumerate() {
        verified
            .expect("independent reopen must find each actual chunk")
            .expect("independent exact EOF/digest check");
        let entry = entries
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
    let observed = observed.expect("independent reopen must find actual whole-file output");
    assert_eq!(observed.mode & 0o7777, 0o400);
    assert_eq!(observed.owner, original_root.2);
    assert_eq!(observed.links, 1);
    assert_eq!(observed.length, manifest.byte_length());
    assert!(observed.same_visible_inode);
    observed
        .verified
        .expect("whole-file chunk order, exact EOF and digest check");
    assert_eq!(observed.observed_length, FILE_TRANSFER_CHUNK_BYTES + 17);
    assert!(observed.pattern_matches);
    assert_eq!(observed.digest, manifest.sha256().to_string());
    assert_eq!(final_root, original_root);
    assert_eq!(final_root.3 & 0o7777, 0o700);
}
