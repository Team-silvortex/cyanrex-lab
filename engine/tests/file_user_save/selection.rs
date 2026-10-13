use crate::{fixture::*, support::*, *};
use std::{
    fs,
    os::unix::fs::{symlink, MetadataExt},
    path::PathBuf,
};

#[tokio::test]
async fn exact_owned_700_750_755_targets_clone_original_policy_and_save_empty_or_short_explicit_names(
) {
    for bits in [0o700, 0o750, 0o755] {
        for length in [0, 17] {
            checked(Fixture::with_length(length), |f| {
                Box::pin(async move {
                    let assembled = f.assemble().await;
                    let selected = selected_directory(f);
                    mode(&selected, bits);
                    let before = directory_policy(&selected);
                    let source = bytes::tree(&f.source_root);
                    let leaf = if length == 0 {
                        "chosen-empty.bin".to_owned()
                    } else {
                        "界".repeat(85)
                    };
                    assert!(leaf.len() <= 255);
                    let target = FileSaveTarget::open_user_directory(&selected, &leaf).unwrap();
                    let clone = target.clone();
                    drop(target);
                    assert!(
                        bytes::tree(&selected).is_empty(),
                        "construction/cloning reserves no leaf"
                    );
                    assert_eq!(
                        save(f, &assembled, &clone).await.unwrap(),
                        FileVersionRef::from_manifest(&f.manifest)
                    );
                    let saved = assert_output(f, &selected.join(&leaf));
                    assert_ne!(
                        saved,
                        observe(&f.source_path(), &f.manifest).unwrap().identity
                    );
                    assert_eq!(
                        directory_policy(&selected),
                        before,
                        "original dev/inode/uid/gid/fullmode stay exact"
                    );
                    assert_eq!(before.4, bits);
                    assert_eq!(bytes::tree(&f.source_root), source);
                    assert_eq!(bytes::tree(&selected).len(), 1);
                    assert!(!selected.join(f.manifest.display_name()).exists());
                    assert!(!selected.join(CHOSEN_LEAF).exists());
                })
            })
            .await;
        }
    }
}

#[tokio::test]
async fn user_constructor_rejects_bad_leaf_nonexistent_non_directory_symlink_unsafe_ancestor_and_non_allowlisted_modes_without_repairs(
) {
    checked(Fixture::with_length(17), |f| {
        Box::pin(async move {
            let selected = selected_directory(f);
            let missing = f.directory().join("missing");
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
                for parent in [&selected, &missing] {
                    assert_eq!(
                        FileSaveTarget::open_user_directory(parent, &leaf).err(),
                        Some(FileSaveError::InvalidInput)
                    );
                }
            }
            let child = selected.join("child");
            directory(&child, 0o750);
            let alias = f.directory().join("parent-alias");
            symlink(&selected, &alias).unwrap();
            let marker = f.directory().join("regular-parent");
            write_private(&marker, b"inert marker");
            let before = bytes::tree(f.directory());
            for path in [
                PathBuf::from("relative"),
                PathBuf::from("/"),
                missing.clone(),
                alias.clone(),
                alias.join("child"),
                marker.clone(),
                marker.join("child"),
                selected.join("..").join("ordinary-selected-parent"),
                PathBuf::from(format!("/{}", "p".repeat(4096))),
            ] {
                assert_eq!(
                    FileSaveTarget::open_user_directory(&path, CHOSEN_LEAF).err(),
                    Some(FileSaveError::Unavailable)
                );
                assert_eq!(bytes::tree(f.directory()), before);
            }
            for bits in [
                0o000, 0o500, 0o600, 0o710, 0o744, 0o754, 0o775, 0o770, 0o702, 0o777, 0o1700,
                0o1755, 0o2750, 0o4755,
            ] {
                mode(&selected, bits);
                let snapshot = bytes::snapshot(&selected);
                let result = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).err();
                let after = bytes::snapshot(&selected);
                mode(&selected, 0o755);
                assert_eq!(
                    result,
                    Some(FileSaveError::Unavailable),
                    "only exact ordinary 700/750/755 modes are allowed"
                );
                assert_eq!(after, snapshot);
            }
            mode(f.directory(), 0o770);
            let ancestor = bytes::snapshot(f.directory());
            let result = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).err();
            let after = bytes::snapshot(f.directory());
            mode(f.directory(), 0o700);
            assert_eq!(result, Some(FileSaveError::Unavailable));
            assert_eq!(after, ancestor);
            // Existing differently-owned directories are read-only probes, never changed by this test.
            let outside = PathBuf::from("/usr");
            if fs::metadata(&outside).unwrap().uid() != directory_policy(&selected).2 {
                assert_eq!(
                    FileSaveTarget::open_user_directory(&outside, CHOSEN_LEAF).err(),
                    Some(FileSaveError::Unavailable)
                );
            }
            let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
            drop(target);
            assert!(!missing.exists());
            assert_eq!(bytes::tree(&selected).len(), 1);
            assert!(!selected.join(CHOSEN_LEAF).exists());
        })
    })
    .await;
}

#[tokio::test]
async fn ordinary_target_opt_in_never_relaxes_old_private_constructor_staging_or_original_scope_and_source_root_binding(
) {
    checked(Fixture::with_length(17), |f| {
        Box::pin(async move {
            let assembled = f.assemble().await;
            let selected = selected_directory(f);
            for bits in [0o750, 0o755] {
                mode(&selected, bits);
                let before = bytes::snapshot(&selected);
                assert_eq!(
                    FileSaveTarget::open(&selected, CHOSEN_LEAF).err(),
                    Some(FileSaveError::Unavailable)
                );
                assert_eq!(
                    ChunkStaging::open(scope(), &selected, 1).err(),
                    Some(ChunkStagingError::Unavailable)
                );
                assert_eq!(bytes::snapshot(&selected), before);
            }
            let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
            let same_root =
                FileSaveTarget::open_user_directory(&f.source_root, "never-create.bin").unwrap();
            let source = bytes::tree(&f.source_root);
            assert_eq!(
                save(f, &assembled, &same_root).await.err(),
                Some(FileSaveError::InvalidInput)
            );
            for authority in [false, true] {
                let mut wrong = scope();
                if authority {
                    wrong.authority_id = Uuid::from_u128(0xab).try_into().unwrap();
                } else {
                    wrong.workspace_id = Uuid::from_u128(0xab).try_into().unwrap();
                }
                let manifest = changed(&f.manifest, "workspace", serde_json::json!(wrong));
                assert_eq!(
                    within(f.store.save_assembled_file(&manifest, &assembled, &target))
                        .await
                        .err(),
                    Some(FileSaveError::InvalidInput)
                );
            }
            assert_eq!(bytes::tree(&f.source_root), source);
            assert!(bytes::tree(&selected).is_empty());
            save(f, &assembled, &target).await.unwrap();
            assert_output(f, &selected.join(CHOSEN_LEAF));
            assert_eq!(bytes::tree(&f.source_root), source);
            assert_eq!(directory_policy(&f.source_root).4, 0o700);
        })
    })
    .await;
}
