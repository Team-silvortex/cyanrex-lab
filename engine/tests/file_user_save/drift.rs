use crate::{fixture::*, support::*, *};
use std::{fs, os::unix::fs::symlink};

#[tokio::test]
async fn original_user_target_rejects_safe_to_safe_or_unsafe_mode_drift_before_create_and_restores_only_original_policy(
) {
    for changed_mode in [0o750, 0o700, 0o775, 0o1755] {
        checked(Fixture::with_length(17), |f| Box::pin(async move {
            let assembled = f.assemble().await;
            let selected = selected_directory(f);
            let original = directory_policy(&selected);
            let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
            let source = bytes::tree(&f.source_root);
            mode(&selected, changed_mode);
            let drifted = bytes::snapshot(&selected);
            assert_eq!(save(f, &assembled, &target.clone()).await.err(), Some(FileSaveError::Unavailable),
                "currently safe is insufficient: this capability retains its original exact mode");
            assert_eq!(bytes::snapshot(&selected), drifted);
            assert!(bytes::tree(&selected).is_empty());
            assert_eq!(bytes::tree(&f.source_root), source);
            mode(&selected, 0o755);
            assert_eq!(directory_policy(&selected), original, "uid/gid are observed unchanged; no privileged ownership mutation is claimed");
            save(f, &assembled, &target).await.unwrap();
            assert_output(f, &selected.join(CHOSEN_LEAF));
            assert_eq!(directory_policy(&selected), original);
            assert_eq!(bytes::tree(&f.source_root), source);
        })).await;
    }
}

#[tokio::test]
async fn original_user_target_root_replacement_symlink_or_new_unsafe_ancestor_never_writes_old_or_new_parent_and_restores_healthily(
) {
    for attack in 0..3 {
        checked(Fixture::with_length(17), |f| {
            Box::pin(async move {
                let assembled = f.assemble().await;
                let selected = selected_directory(f);
                let original = directory_policy(&selected);
                let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
                let retained = f.directory().join("retained-selected-parent");
                let outside = f.directory().join("other-ordinary-parent");
                directory(&outside, 0o755);
                if attack < 2 {
                    fs::rename(&selected, &retained).unwrap();
                    if attack == 0 {
                        directory(&selected, 0o755);
                    } else {
                        symlink(&outside, &selected).unwrap();
                    }
                    assert_ne!(directory_policy(&selected).1, original.1);
                } else {
                    mode(f.directory(), 0o770);
                }
                let source = bytes::tree(&f.source_root);
                let selected_before = bytes::snapshot(&selected);
                let retained_before = (attack < 2).then(|| bytes::snapshot(&retained));
                let outside_before = bytes::snapshot(&outside);
                assert_eq!(
                    save(f, &assembled, &target).await.err(),
                    Some(FileSaveError::Unavailable)
                );
                assert_eq!(bytes::snapshot(&selected), selected_before);
                assert_eq!(bytes::snapshot(&outside), outside_before);
                assert!(bytes::tree(&selected).is_empty() && bytes::tree(&outside).is_empty());
                if let Some(before) = retained_before {
                    assert_eq!(bytes::snapshot(&retained), before);
                    assert!(bytes::tree(&retained).is_empty());
                }
                assert_eq!(bytes::tree(&f.source_root), source);
                match attack {
                    0 => fs::remove_dir(&selected).unwrap(),
                    1 => fs::remove_file(&selected).unwrap(),
                    _ => mode(f.directory(), 0o700),
                }
                if attack < 2 {
                    fs::rename(&retained, &selected).unwrap();
                }
                assert_eq!(directory_policy(&selected), original);
                save(f, &assembled, &target).await.unwrap();
                assert_output(f, &selected.join(CHOSEN_LEAF));
                assert_eq!(bytes::tree(&f.source_root), source);
                assert!(bytes::tree(&outside).is_empty());
            })
        })
        .await;
    }
}
