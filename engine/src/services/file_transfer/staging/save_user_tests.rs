use super::super::ChunkStagingError;
use super::{test_support::*, *};
use std::{
    fs,
    future::Future,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    sync::{mpsc, Arc},
    task::Context,
    time::Duration,
};

#[test]
fn cancelled_public_user_save_retains_original_proof_and_real_queued_worker_to_completion() {
    let source = Root::new();
    let destination = Root::new();
    fs::set_permissions(&destination.0, fs::Permissions::from_mode(0o755)).unwrap();
    let runtime = runtime();
    let bytes = b"seventeen bytes!!";
    let (fixture, assembled) = Fixture::ready(&runtime, &source, &destination, bytes, 1);
    let target = FileSaveTarget::open_user_directory(&destination.0, "queued-user.bin").unwrap();
    let same_source = FileSaveTarget::open_user_directory(&source.0, "never-created.bin").unwrap();
    let weak = Arc::downgrade(assembled.proof());
    let held = Barrier::new();
    let release = ReleaseOnDrop(held.clone());
    let (entered, started) = mpsc::channel();
    let external = runtime.spawn_blocking(move || {
        entered.send(()).unwrap();
        held.wait();
    });
    started.recv_timeout(Duration::from_secs(10)).unwrap();
    let mut pending = Box::pin(fixture.staging.save_assembled_file(
        &fixture.manifest,
        &assembled,
        &target,
    ));
    let waker = waker();
    let mut context = Context::from_waker(&waker);
    {
        let _entered = runtime.enter();
        assert!(pending.as_mut().poll(&mut context).is_pending());
    }
    assert_eq!(fixture.staging.inner.workers.available_permits(), 0);
    assert_eq!(weak.strong_count(), 2);
    assert!(!fixture.path("queued-user.bin").exists());
    // Cached original-source identity rejects even with a full worker gate, without dispatch.
    assert!(matches!(
        runtime.block_on(fixture.staging.save_assembled_file(
            &fixture.manifest,
            &assembled,
            &same_source
        )),
        Err(FileSaveError::InvalidInput)
    ));
    assert!(!source.0.join("never-created.bin").exists());
    drop(pending);
    drop(assembled);
    assert_eq!(weak.strong_count(), 1);
    assert_eq!(
        runtime.block_on(fixture.staging.verify_staged_chunk(&fixture.manifest, 0)),
        Err(ChunkStagingError::CapacityExhausted)
    );
    release.0.release();
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(10), external)
            .await
            .unwrap()
            .unwrap();
    });
    restored(&runtime, &fixture.staging);
    proof_gone(&runtime, &weak);
    fixture.saved("queued-user.bin", bytes, None);
    assert_eq!(fs::metadata(&destination.0).unwrap().mode() & 0o7777, 0o755);
    assert_eq!(fs::read_dir(&destination.0).unwrap().count(), 1);
}

#[test]
fn user_save_created_safe_mode_or_parent_identity_drift_is_unconfirmed_and_keeps_evidence() {
    for replace in [false, true] {
        let source = Root::new();
        let destination = Root::new();
        let retained = Root::new();
        let replacements = Root::new();
        fs::set_permissions(&destination.0, fs::Permissions::from_mode(0o750)).unwrap();
        let runtime = runtime();
        let bytes = b"seventeen bytes!!";
        let (fixture, assembled) = Fixture::ready(&runtime, &source, &destination, bytes, 1);
        let mut target =
            FileSaveTarget::open_user_directory(&destination.0, "drift-user.bin").unwrap();
        let original_parent = destination.identity();
        let parent = destination.0.clone();
        let saved_parent = retained.0.join("original-parent");
        let old_parent = saved_parent.clone();
        let (created, observed) = mpsc::channel();
        target.set_test_observer(Arc::new(move |phase| {
            if phase == SaveTestPhase::Created {
                let candidate = fs::metadata(parent.join("drift-user.bin")).unwrap();
                assert_eq!((candidate.len(), candidate.mode() & 0o7777), (0, 0o600));
                created.send((candidate.dev(), candidate.ino())).unwrap();
                if replace {
                    fs::rename(&parent, &old_parent).unwrap();
                    fs::DirBuilder::new().mode(0o750).create(&parent).unwrap();
                    fs::write(parent.join("drift-user.bin"), bytes).unwrap();
                    fs::set_permissions(
                        parent.join("drift-user.bin"),
                        fs::Permissions::from_mode(0o400),
                    )
                    .unwrap();
                } else {
                    // Still an allowed mode: the original exact pin, not allowlist membership, rejects it.
                    fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
                }
            }
        }));
        let result = runtime.block_on(fixture.staging.save_assembled_file(
            &fixture.manifest,
            &assembled,
            &target,
        ));
        assert!(matches!(result, Err(FileSaveError::Unconfirmed)));
        let original = observed.recv_timeout(Duration::from_secs(10)).unwrap();
        let candidate_parent = if replace {
            &saved_parent
        } else {
            &destination.0
        };
        let candidate = candidate_parent.join("drift-user.bin");
        let metadata = fs::metadata(&candidate).unwrap();
        assert_eq!((metadata.dev(), metadata.ino()), original);
        assert_eq!(metadata.mode() & 0o7777, 0o400);
        assert_eq!(fs::read(&candidate).unwrap(), bytes);
        crate::services::file_transfer::verify_file(
            &fixture.manifest,
            &mut fs::File::open(&candidate).unwrap(),
        )
        .unwrap();
        if replace {
            let visible = fs::metadata(fixture.path("drift-user.bin")).unwrap();
            assert_ne!((visible.dev(), visible.ino()), original);
            assert_eq!(fs::read(fixture.path("drift-user.bin")).unwrap(), bytes);
            assert_ne!(destination.identity(), original_parent);
            fs::rename(&destination.0, replacements.0.join("replacement-parent")).unwrap();
            fs::rename(&saved_parent, &destination.0).unwrap();
        } else {
            assert_eq!(destination.identity().0, original_parent.0);
            assert_eq!(destination.identity().1, original_parent.1);
            fs::set_permissions(&destination.0, fs::Permissions::from_mode(0o750)).unwrap();
        }
        fixture.unchanged_source();
        let before = tree(&destination.0);
        assert!(
            matches!(
                runtime.block_on(fixture.staging.save_assembled_file(
                    &fixture.manifest,
                    &assembled,
                    &target
                )),
                Err(FileSaveError::Conflict)
            ),
            "retained evidence is not adopted after the trusted test restores its parent"
        );
        assert_eq!(tree(&destination.0), before);
        assert_eq!(fixture.staging.inner.workers.available_permits(), 1);
    }
}

