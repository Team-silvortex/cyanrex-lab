use crate::{fixture::*, *};

#[tokio::test]
async fn actual_original_cross_chunk_assembly_saves_to_explicit_new_host_file_and_preserves_private_source(
) {
    let (
        result,
        manifest,
        original_ack,
        source,
        saved,
        before,
        after,
        source_root_before,
        source_root_after,
        target_root_before,
        target_root_after,
        target_before,
        target_after,
    ) = checked(Fixture::new(), |f| {
        Box::pin(async move {
            let assembled = f.assemble().await;
            let original_ack = assembled.file().clone();
            let before = bytes::tree(&f.source_root);
            let source_root_before = bytes::snapshot(&f.source_root);
            let target_root_before = root_identity(&f.target_root);
            let target_before = bytes::tree(&f.target_root);
            let source = observe(&f.source_path(), &f.manifest);
            let result = within(
                f.store
                    .save_assembled_file(&f.manifest, &assembled, &f.target),
            )
            .await
            .map(|saved| saved.file().clone());
            let saved = observe(&f.target_path(), &f.manifest);
            let after = bytes::tree(&f.source_root);
            let source_root_after = bytes::snapshot(&f.source_root);
            let target_root_after = root_identity(&f.target_root);
            let target_after = bytes::tree(&f.target_root);
            drop(assembled);
            (
                result,
                f.manifest.clone(),
                original_ack,
                source,
                saved,
                before,
                after,
                source_root_before,
                source_root_after,
                target_root_before,
                target_root_after,
                target_before,
                target_after,
            )
        })
    })
    .await;
    let saved_version = result.expect("actual save must copy and acknowledge the original private assembled file, not a placeholder");
    let expected = FileVersionRef::from_manifest(&manifest);
    assert_eq!(original_ack, expected);
    assert_eq!(saved_version, expected);
    assert_eq!(manifest.byte_length(), FILE_TRANSFER_CHUNK_BYTES + 17);
    assert_eq!(manifest.chunk_sha256().len(), 2);
    assert_ne!(
        manifest.chunk(0).unwrap().sha256,
        manifest.chunk(1).unwrap().sha256
    );
    assert_ne!(
        CHOSEN_LEAF,
        manifest.display_name(),
        "trusted host selection is not remote display metadata"
    );
    let source = source.expect("independent reopen must find actual original f1 source");
    let saved =
        saved.expect("independent reopen must find the explicitly selected actual saved file");
    assert_ne!(
        saved.identity, source.identity,
        "copy creates a distinct inode, never a rename or hardlink"
    );
    for observed in [source, saved] {
        assert_eq!(observed.mode & 0o7777, 0o400);
        assert_eq!(observed.owner, target_root_before.2);
        assert_eq!(observed.links, 1);
        assert_eq!(observed.length, manifest.byte_length());
        assert!(observed.same_visible_inode);
        observed
            .verified
            .expect("independent every-chunk/whole SHA and exact EOF verification");
        assert_eq!(observed.observed_length, FILE_TRANSFER_CHUNK_BYTES + 17);
        assert!(observed.pattern_matches);
        assert_eq!(observed.digest, manifest.sha256().to_string());
    }
    assert_eq!(
        before.len(),
        3,
        "actual source contains two confirmed chunks plus its original whole output"
    );
    assert_eq!(
        after, before,
        "save cannot mutate source chunk/f1 inode, bytes, mode, links or timestamps"
    );
    assert_eq!(source_root_after, source_root_before);
    assert_eq!(source_root_before.mode & 0o7777, 0o700);
    assert_eq!(
        target_root_after, target_root_before,
        "selected parent retains its original identity, owner and mode"
    );
    assert_eq!(target_root_before.3 & 0o7777, 0o700);
    assert_ne!(
        (source_root_before.device, source_root_before.inode),
        (target_root_before.0, target_root_before.1)
    );
    assert!(target_before.is_empty());
    assert_eq!(
        target_after.len(),
        1,
        "only the host-selected new output is created"
    );
    assert_eq!(target_after[0].name, std::ffi::OsString::from(CHOSEN_LEAF));
    assert_eq!(target_after[0].mode & 0o7777, 0o400);
    assert_eq!(target_after[0].links, 1);
    assert_eq!(target_after[0].length, manifest.byte_length());
    assert_eq!(
        target_after[0].digest.as_deref(),
        Some(manifest.sha256().as_str())
    );
}
