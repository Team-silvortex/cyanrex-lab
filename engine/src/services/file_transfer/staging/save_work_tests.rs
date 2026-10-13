use super::super::{save_io::Phase, work_save, ChunkStagingError};
use super::{test_support::*, *};
use crate::models::collaboration::{FileVersionRef, FILE_TRANSFER_CHUNK_BYTES};
use std::{
    fs,
    future::Future,
    os::unix::fs::MetadataExt,
    sync::{mpsc, Arc},
    task::Context,
    time::Duration,
};

#[test]
fn cancelled_public_queued_save_keeps_original_proof_and_worker_until_verified_completion() {
    let source = Root::new();
    let destination = Root::new();
    let runtime = runtime();
    let bytes = b"seventeen bytes!!";
    let (fixture, assembled) = Fixture::ready(&runtime, &source, &destination, bytes, 1);
    let target = fixture.target("public-queued.bin");
    let weak = Arc::downgrade(assembled.proof());
    let external = Barrier::new();
    let release = ReleaseOnDrop(external.clone());
    let (entered, started) = mpsc::channel();
    let external_job = runtime.spawn_blocking(move || {
        entered.send(()).unwrap();
        external.wait();
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
    assert_eq!(
        weak.strong_count(),
        2,
        "public dispatch cloned the original proof into its actual queued job"
    );
    assert!(!fixture.path("public-queued.bin").exists());
    drop(pending);
    drop(assembled);
    assert_eq!(weak.strong_count(), 1);
    assert!(matches!(
        runtime.block_on(fixture.staging.verify_staged_chunk(&fixture.manifest, 0)),
        Err(ChunkStagingError::CapacityExhausted)
    ));
    release.0.release();
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(10), external_job)
            .await
            .unwrap()
            .unwrap();
    });
    restored(&runtime, &fixture.staging);
    proof_gone(&runtime, &weak);
    fixture.saved("public-queued.bin", bytes, None);
    assert_eq!(fs::read_dir(&destination.0).unwrap().count(), 1);
}

#[test]
fn cancelled_queued_save_retains_actual_worker_and_original_proof_after_ack_drop() {
    let source = Root::new();
    let destination = Root::new();
    let runtime = runtime();
    let bytes = b"seventeen bytes!!";
    let (fixture, assembled) = Fixture::ready(&runtime, &source, &destination, bytes, 1);
    let target = fixture.target("queued.bin");
    let weak = Arc::downgrade(assembled.proof());
    let external = Barrier::new();
    let external_release = ReleaseOnDrop(external.clone());
    let running = Barrier::new();
    let running_release = ReleaseOnDrop(running.clone());
    let (external_tx, external_rx) = mpsc::channel();
    let external_job = runtime.spawn_blocking(move || {
        external_tx.send(()).unwrap();
        external.wait();
    });
    external_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let (created_tx, created_rx) = mpsc::channel();
    let mut created_tx = Some(created_tx);
    let mut pending = Box::pin(work_save::run_with(
        &fixture.staging.inner,
        &fixture.manifest,
        assembled.proof(),
        &target.inner,
        move |phase| {
            if phase == Phase::Created {
                if let Some(tx) = created_tx.take() {
                    tx.send(()).unwrap();
                    running.wait();
                }
            }
        },
    ));
    let waker = waker();
    let mut context = Context::from_waker(&waker);
    {
        let _entered = runtime.enter();
        assert!(pending.as_mut().poll(&mut context).is_pending());
    }
    assert_eq!(fixture.staging.inner.workers.available_permits(), 0);
    assert!(matches!(
        created_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert!(!fixture.path("queued.bin").exists());
    drop(pending);
    drop(assembled);
    assert_eq!(
        weak.strong_count(),
        1,
        "the actual queued job alone retains the two original descriptors"
    );
    assert!(matches!(
        runtime.block_on(fixture.staging.verify_staged_chunk(&fixture.manifest, 0)),
        Err(ChunkStagingError::CapacityExhausted)
    ));
    external_release.0.release();
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(10), external_job)
            .await
            .unwrap()
            .unwrap();
    });
    created_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let created = fs::metadata(fixture.path("queued.bin")).unwrap();
    assert_eq!(
        (created.len(), created.mode() & 0o7777, created.nlink()),
        (0, 0o600, 1)
    );
    assert_eq!(fixture.staging.inner.workers.available_permits(), 0);
    running_release.0.release();
    restored(&runtime, &fixture.staging);
    proof_gone(&runtime, &weak);
    fixture.saved("queued.bin", bytes, Some(created.ino()));
    assert_eq!(fs::read_dir(&fixture.destination.0).unwrap().count(), 1);
}

