use super::fixture::*;
use cyanrex_engine::services::file_transfer::{
    staging::{ChunkStaging, ChunkStagingError},
    FILE_TRANSFER_BUFFER_BYTES,
};
use std::{
    fs::{self, OpenOptions, Permissions},
    os::unix::{
        ffi::OsStrExt,
        fs::{symlink, OpenOptionsExt, PermissionsExt},
    },
    sync::atomic::Ordering,
    time::Duration,
};

#[test]
fn open_bounds_worker_capacity_and_never_creates_or_chmods_a_root() {
    let fixture = Fixture::new();
    let before = snapshot(&fixture.root);
    for workers in [0, 17, usize::MAX] {
        assert!(matches!(
            ChunkStaging::open(super::scope(), &fixture.root, workers),
            Err(ChunkStagingError::InvalidInput)
        ));
        assert_eq!(snapshot(&fixture.root), before);
    }
    assert!(ChunkStaging::open(super::scope(), &fixture.root, 16).is_ok());
    let missing = fixture.directory.join("missing");
    assert!(matches!(
        ChunkStaging::open(super::scope(), &missing, 1),
        Err(ChunkStagingError::Unavailable)
    ));
    assert!(!missing.exists());
    fs::set_permissions(&fixture.root, Permissions::from_mode(0o755)).unwrap();
    let changed = snapshot(&fixture.root);
    assert!(matches!(
        ChunkStaging::open(super::scope(), &fixture.root, 1),
        Err(ChunkStagingError::Unavailable)
    ));
    assert_eq!(snapshot(&fixture.root), changed);
    fs::set_permissions(&fixture.root, Permissions::from_mode(0o700)).unwrap();
    assert!(ChunkStaging::open(super::scope(), &fixture.root, 1).is_ok());
}

#[test]
fn root_paths_reject_relative_parent_traversal_symlink_and_non_directory_components() {
    let fixture = Fixture::new();
    let before = tree(&fixture.root);
    for path in [
        std::path::PathBuf::from("relative"),
        std::path::PathBuf::from("/"),
        fixture.root.join("../chunks"),
    ] {
        assert!(matches!(
            ChunkStaging::open(super::scope(), &path, 1),
            Err(ChunkStagingError::Unavailable)
        ));
    }
    let alias = fixture.directory.join("alias");
    symlink(&fixture.root, &alias).unwrap();
    assert!(ChunkStaging::open(super::scope(), &alias, 1).is_err());
    let nested = fixture.root.join("nested");
    private_directory(&nested);
    assert!(ChunkStaging::open(super::scope(), &alias.join("nested"), 1).is_err());
    fs::remove_dir(&nested).unwrap();
    let file = fixture.real_file(17);
    assert!(ChunkStaging::open(super::scope(), &file, 1).is_err());
    assert!(ChunkStaging::open(super::scope(), &file.join("child"), 1).is_err());
    assert_eq!(tree(&fixture.root), before);
    assert!(ChunkStaging::open(super::scope(), &fixture.root, 1).is_ok());
}

#[tokio::test]
async fn writable_parent_or_changed_root_permissions_reject_existing_store_without_writes() {
    let fixture = Fixture::new();
    let expected = manifest(17, 30);
    for change_parent in [true, false] {
        let path = if change_parent {
            &fixture.directory
        } else {
            &fixture.root
        };
        fs::set_permissions(path, Permissions::from_mode(0o777)).unwrap();
        let root_before = tree(&fixture.root);
        let metadata_before = snapshot(path);
        assert!(ChunkStaging::open(super::scope(), &fixture.root, 1).is_err());
        assert_eq!(
            fixture
                .store
                .stage_chunk(&expected, 0, Pattern::new(0, 17))
                .await,
            Err(ChunkStagingError::Unavailable)
        );
        assert_eq!(
            fixture.store.verify_staged_chunk(&expected, 0).await,
            Err(ChunkStagingError::Unavailable)
        );
        assert_eq!(tree(&fixture.root), root_before);
        assert_eq!(snapshot(path), metadata_before);
        fs::set_permissions(path, Permissions::from_mode(0o700)).unwrap();
    }
    fixture
        .store
        .stage_chunk(&expected, 0, Pattern::new(0, 17))
        .await
        .unwrap();
    fixture
        .store
        .verify_staged_chunk(&expected, 0)
        .await
        .unwrap();
}

