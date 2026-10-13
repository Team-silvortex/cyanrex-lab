use crate::{fixture::*, support::*, *};
use std::{os::unix::fs::symlink, path::PathBuf};

#[tokio::test]
async fn actual_empty_and_short_original_acks_save_to_explicit_host_names_with_private_output() {
    for length in [0, 17] {
        checked(Fixture::with_length(length), |f| {
            Box::pin(async move {
                let assembled = f.assemble().await;
                let source = bytes::tree(&f.source_root);
                let leaf = if length == 0 {
                    "empty-selected.bin".to_owned()
                } else {
                    "界".repeat(85)
                };
                assert!(leaf.len() <= 255);
                assert_ne!(leaf, f.manifest.display_name());
                let target = f.target_for(&leaf);
                let saved = save(f, &assembled, &target).await.unwrap();
                assert_eq!(saved, FileVersionRef::from_manifest(&f.manifest));
                let identity = assert_output(f, &f.target_root.join(&leaf), &f.manifest);
                let original = observe(&f.source_path(), &f.manifest).unwrap();
                assert_ne!(identity, original.identity);
                assert_eq!(bytes::tree(&f.source_root), source);
                assert_eq!(bytes::tree(&f.target_root).len(), 1);
                assert!(
                    !f.target_path().exists(),
                    "opening a target never creates a default output"
                );
                assert!(!f.target_root.join(f.manifest.display_name()).exists());
            })
        })
        .await;
    }
}

#[tokio::test]
async fn target_basename_is_host_selected_utf8_byte_bounded_without_creation_or_remote_normalization(
) {
    checked(Fixture::with_length(17), |f| {
        Box::pin(async move {
            let before = bytes::tree(f.directory());
            let destination = bytes::tree(&f.target_root);
            let missing = f.directory().join("missing-parent");
            for leaf in [
                "".to_owned(),
                ".".to_owned(),
                "..".to_owned(),
                "../escape".to_owned(),
                "/absolute".to_owned(),
                "a/b".to_owned(),
                "a\\b".to_owned(),
                "nul\0byte".to_owned(),
                "newline\nbyte".to_owned(),
                "control\u{7f}".to_owned(),
                "x".repeat(256),
                "界".repeat(86),
            ] {
                for parent in [&f.target_root, &missing] {
                    assert_eq!(
                        FileSaveTarget::open(parent, &leaf).err(),
                        Some(FileSaveError::InvalidInput),
                        "invalid leaf is rejected without visiting even a missing parent"
                    );
                }
            }
            for leaf in [
                "a".to_owned(),
                "x".repeat(255),
                "界".repeat(85),
                "host:name.".to_owned(),
            ] {
                let target = FileSaveTarget::open(&f.target_root, &leaf).unwrap();
                drop(target.clone());
                drop(target);
            }
            assert_eq!(bytes::tree(f.directory()), before);
            assert_eq!(bytes::tree(&f.target_root), destination);
            assert!(!missing.exists());
        })
    })
    .await;
}

#[tokio::test]
async fn target_parent_requires_existing_private_original_no_follow_directory_and_never_repairs_it()
{
    checked(Fixture::with_length(17), |f| {
        Box::pin(async move {
            let alias = f.directory().join("parent-alias");
            symlink(&f.target_root, &alias).unwrap();
            let child = f.target_root.join("child");
            private_directory(&child);
            let regular = f.directory().join("not-a-directory");
            write_private(&regular, b"inert parent marker");
            let paths = [
                PathBuf::from("relative-parent"),
                f.directory().join("missing"),
                alias.clone(),
                alias.join("child"),
                regular.clone(),
                regular.join("child"),
                f.source_root.join("..").join("selected-host-parent"),
                PathBuf::from(format!("/{}", "p".repeat(4096))),
            ];
            let before = bytes::tree(f.directory());
            let destination = bytes::tree(&f.target_root);
            for path in paths {
                assert_eq!(
                    FileSaveTarget::open(&path, CHOSEN_LEAF).err(),
                    Some(FileSaveError::Unavailable)
                );
                assert_eq!(bytes::tree(f.directory()), before);
                assert_eq!(bytes::tree(&f.target_root), destination);
            }
            for bits in [0o755, 0o770, 0o1700] {
                mode(&f.target_root, bits);
                let drifted = bytes::snapshot(&f.target_root);
                assert_eq!(
                    FileSaveTarget::open(&f.target_root, CHOSEN_LEAF).err(),
                    Some(FileSaveError::Unavailable)
                );
                assert_eq!(bytes::snapshot(&f.target_root), drifted);
                mode(&f.target_root, 0o700);
            }
            mode(f.directory(), 0o777);
            let ancestor = bytes::snapshot(f.directory());
            assert_eq!(
                FileSaveTarget::open(&f.target_root, CHOSEN_LEAF).err(),
                Some(FileSaveError::Unavailable)
            );
            assert_eq!(bytes::snapshot(f.directory()), ancestor);
            mode(f.directory(), 0o700);
            let target = FileSaveTarget::open(&f.target_root, CHOSEN_LEAF).unwrap();
            drop(target);
            assert_eq!(bytes::tree(&f.target_root), destination);
            assert!(!f.target_path().exists());
        })
    })
    .await;
}
