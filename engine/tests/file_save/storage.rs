use crate::{fixture::*, support::*, *};
use std::{
    ffi::CString,
    fs::{self, OpenOptions},
    os::unix::{
        ffi::OsStrExt,
        fs::{symlink, MetadataExt, OpenOptionsExt},
    },
    time::Duration,
};

#[tokio::test]
async fn original_source_replacement_unlink_permissions_length_digest_and_special_entry_drift_never_create_target(
) {
    for attack in 0..10 {
        checked(Fixture::with_length(17), |f| Box::pin(async move {
            let assembled = f.assemble().await;
            let path = f.source_path();
            let original_identity = observe(&path, &f.manifest).unwrap().identity;
            let held = f.directory().join("retained-original-source");
            let marker = f.directory().join("outside-marker");
            write_private(&marker, b"untouched outside marker");
            match attack {
                0 => {
                    fs::rename(&path, &held).unwrap();
                    fs::copy(&held, &path).unwrap();
                    mode(&path, 0o400);
                    let replacement = observe(&path, &f.manifest).unwrap();
                    replacement.verified.unwrap();
                    assert_ne!(replacement.identity, original_identity,
                        "equal valid bytes at a replacement inode cannot recover an original assembly ACK");
                }
                1 => fs::remove_file(&path).unwrap(),
                2 => mode(&path, 0o600),
                3 | 4 => {
                    mode(&path, 0o600);
                    OpenOptions::new().write(true).open(&path).unwrap().set_len(if attack == 3 { 16 } else { 18 }).unwrap();
                    mode(&path, 0o400);
                }
                5 => {
                    mode(&path, 0o600);
                    fs::write(&path, vec![255; 17]).unwrap();
                    mode(&path, 0o400);
                    assert!(observe(&path, &f.manifest).unwrap().verified.is_err());
                }
                6 => fs::hard_link(&path, f.directory().join("second-source-link")).unwrap(),
                7 | 8 | 9 => {
                    fs::rename(&path, &held).unwrap();
                    match attack {
                        7 => symlink(&marker, &path).unwrap(),
                        8 => {
                            let path = CString::new(path.as_os_str().as_bytes()).unwrap();
                            // SAFETY: NUL-terminated path belongs only to this exclusive fixture.
                            assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
                        }
                        _ => private_directory(&path),
                    }
                }
                _ => unreachable!(),
            }
            let marker_before = bytes::snapshot(&marker);
            let retained_before = held.exists().then(|| bytes::snapshot(&held));
            denied(f, &assembled, FileSaveError::Unavailable).await;
            assert_eq!(bytes::snapshot(&marker), marker_before);
            if let Some(before) = retained_before { assert_eq!(bytes::snapshot(&held), before); }
            assert!(bytes::tree(&f.target_root).is_empty());
            // A changed proof cannot be restored by matching bytes. A separate real assembly
            // supplies the healthy worker/control instead of adopting the damaged original.
            let control = bytes::manifest(17, 2);
            let control_ack = f.assemble_manifest(&control).await;
            let saved = within(f.store.save_assembled_file(&control, &control_ack, &f.target)).await.unwrap();
            assert_eq!(saved.file(), &FileVersionRef::from_manifest(&control));
            assert_output(f, &f.target_path(), &control);
        })).await;
    }
}

#[tokio::test]
async fn source_root_replacement_or_permission_drift_is_unavailable_without_repair_and_original_root_restoration_is_healthy(
) {
    for replace in [false, true] {
        checked(Fixture::with_length(17), |f| {
            Box::pin(async move {
                let assembled = f.assemble().await;
                let retained = f.directory().join("retained-source-root");
                if replace {
                    fs::rename(&f.source_root, &retained).unwrap();
                    private_directory(&f.source_root);
                    fs::copy(retained.join(output_name(&f.manifest)), f.source_path()).unwrap();
                    mode(&f.source_path(), 0o400);
                    observe(&f.source_path(), &f.manifest)
                        .unwrap()
                        .verified
                        .unwrap();
                    assert_ne!(root_identity(&f.source_root).1, root_identity(&retained).1);
                } else {
                    mode(&f.source_root, 0o770);
                }
                let old_entries = replace.then(|| bytes::tree(&retained));
                denied(f, &assembled, FileSaveError::Unavailable).await;
                if let Some(before) = old_entries {
                    assert_eq!(bytes::tree(&retained), before);
                }
                assert!(bytes::tree(&f.target_root).is_empty());
                if replace {
                    fs::remove_file(f.source_path()).unwrap();
                    fs::remove_dir(&f.source_root).unwrap();
                    fs::rename(&retained, &f.source_root).unwrap();
                } else {
                    mode(&f.source_root, 0o700);
                }
                let source = bytes::tree(&f.source_root);
                assert_eq!(
                    save(f, &assembled, &f.target).await.unwrap(),
                    FileVersionRef::from_manifest(&f.manifest)
                );
                assert_output(f, &f.target_path(), &f.manifest);
                assert_eq!(bytes::tree(&f.source_root), source);
            })
        })
        .await;
    }
}