#[tokio::test]
async fn existing_symbolic_hard_fifo_and_directory_entries_are_never_followed_or_overwritten() {
    let fixture = Fixture::new();
    let target = fixture.real_file(17);
    fs::set_permissions(&target, Permissions::from_mode(0o400)).unwrap();
    for attack in 0..4 {
        let expected = manifest(17, 40 + attack);
        let path = fixture.path(&expected, 0);
        match attack {
            0 => symlink(&target, &path).unwrap(),
            1 => fs::hard_link(&target, &path).unwrap(),
            2 => {
                let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
                // SAFETY: NUL-terminated path is confined to this test's private root.
                assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
            }
            _ => private_directory(&path),
        };
        let before = tree(&fixture.root);
        let target_before = snapshot(&target);
        let reader = Pattern::new(0, 17);
        let calls = reader.calls.clone();
        let stage = tokio::time::timeout(
            Duration::from_secs(1),
            fixture.store.stage_chunk(&expected, 0, reader),
        )
        .await;
        let verify = tokio::time::timeout(
            Duration::from_secs(1),
            fixture.store.verify_staged_chunk(&expected, 0),
        )
        .await;
        let mut rescue = None;
        if attack == 2 && verify.is_err() {
            // Normal coverage has no writer. Only rescue an incorrectly blocking FIFO open so
            // this deliberately failing case cannot strand the runtime's blocking worker.
            rescue = Some(
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .custom_flags(libc::O_NONBLOCK)
                    .open(&path)
                    .unwrap(),
            );
            fresh_stage_after_release(&fixture.store, &manifest(17, 1042)).await;
        }
        drop(rescue);
        assert_eq!(
            stage.expect("existing entry must not block staging"),
            Err(ChunkStagingError::Conflict)
        );
        assert_eq!(
            verify.expect("special-file verification must not block"),
            Err(ChunkStagingError::Unavailable)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(tree(&fixture.root), before);
        assert_eq!(snapshot(&target), target_before);
        if attack == 3 {
            fs::remove_dir(&path).unwrap();
        } else {
            fs::remove_file(&path).unwrap();
        }
        fixture
            .store
            .stage_chunk(&expected, 0, Pattern::new(0, 17))
            .await
            .unwrap();
        fixture
            .store
            .verify_staged_chunk(&expected, 0)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn stored_hardlink_and_symlink_drift_reject_with_healthy_restoration() {
    let fixture = Fixture::new();
    let expected = manifest(17, 50);
    fixture
        .store
        .stage_chunk(&expected, 0, Pattern::new(0, 17))
        .await
        .unwrap();
    let path = fixture.path(&expected, 0);
    let link = fixture.directory.join("second-link");
    fs::hard_link(&path, &link).unwrap();
    let before = tree(&fixture.root);
    assert_eq!(
        fixture.store.verify_staged_chunk(&expected, 0).await,
        Err(ChunkStagingError::Unavailable)
    );
    assert_eq!(tree(&fixture.root), before);
    fs::remove_file(&link).unwrap();
    fixture
        .store
        .verify_staged_chunk(&expected, 0)
        .await
        .unwrap();
    let saved = fixture.directory.join("saved-chunk");
    fs::rename(&path, &saved).unwrap();
    symlink(&saved, &path).unwrap();
    let before = tree(&fixture.root);
    let saved_before = snapshot(&saved);
    assert_eq!(
        fixture.store.verify_staged_chunk(&expected, 0).await,
        Err(ChunkStagingError::Unavailable)
    );
    assert_eq!(tree(&fixture.root), before);
    assert_eq!(snapshot(&saved), saved_before);
    fs::remove_file(&path).unwrap();
    fs::rename(&saved, &path).unwrap();
    let restored = tree(&fixture.root);
    fixture
        .store
        .verify_staged_chunk(&expected, 0)
        .await
        .unwrap();
    assert_eq!(tree(&fixture.root), restored);
}

#[tokio::test]
async fn replaced_root_before_dispatch_rejects_without_using_old_or_new_directory() {
    let fixture = Fixture::new();
    let expected = manifest(17, 51);
    fixture
        .store
        .stage_chunk(&expected, 0, Pattern::new(0, 17))
        .await
        .unwrap();
    let moved = fixture.directory.join("moved-root");
    fs::rename(&fixture.root, &moved).unwrap();
    private_directory(&fixture.root);
    let before = tree(&moved);
    let new_before = tree(&fixture.root);
    assert_eq!(
        fixture.store.verify_staged_chunk(&expected, 0).await,
        Err(ChunkStagingError::Unavailable)
    );
    let fresh = manifest(17, 52);
    assert_eq!(
        fixture
            .store
            .stage_chunk(&fresh, 0, Pattern::new(0, 17))
            .await,
        Err(ChunkStagingError::Unavailable)
    );
    assert_eq!(tree(&moved), before);
    assert_eq!(tree(&fixture.root), new_before);
    fs::remove_dir(&fixture.root).unwrap();
    fs::rename(&moved, &fixture.root).unwrap();
    fixture
        .store
        .verify_staged_chunk(&expected, 0)
        .await
        .unwrap();
    fixture
        .store
        .stage_chunk(&fresh, 0, Pattern::new(0, 17))
        .await
        .unwrap();
}

#[tokio::test]
async fn root_replacement_after_actual_partial_write_is_unconfirmed_and_retains_evidence() {
    let fixture = Fixture::new();
    let length = (FILE_TRANSFER_BUFFER_BYTES * 2 + 17) as u64;
    let expected = manifest(length, 53);
    let (reader, release, started, finished) = held(length, FILE_TRANSFER_BUFFER_BYTES as u64);
    let store = fixture.store.clone();
    let task_manifest = expected.clone();
    let waiting = tokio::spawn(async move { store.stage_chunk(&task_manifest, 0, reader).await });
    signal(started).await;
    let partial = snapshot(&fixture.path(&expected, 0));
    assert!(partial.length > 0 && partial.length < length);
    let moved = fixture.directory.join("moved-inflight-root");
    fs::rename(&fixture.root, &moved).unwrap();
    private_directory(&fixture.root);
    drop(release);
    assert_eq!(waiting.await.unwrap(), Err(ChunkStagingError::Unconfirmed));
    signal(finished).await;
    assert!(tree(&fixture.root).is_empty());
    let retained = snapshot(&moved.join(name(&expected, 0)));
    assert_eq!(retained.inode, partial.inode);
    assert!(retained.length >= partial.length && retained.length <= length);
    fs::remove_dir(&fixture.root).unwrap();
    fs::rename(&moved, &fixture.root).unwrap();
    let before = tree(&fixture.root);
    assert_eq!(
        fixture
            .store
            .stage_chunk(&expected, 0, Pattern::new(0, length))
            .await,
        Err(ChunkStagingError::Conflict)
    );
    assert_eq!(tree(&fixture.root), before);
    let fresh = manifest(17, 54);
    fresh_stage_after_release(&fixture.store, &fresh).await;
}

#[tokio::test]
async fn same_bytes_different_inode_substitution_cannot_acknowledge_original_write() {
    let fixture = Fixture::new();
    let length = (FILE_TRANSFER_BUFFER_BYTES * 2 + 17) as u64;
    let expected = manifest(length, 55);
    let (reader, release, started, finished) = held(length, FILE_TRANSFER_BUFFER_BYTES as u64);
    let store = fixture.store.clone();
    let pending = expected.clone();
    let waiting = tokio::spawn(async move { store.stage_chunk(&pending, 0, reader).await });
    signal(started).await;
    let path = fixture.path(&expected, 0);
    let original = snapshot(&path);
    assert!(original.length > 0 && original.length < length);
    let saved = fixture.directory.join("original-inflight-inode");
    fs::rename(&path, &saved).unwrap();
    let mut replacement = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    std::io::copy(&mut Pattern::new(0, length), &mut replacement).unwrap();
    fs::set_permissions(&path, Permissions::from_mode(0o400)).unwrap();
    let substituted = snapshot(&path);
    assert_ne!(substituted.inode, original.inode);
    assert_eq!(substituted.content.as_ref(), Some(expected.sha256()));
    drop(release);
    assert_eq!(waiting.await.unwrap(), Err(ChunkStagingError::Unconfirmed));
    signal(finished).await;
    let retained_original = snapshot(&saved);
    assert_eq!(retained_original.inode, original.inode);
    assert_eq!(retained_original.links, 1);
    assert_eq!(retained_original.length, length);
    assert_eq!(retained_original.content.as_ref(), Some(expected.sha256()));
    assert_eq!(snapshot(&path), substituted);
    // Current byte verification is not adoption or a retroactive write acknowledgement.
    fixture
        .store
        .verify_staged_chunk(&expected, 0)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .store
            .stage_chunk(&expected, 0, Pattern::new(0, length))
            .await,
        Err(ChunkStagingError::Conflict)
    );
    assert_eq!(snapshot(&path), substituted);
    fresh_stage_after_release(&fixture.store, &manifest(17, 56)).await;
}
