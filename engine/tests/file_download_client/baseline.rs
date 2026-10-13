use crate::{fixture::*, *};
use std::{fs, os::unix::fs::MetadataExt, time::Duration};

#[tokio::test]
async fn download_makes_two_real_ordered_requests_and_assembles_verified_private_whole_file() {
    let (result, manifest, requests, entries, observed, original_root, final_root, chunks) =
        checked(Fixture::new(FILE_TRANSFER_CHUNK_BYTES + 17).await, |f| {
            Box::pin(async move {
                let manifest = f.manifest.clone();
                let original_root = fs::metadata(&f.root).unwrap();
                let client = f.client(1);
                let result = tokio::time::timeout(
                    Duration::from_secs(20),
                    client.fetch_and_assemble(TOKEN, grant(), upload(), &manifest),
                )
                .await;
                let observed = observe_output(&f.root.join(output_name(&manifest)), &manifest);
                let chunks = [0, 1].map(|index| {
                    fs::File::open(f.root.join(chunk_name(&manifest, index))).map(|mut file| {
                        verify_chunk(&manifest, index, &mut file).map_err(|error| error.to_string())
                    })
                });
                let entries = bytes::tree(&f.root);
                let final_root = fs::metadata(&f.root).unwrap();
                let requests = f.take_requests();
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
        .expect("capacity one must complete without self-deadlock")
        .expect("actual HTTP chunks must produce a private whole-file acknowledgement");
    assert_eq!(assembled.file(), &FileVersionRef::from_manifest(&manifest));
    assert_eq!(
        requests.len(),
        2,
        "one request per chunk, with no hidden retry"
    );
    for (index, request) in requests.iter().enumerate() {
        assert_eq!(request.method, "GET");
        assert_eq!(request.uri.path(), "/platform/v1/file-chunks");
        let raw = request.uri.query().unwrap();
        assert!(!raw.contains('%') && !raw.contains('+'));
        assert_eq!(raw.split('&').count(), 7);
        assert_eq!(
            request.query.as_ref().unwrap(),
            &RequestQuery {
                grant_id: grant(),
                upload_id: upload(),
                file_id: manifest.file_id(),
                version_id: manifest.version_id(),
                byte_length: manifest.byte_length(),
                sha256: manifest.sha256().clone(),
                chunk_index: index as u32,
            }
        );
        for (name, value) in [
            ("cookie", format!("cyanrex_platform_session={TOKEN}")),
            ("x-cyanrex-platform-request", "1".to_owned()),
            ("accept-encoding", "identity".to_owned()),
            ("accept", "application/octet-stream".to_owned()),
        ] {
            assert_eq!(request.headers.get_all(name).iter().count(), 1);
            assert_eq!(request.headers[name], value);
        }
        for name in [
            "origin",
            "authorization",
            "content-type",
            "range",
            "if-range",
            "if-match",
            "if-none-match",
        ] {
            assert!(
                !request.headers.contains_key(name),
                "download must not invent {name}"
            );
        }
        assert!(request.body.is_empty());
    }
    assert!(requests[0].staged_before_request.is_empty());
    assert_eq!(
        requests[1].staged_before_request.len(),
        1,
        "chunk zero must be actually saved before requesting chunk one"
    );
    let original_chunk = &requests[1].staged_before_request[0];
    assert_eq!(
        original_chunk.name,
        std::ffi::OsString::from(chunk_name(&manifest, 0))
    );
    assert!(
        entries.contains(original_chunk),
        "assembly preserves original chunk identity and bytes"
    );
    assert_eq!(entries.len(), 3);
    for (index, verified) in chunks.into_iter().enumerate() {
        verified
            .expect("independent reopen must find the actual selected chunk")
            .expect("each chunk must pass exact EOF and SHA-256");
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
    assert_eq!(
        (
            final_root.dev(),
            final_root.ino(),
            final_root.uid(),
            final_root.mode()
        ),
        (
            original_root.dev(),
            original_root.ino(),
            original_root.uid(),
            original_root.mode()
        )
    );
    assert_eq!(final_root.mode() & 0o7777, 0o700);
    let observed = observed.expect("independent reopen must find an actual f1 whole-file output");
    assert_eq!(observed.mode & 0o7777, 0o400);
    assert_eq!(observed.owner, original_root.uid());
    assert_eq!(observed.links, 1);
    assert_eq!(observed.length, manifest.byte_length());
    assert!(observed.same_visible_inode);
    observed
        .verified
        .expect("whole-file chunk order, exact EOF and digest must verify");
    assert_eq!(observed.observed_length, FILE_TRANSFER_CHUNK_BYTES + 17);
    assert!(observed.pattern_matches);
    assert_eq!(observed.digest, manifest.sha256().to_string());
}
