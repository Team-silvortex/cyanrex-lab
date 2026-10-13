use crate::{fixture::*, *};
use std::{fs, time::Duration};

#[tokio::test]
async fn actual_cross_chunk_original_ack_saves_new_file_to_explicit_owned_0755_directory_without_relaxing_source(
) {
    let (
        constructor,
        saved,
        manifest,
        original_ack,
        source,
        destination,
        chunks,
        source_before,
        source_after,
        source_root_before,
        source_root_after,
        selected_before,
        selected_after,
        selected_entries,
    ) = checked(Fixture::new(), |f| {
        Box::pin(async move {
            let assembled = f.assemble().await;
            let manifest = f.manifest.clone();
            let original_ack = assembled.file().clone();
            let selected = selected_directory(f);
            let source_before = bytes::tree(&f.source_root);
            let source_root_before = bytes::snapshot(&f.source_root);
            let selected_before = directory_policy(&selected);
            let (constructor, saved) =
                match FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF) {
                    Ok(target) => {
                        let saved = tokio::time::timeout(
                            Duration::from_secs(30),
                            f.store.save_assembled_file(&manifest, &assembled, &target),
                        )
                        .await
                        .map(|result| result.map(|saved| saved.file().clone()));
                        drop(target);
                        (Ok(()), Some(saved))
                    }
                    Err(error) => (Err(error), None),
                };
            // No constructor/save success is asserted until the original ACK, target and fixture
            // have released their descriptors and the exclusive fixture has been cleaned.
            drop(assembled);
            let source = observe(&f.source_path(), &manifest);
            let destination = observe(&selected.join(CHOSEN_LEAF), &manifest);
            let chunks = [0, 1].map(|index| {
                fs::File::open(f.source_root.join(chunk_name(&manifest, index))).map(|mut file| {
                    verify_chunk(&manifest, index, &mut file).map_err(|error| error.to_string())
                })
            });
            (
                constructor,
                saved,
                manifest,
                original_ack,
                source,
                destination,
                chunks,
                source_before,
                bytes::tree(&f.source_root),
                source_root_before,
                bytes::snapshot(&f.source_root),
                selected_before,
                directory_policy(&selected),
                bytes::tree(&selected),
            )
        })
    })
    .await;
    constructor.expect(
        "explicit owned 0755 target must open, not a placeholder or private-root-only policy",
    );
    let saved = saved
        .expect("a successful target constructor must invoke actual save")
        .expect("actual selected-file worker must finish within its bound")
        .expect("the original acknowledged assembly must save to this explicit ordinary directory");
    let expected = FileVersionRef::from_manifest(&manifest);
    assert_eq!(original_ack, expected);
    assert_eq!(saved, expected);
    assert_eq!(manifest.byte_length(), FILE_TRANSFER_CHUNK_BYTES + 17);
    assert_eq!(manifest.chunk_sha256().len(), 2);
    assert_ne!(
        manifest.chunk(0).unwrap().sha256,
        manifest.chunk(1).unwrap().sha256
    );
    assert_ne!(CHOSEN_LEAF, manifest.display_name());
    assert_eq!(
        source_before.len(),
        3,
        "two confirmed chunks and the private original f1"
    );
    assert_eq!(
        source_after, source_before,
        "all original chunk/f1 bytes, inodes and snapshots remain unchanged"
    );
    assert_eq!(source_root_after, source_root_before);
    assert_eq!(source_root_before.mode & 0o7777, 0o700);
    for chunk in chunks {
        chunk
            .expect("independent reopen must find a real source chunk")
            .expect("independent chunk SHA and exact EOF must verify");
    }
    let source = source.expect("independent reopen must find the original private f1");
    let destination = destination.expect("independent reopen must find the host-selected new file");
    assert_ne!(
        source.identity, destination.identity,
        "selected file is a new inode, not a rename or hardlink"
    );
    for observed in [source, destination] {
        observed
            .verified
            .expect("independent ordered chunk SHA, whole SHA and exact EOF");
        assert!(observed.same_visible_inode && observed.pattern_matches);
        assert_eq!(observed.mode & 0o7777, 0o400);
        assert_eq!(observed.owner, selected_before.2);
        assert_eq!(observed.links, 1);
        assert_eq!(observed.length, manifest.byte_length());
        assert_eq!(observed.observed_length, manifest.byte_length());
        assert_eq!(observed.digest, manifest.sha256().to_string());
    }
    assert_eq!(selected_before.4, 0o755);
    assert_eq!(
        selected_after, selected_before,
        "dev/inode/uid/gid/fullmode remain the selected policy; directory timestamps may change"
    );
    assert_ne!(
        (source_root_before.device, source_root_before.inode),
        (selected_before.0, selected_before.1)
    );
    assert_eq!(selected_entries.len(), 1);
    assert_eq!(
        selected_entries[0].name,
        std::ffi::OsString::from(CHOSEN_LEAF)
    );
    assert_eq!(selected_entries[0].mode & 0o7777, 0o400);
    assert_eq!(selected_entries[0].links, 1);
    assert_eq!(selected_entries[0].length, manifest.byte_length());
    assert_eq!(
        selected_entries[0].digest.as_deref(),
        Some(manifest.sha256().as_str())
    );
}
