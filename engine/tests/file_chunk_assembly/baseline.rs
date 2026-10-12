use crate::{fixture::*, *};
use std::{fs, os::unix::fs::MetadataExt};

#[tokio::test]
async fn assembly_creates_actual_cross_chunk_whole_file_and_preserves_original_private_inputs() {
    let f = Fixture::staged(FILE_TRANSFER_CHUNK_BYTES + 17).await;
    let manifest = f.manifest.clone();
    let expected_file = FileVersionRef::from_manifest(&manifest);
    let name = output_name(&manifest);
    let before = bytes::tree(&f.root);
    let original_source = bytes::snapshot(&f.source);
    let original_root = fs::metadata(&f.root).unwrap();
    let result = f.staging.assemble_staged_file(&manifest).await;
    let observed = observe_output(&f.root.join(&name), &manifest);
    let after = bytes::tree(&f.root);
    let source_after = bytes::snapshot(&f.source);
    let root_after = fs::metadata(&f.root).unwrap();
    f.cleanup();

    let assembled = result.expect(
        "actual legacy chunks must create a whole-file output, not return placeholder metadata",
    );
    assert_eq!(assembled.file(), &expected_file);
    assert_eq!(before.len(), 2);
    for index in [0, 1] {
        let entry = before
            .iter()
            .find(|e| e.name == std::ffi::OsString::from(chunk_name(&manifest, index)))
            .unwrap();
        assert_eq!(entry.mode & 0o7777, 0o400);
        assert_eq!(entry.links, 1);
        assert_eq!(entry.length, manifest.chunk(index).unwrap().byte_length);
        assert!(
            after.contains(entry),
            "assembly must not modify original input identity or bytes"
        );
    }
    assert_eq!(after.len(), before.len() + 1);
    assert!(after
        .iter()
        .any(|e| e.name == std::ffi::OsString::from(name.as_str())));
    assert_eq!(source_after, original_source);
    assert_eq!(
        (
            root_after.dev(),
            root_after.ino(),
            root_after.uid(),
            root_after.mode()
        ),
        (
            original_root.dev(),
            original_root.ino(),
            original_root.uid(),
            original_root.mode()
        )
    );
    let observed = observed.expect("independent reopen must find the actual whole-file output");
    assert_eq!(observed.mode & 0o7777, 0o400);
    assert_eq!(observed.owner, original_root.uid());
    assert_eq!(observed.links, 1);
    assert_eq!(observed.length, FILE_TRANSFER_CHUNK_BYTES + 17);
    assert!(observed.same_visible_inode);
    observed
        .verified
        .expect("whole-file chunk order, exact EOF and digest must verify");
    assert_eq!(observed.observed_length, manifest.byte_length());
    assert!(observed.pattern_matches);
    assert_eq!(observed.digest, manifest.sha256().to_string());
}