#[tokio::test]
async fn selected_parent_replacement_or_permission_drift_never_writes_original_or_new_parent_and_restores_cleanly(
) {
    for replace in [false, true] {
        checked(Fixture::with_length(17), |f| {
            Box::pin(async move {
                let assembled = f.assemble().await;
                let retained = f.directory().join("retained-selected-parent");
                let original = root_identity(&f.target_root);
                if replace {
                    fs::rename(&f.target_root, &retained).unwrap();
                    private_directory(&f.target_root);
                    assert_ne!(root_identity(&f.target_root).1, original.1);
                } else {
                    mode(&f.target_root, 0o755);
                }
                let retained_before = replace.then(|| bytes::snapshot(&retained));
                denied(f, &assembled, FileSaveError::Unavailable).await;
                assert!(bytes::tree(&f.target_root).is_empty());
                if let Some(before) = retained_before {
                    assert_eq!(bytes::snapshot(&retained), before);
                    assert!(bytes::tree(&retained).is_empty());
                }
                if replace {
                    fs::remove_dir(&f.target_root).unwrap();
                    fs::rename(&retained, &f.target_root).unwrap();
                } else {
                    mode(&f.target_root, 0o700);
                }
                assert_eq!(root_identity(&f.target_root), original);
                assert_eq!(
                    save(f, &assembled, &f.target.clone()).await.unwrap(),
                    FileVersionRef::from_manifest(&f.manifest)
                );
                assert_output(f, &f.target_path(), &f.manifest);
            })
        })
        .await;
    }
}

#[tokio::test]
async fn valid_original_proof_never_overwrites_regular_empty_directory_symlink_dangling_hardlink_or_fifo_target(
) {
    for attack in 0..7 {
        checked(Fixture::with_length(17), |f| {
            Box::pin(async move {
                let assembled = f.assemble().await;
                let path = f.target_path();
                let marker = f.directory().join("outside-existing-marker");
                write_private(&marker, b"existing bytes are not adopted");
                match attack {
                    0 => write_private(&path, b"already present"),
                    1 => write_private(&path, b""),
                    2 => private_directory(&path),
                    3 => symlink(&marker, &path).unwrap(),
                    4 => symlink(f.directory().join("does-not-exist"), &path).unwrap(),
                    5 => fs::hard_link(&marker, &path).unwrap(),
                    6 => {
                        let path = CString::new(path.as_os_str().as_bytes()).unwrap();
                        // SAFETY: this path is solely the selected entry in this private fixture.
                        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
                    }
                    _ => unreachable!(),
                }
                let before = bytes::tree(&f.target_root);
                let source = bytes::tree(&f.source_root);
                let marker_before = bytes::snapshot(&marker);
                let result = tokio::time::timeout(
                    Duration::from_secs(3),
                    f.store
                        .save_assembled_file(&f.manifest, &assembled, &f.target),
                )
                .await;
                // A deliberately regressed blocking FIFO open needs its own bounded rescue before
                // reporting failure, so no underlying file worker can strand this test's runtime.
                let rescue = if attack == 6 && result.is_err() {
                    let file = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .custom_flags(libc::O_NONBLOCK)
                        .open(&path)
                        .unwrap();
                    restored(f).await;
                    Some(file)
                } else {
                    None
                };
                drop(rescue);
                assert_eq!(
                    result
                        .expect("existing entries must never block output creation")
                        .err(),
                    Some(FileSaveError::Conflict)
                );
                assert_eq!(bytes::tree(&f.target_root), before);
                assert_eq!(bytes::tree(&f.source_root), source);
                assert_eq!(bytes::snapshot(&marker), marker_before);
                assert!(!f.directory().join("does-not-exist").exists());
                assert_eq!(
                    fs::metadata(f.source_path()).unwrap().nlink(),
                    1,
                    "hardlink target attacks do not damage the valid source proof"
                );
            })
        })
        .await;
    }
}