#[test]
fn cancelled_running_save_keeps_created_inode_and_proof_until_actual_readback_finishes() {
    let source = Root::new();
    let destination = Root::new();
    let runtime = runtime();
    let bytes = b"seventeen bytes!!";
    let (fixture, assembled) = Fixture::ready(&runtime, &source, &destination, bytes, 1);
    let target = fixture.target("running.bin");
    let weak = Arc::downgrade(assembled.proof());
    let held = Barrier::new();
    let release = ReleaseOnDrop(held.clone());
    let (entered, started) = mpsc::channel();
    let mut entered = Some(entered);
    let mut pending = Box::pin(work_save::run_with(
        &fixture.staging.inner,
        &fixture.manifest,
        assembled.proof(),
        &target.inner,
        move |phase| {
            if phase == Phase::Created {
                if let Some(tx) = entered.take() {
                    tx.send(()).unwrap();
                    held.wait();
                }
            }
        },
    ));
    let waker = waker();
    let mut context = Context::from_waker(&waker);
    {
        let _entered = runtime.enter();
        assert!(pending.as_mut().poll(&mut context).is_pending());
    }
    started.recv_timeout(Duration::from_secs(10)).unwrap();
    let created = fs::metadata(fixture.path("running.bin")).unwrap();
    assert_eq!(
        (created.len(), created.mode() & 0o7777, created.nlink()),
        (0, 0o600, 1)
    );
    drop(pending);
    assert!(matches!(
        runtime.block_on(fixture.staging.save_assembled_file(
            &fixture.manifest,
            &assembled,
            &target
        )),
        Err(FileSaveError::CapacityExhausted)
    ));
    drop(assembled);
    assert_eq!(weak.strong_count(), 1);
    assert!(matches!(
        runtime.block_on(fixture.staging.verify_staged_chunk(&fixture.manifest, 0)),
        Err(ChunkStagingError::CapacityExhausted)
    ));
    release.0.release();
    restored(&runtime, &fixture.staging);
    proof_gone(&runtime, &weak);
    fixture.saved("running.bin", bytes, Some(created.ino()));
    assert_eq!(fs::read_dir(&fixture.destination.0).unwrap().count(), 1);
}

#[test]
fn same_original_ack_borrows_overlap_in_two_actual_jobs_without_shared_offset_reuse() {
    let source = Root::new();
    let destination = Root::new();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let mut bytes = vec![7; FILE_TRANSFER_CHUNK_BYTES as usize];
    bytes.extend_from_slice(b"seventeen bytes!!");
    let (fixture, assembled) = Fixture::ready(&runtime, &source, &destination, &bytes, 2);
    let left = fixture.target("left.bin");
    let right = fixture.target("right.bin");
    let held = Barrier::new();
    let release = ReleaseOnDrop(held.clone());
    let (entered, started) = mpsc::channel();
    let left_held = held.clone();
    let left_entered = entered.clone();
    let mut left_job = Box::pin(work_save::run_with(
        &fixture.staging.inner,
        &fixture.manifest,
        assembled.proof(),
        &left.inner,
        move |phase| {
            if phase == Phase::Created {
                left_entered.send(()).unwrap();
                left_held.wait();
            }
        },
    ));
    let mut right_job = Box::pin(work_save::run_with(
        &fixture.staging.inner,
        &fixture.manifest,
        assembled.proof(),
        &right.inner,
        move |phase| {
            if phase == Phase::Created {
                entered.send(()).unwrap();
                held.wait();
            }
        },
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
    let left_created = fs::metadata(fixture.path("left.bin")).unwrap();
    let right_created = fs::metadata(fixture.path("right.bin")).unwrap();
    assert_eq!(left_created.len(), 0);
    assert_eq!(right_created.len(), 0);
    assert_ne!(left_created.ino(), right_created.ino());
    assert_eq!(fixture.staging.inner.workers.available_permits(), 0);
    release.0.release();
    let (left_saved, right_saved) = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(10), async {
            tokio::join!(&mut left_job, &mut right_job)
        })
        .await
        .unwrap()
    });
    assert_eq!(
        left_saved.unwrap().file(),
        &FileVersionRef::from_manifest(&fixture.manifest)
    );
    assert_eq!(
        right_saved.unwrap().file(),
        &FileVersionRef::from_manifest(&fixture.manifest)
    );
    drop((left_job, right_job));
    fixture.saved("left.bin", &bytes, Some(left_created.ino()));
    fixture.saved("right.bin", &bytes, Some(right_created.ino()));
    assert_eq!(fs::read_dir(&fixture.destination.0).unwrap().count(), 2);
    assert!(matches!(
        runtime.block_on(
            fixture
                .staging
                .save_assembled_file(&fixture.manifest, &assembled, &left)
        ),
        Err(FileSaveError::Conflict)
    ));
    assert_eq!(fixture.staging.inner.workers.available_permits(), 2);
}