#[test]
fn two_public_user_save_jobs_overlap_in_one_parent_without_freezing_entry_timestamps() {
    let source = Root::new();
    let destination = Root::new();
    fs::set_permissions(&destination.0, fs::Permissions::from_mode(0o750)).unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let bytes = b"seventeen bytes!!";
    let (fixture, assembled) = Fixture::ready(&runtime, &source, &destination, bytes, 2);
    let mut left = FileSaveTarget::open_user_directory(&destination.0, "user-left.bin").unwrap();
    let mut right = FileSaveTarget::open_user_directory(&destination.0, "user-right.bin").unwrap();
    let held = Barrier::new();
    let release = ReleaseOnDrop(held.clone());
    let (entered, started) = mpsc::channel();
    for target in [&mut left, &mut right] {
        let held = held.clone();
        let entered = entered.clone();
        target.set_test_observer(Arc::new(move |phase| {
            if phase == SaveTestPhase::Created {
                entered.send(()).unwrap();
                held.wait();
            }
        }));
    }
    let parent_before = fs::metadata(&destination.0).unwrap();
    let mut left_job = Box::pin(fixture.staging.save_assembled_file(
        &fixture.manifest,
        &assembled,
        &left,
    ));
    let mut right_job = Box::pin(fixture.staging.save_assembled_file(
        &fixture.manifest,
        &assembled,
        &right,
    ));
    let waker = waker();
    let mut context = Context::from_waker(&waker);
    {
        let _entered = runtime.enter();
        assert!(left_job.as_mut().poll(&mut context).is_pending());
        assert!(right_job.as_mut().poll(&mut context).is_pending());
    }
    started.recv_timeout(Duration::from_secs(10)).unwrap();
    started.recv_timeout(Duration::from_secs(10)).unwrap();
    let left_inode = fs::metadata(fixture.path("user-left.bin")).unwrap();
    let right_inode = fs::metadata(fixture.path("user-right.bin")).unwrap();
    assert_eq!((left_inode.len(), right_inode.len()), (0, 0));
    assert_ne!(left_inode.ino(), right_inode.ino());
    assert_eq!(fs::read_dir(&destination.0).unwrap().count(), 2);
    let current = fs::metadata(&destination.0).unwrap();
    assert_eq!(
        (
            current.dev(),
            current.ino(),
            current.uid(),
            current.gid(),
            current.mode()
        ),
        (
            parent_before.dev(),
            parent_before.ino(),
            parent_before.uid(),
            parent_before.gid(),
            parent_before.mode()
        )
    );
    assert_eq!(fixture.staging.inner.workers.available_permits(), 0);
    release.0.release();
    let (left_saved, right_saved) = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(10), async {
            tokio::join!(&mut left_job, &mut right_job)
        })
        .await
        .unwrap()
    });
    assert_eq!(left_saved.unwrap().file(), assembled.file());
    assert_eq!(right_saved.unwrap().file(), assembled.file());
    drop((left_job, right_job));
    fixture.saved("user-left.bin", bytes, Some(left_inode.ino()));
    fixture.saved("user-right.bin", bytes, Some(right_inode.ino()));
    assert_eq!(fixture.staging.inner.workers.available_permits(), 2);
}
