use super::super::{save_io::Phase, work_save};
use super::{test_support::*, *};
use std::{
    fs::{self, OpenOptions, Permissions},
    io::Write,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

const BYTES: &[u8] = b"seventeen bytes!!";
fn replacement(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.set_permissions(Permissions::from_mode(0o400)).unwrap();
    file.sync_all().unwrap();
}

#[test]
fn save_rejects_source_same_bytes_replacement_after_created_and_keeps_candidate_evidence() {
    let source = Root::new();
    let destination = Root::new();
    let runtime = runtime();
    let (fixture, assembled) = Fixture::ready(&runtime, &source, &destination, BYTES, 1);
    let target = fixture.target("source-substitution.bin");
    let source_path = output(&source, &fixture.manifest);
    let retained = source.0.join("retained-original-f1");
    let original = fs::metadata(&source_path).unwrap();
    let changed = Arc::new(AtomicBool::new(false));
    let observed = changed.clone();
    let expected = source_path.clone();
    let old = retained.clone();
    let result = runtime.block_on(work_save::run_with(
        &fixture.staging.inner,
        &fixture.manifest,
        assembled.proof(),
        &target.inner,
        move |phase| {
            if phase == Phase::Created {
                fs::rename(&expected, &old).unwrap();
                replacement(&expected, BYTES);
                assert_ne!(
                    fs::metadata(&expected).unwrap().ino(),
                    fs::metadata(&old).unwrap().ino()
                );
                observed.store(true, Ordering::SeqCst);
            }
        },
    ));
    assert!(matches!(result, Err(FileSaveError::Unconfirmed)));
    assert!(changed.load(Ordering::SeqCst));
    assert_eq!(fs::metadata(&retained).unwrap().ino(), original.ino());
    assert_eq!(fs::read(&retained).unwrap(), BYTES);
    assert_eq!(fs::read(&source_path).unwrap(), BYTES);
    assert_eq!(
        fs::read(fixture.path("source-substitution.bin")).unwrap(),
        BYTES
    );
    assert_eq!(fs::read_dir(&destination.0).unwrap().count(), 1);
    // A matching replacement cannot revive the original acknowledgement on another explicit target.
    assert!(matches!(
        runtime.block_on(fixture.staging.save_assembled_file(
            &fixture.manifest,
            &assembled,
            &fixture.target("not-adopted.bin")
        )),
        Err(FileSaveError::Unavailable)
    ));
    assert!(!fixture.path("not-adopted.bin").exists());
}

#[test]
fn save_final_readback_rejects_same_bytes_new_target_inode_without_adoption() {
    let source = Root::new();
    let destination = Root::new();
    let runtime = runtime();
    let (fixture, assembled) = Fixture::ready(&runtime, &source, &destination, BYTES, 1);
    let target = fixture.target("target-substitution.bin");
    let path = fixture.path("target-substitution.bin");
    let retained = fixture.path("retained-original-target.bin");
    let expected = path.clone();
    let old = retained.clone();
    let original_inode = Arc::new(std::sync::Mutex::new(None));
    let observed = original_inode.clone();
    let result = runtime.block_on(work_save::run_with(
        &fixture.staging.inner,
        &fixture.manifest,
        assembled.proof(),
        &target.inner,
        move |phase| {
            if phase == Phase::Readback {
                *observed.lock().unwrap() = Some(fs::metadata(&expected).unwrap().ino());
                fs::rename(&expected, &old).unwrap();
                replacement(&expected, BYTES);
            }
        },
    ));
    assert!(matches!(result, Err(FileSaveError::Unconfirmed)));
    let original_inode = original_inode.lock().unwrap().unwrap();
    assert_eq!(fs::metadata(&retained).unwrap().ino(), original_inode);
    assert_ne!(fs::metadata(&path).unwrap().ino(), original_inode);
    assert_eq!(fs::read(&retained).unwrap(), BYTES);
    assert_eq!(fs::read(&path).unwrap(), BYTES);
    assert_eq!(fs::read_dir(&destination.0).unwrap().count(), 2);
    fixture.unchanged_source();
    assert!(matches!(
        runtime.block_on(fixture.staging.save_assembled_file(
            &fixture.manifest,
            &assembled,
            &target
        )),
        Err(FileSaveError::Conflict)
    ));
}

#[test]
fn save_created_source_or_destination_root_replacement_remains_unconfirmed_and_retained() {
    for replace_source in [true, false] {
        let source = Root::new();
        let destination = Root::new();
        let retired = Root::new();
        let runtime = runtime();
        let (fixture, assembled) = Fixture::ready(&runtime, &source, &destination, BYTES, 1);
        let target = fixture.target("root-drift.bin");
        let chosen = if replace_source {
            source.0.clone()
        } else {
            destination.0.clone()
        };
        let retained = retired.0.join("original-parent");
        let f1_name = output(&source, &fixture.manifest)
            .file_name()
            .unwrap()
            .to_owned();
        let f1 = f1_name.clone();
        let original = chosen.clone();
        let moved = retained.clone();
        let result = runtime.block_on(work_save::run_with(
            &fixture.staging.inner,
            &fixture.manifest,
            assembled.proof(),
            &target.inner,
            move |phase| {
                if phase == Phase::Created {
                    fs::rename(&original, &moved).unwrap();
                    fs::DirBuilder::new().mode(0o700).create(&original).unwrap();
                    if replace_source {
                        replacement(&original.join(&f1), BYTES);
                    }
                }
            },
        ));
        assert!(matches!(result, Err(FileSaveError::Unconfirmed)));
        if replace_source {
            assert_eq!(fs::read(retained.join(&f1_name)).unwrap(), BYTES);
            assert_eq!(fs::read(source.0.join(&f1_name)).unwrap(), BYTES);
            assert_ne!(
                fs::metadata(retained.join(&f1_name)).unwrap().ino(),
                fs::metadata(source.0.join(&f1_name)).unwrap().ino()
            );
            assert_eq!(fs::read(fixture.path("root-drift.bin")).unwrap(), BYTES);
        } else {
            assert_eq!(fs::read(retained.join("root-drift.bin")).unwrap(), BYTES);
            assert_eq!(fs::read_dir(&destination.0).unwrap().count(), 0);
        }
        assert_ne!(
            fs::metadata(&chosen).unwrap().ino(),
            fs::metadata(&retained).unwrap().ino()
        );
        assert_eq!(fixture.staging.inner.workers.available_permits(), 1);
    }
}

#[test]
fn save_actual_target_hash_readback_rejects_equal_length_corruption_before_written_snapshot() {
    let source = Root::new();
    let destination = Root::new();
    let runtime = runtime();
    let (fixture, assembled) = Fixture::ready(&runtime, &source, &destination, BYTES, 1);
    let target = fixture.target("corrupt-after-copy.bin");
    let path = fixture.path("corrupt-after-copy.bin");
    let expected = path.clone();
    let copied = Arc::new(AtomicBool::new(false));
    let copied_observer = copied.clone();
    let readback = Arc::new(AtomicBool::new(false));
    let readback_observer = readback.clone();
    let original_inode = Arc::new(std::sync::Mutex::new(None));
    let observed_inode = original_inode.clone();
    let result = runtime.block_on(work_save::run_with(
        &fixture.staging.inner,
        &fixture.manifest,
        assembled.proof(),
        &target.inner,
        move |phase| {
            if phase == Phase::Copied {
                let metadata = fs::metadata(&expected).unwrap();
                assert_eq!(metadata.mode() & 0o7777, 0o600);
                assert_eq!(metadata.len(), BYTES.len() as u64);
                *observed_inode.lock().unwrap() = Some(metadata.ino());
                let mut file = OpenOptions::new().write(true).open(&expected).unwrap();
                file.write_all(&vec![b'x'; BYTES.len()]).unwrap();
                file.sync_all().unwrap();
                copied_observer.store(true, Ordering::SeqCst);
            }
            if phase == Phase::Readback {
                readback_observer.store(true, Ordering::SeqCst);
            }
        },
    ));
    assert!(matches!(result, Err(FileSaveError::Unconfirmed)));
    assert!(copied.load(Ordering::SeqCst));
    assert!(
        !readback.load(Ordering::SeqCst),
        "actual readback hashing must fail before its verified phase"
    );
    let metadata = fs::metadata(&path).unwrap();
    assert_eq!(metadata.ino(), original_inode.lock().unwrap().unwrap());
    assert_eq!(metadata.mode() & 0o7777, 0o400);
    assert_eq!(metadata.len(), BYTES.len() as u64);
    assert_eq!(fs::read(&path).unwrap(), vec![b'x'; BYTES.len()]);
    fixture.unchanged_source();
    assert!(matches!(
        runtime.block_on(fixture.staging.save_assembled_file(
            &fixture.manifest,
            &assembled,
            &target
        )),
        Err(FileSaveError::Conflict)
    ));
}
