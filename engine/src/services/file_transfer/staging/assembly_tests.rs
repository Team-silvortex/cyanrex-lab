use super::{assemble::Phase, assembly_test_support::*, binding, work_assemble, ChunkStagingError};
use std::{
    fs::{self, Permissions},
    future::Future,
    io::Write,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    task::Context,
    time::Duration,
};

const BYTES: &[u8] = b"whole inert bytes";

#[test]
fn cancelled_queued_assembly_keeps_actual_job_slot_and_original_output() {
    // Root outlives the runtime, and unwind guards release held jobs before runtime shutdown.
    let root = Root::new();
    let control_root = Root::new();
    let runtime = runtime();
    let (staging, manifest) = staged(&runtime, &root, BYTES);
    let original_root = root.identity();
    let original_sources = sources(&root, &manifest);
    let external = Barrier::new();
    let external_release = ReleaseOnDrop(external.clone());
    let held = Barrier::new();
    let held_release = ReleaseOnDrop(held.clone());
    let (external_tx, external_rx) = mpsc::channel();
    let (created_tx, created_rx) = mpsc::channel();
    let external_job = runtime.spawn_blocking(move || {
        external_tx.send(()).unwrap();
        external.wait();
    });
    external_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let mut created_tx = Some(created_tx);
    let mut waiter = Box::pin(work_assemble::run_with(
        &staging.inner,
        &manifest,
        move |phase| {
            if phase == Phase::Created {
                if let Some(tx) = created_tx.take() {
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
        // The only await is the dispatched JoinHandle: Pending proves actual queued submission.
        assert!(waiter.as_mut().poll(&mut context).is_pending());
    }
    assert_eq!(staging.inner.workers.available_permits(), 0);
    assert!(matches!(
        created_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert!(!output(&root, &manifest).exists());
    drop(waiter);
    assert!(matches!(
        runtime.block_on(staging.assemble_staged_file(&manifest)),
        Err(ChunkStagingError::CapacityExhausted)
    ));
    assert!(matches!(
        runtime.block_on(staging.verify_staged_chunk(&manifest, 0)),
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
    let created = fs::metadata(output(&root, &manifest)).unwrap();
    assert_eq!(created.len(), 0);
    assert_eq!(created.mode() & 0o7777, 0o600);
    assert_eq!(created.nlink(), 1);
    assert_eq!(staging.inner.workers.available_permits(), 0);
    held_release.0.release();
    restored(&runtime, &staging);
    verified_output(&root, &manifest, BYTES, created.ino());
    assert_eq!(sources(&root, &manifest), original_sources);
    assert_eq!(root.identity(), original_root);
    let complete = Stamp::of(&fs::metadata(output(&root, &manifest)).unwrap());
    assert!(matches!(
        runtime.block_on(staging.assemble_staged_file(&manifest)),
        Err(ChunkStagingError::Conflict)
    ));
    assert_eq!(
        Stamp::of(&fs::metadata(output(&root, &manifest)).unwrap()),
        complete
    );
    healthy(&runtime, &control_root, BYTES);
}

#[test]
fn cancelled_running_assembly_keeps_actual_job_slot_until_same_inode_finishes() {
    let root = Root::new();
    let control_root = Root::new();
    let runtime = runtime();
    let (staging, manifest) = staged(&runtime, &root, BYTES);
    let original_root = root.identity();
    let original_sources = sources(&root, &manifest);
    let held = Barrier::new();
    let release = ReleaseOnDrop(held.clone());
    let (created_tx, created_rx) = mpsc::channel();
    let mut created_tx = Some(created_tx);
    let mut waiter = Box::pin(work_assemble::run_with(
        &staging.inner,
        &manifest,
        move |phase| {
            if phase == Phase::Created {
                if let Some(tx) = created_tx.take() {
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
        assert!(waiter.as_mut().poll(&mut context).is_pending());
    }
    // The in-job callback follows a real successful O_EXCL create, not an elapsed-time guess.
    created_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let created = fs::metadata(output(&root, &manifest)).unwrap();
    assert_eq!(created.len(), 0);
    assert_eq!(created.mode() & 0o7777, 0o600);
    assert_eq!(created.nlink(), 1);
    drop(waiter);
    assert_eq!(staging.inner.workers.available_permits(), 0);
    assert!(matches!(
        runtime.block_on(staging.assemble_staged_file(&manifest)),
        Err(ChunkStagingError::CapacityExhausted)
    ));
    assert!(matches!(
        runtime.block_on(staging.verify_staged_chunk(&manifest, 0)),
        Err(ChunkStagingError::CapacityExhausted)
    ));
    release.0.release();
    restored(&runtime, &staging);
    verified_output(&root, &manifest, BYTES, created.ino());
    assert_eq!(sources(&root, &manifest), original_sources);
    assert_eq!(root.identity(), original_root);
    assert!(matches!(
        runtime.block_on(staging.assemble_staged_file(&manifest)),
        Err(ChunkStagingError::Conflict)
    ));
    healthy(&runtime, &control_root, BYTES);
}

#[test]
fn earlier_input_replaced_after_later_copy_is_unconfirmed_and_retains_output() {
    let root = Root::new();
    let control_root = Root::new();
    let runtime = runtime();
    let mut bytes = vec![7; crate::models::collaboration::FILE_TRANSFER_CHUNK_BYTES as usize];
    bytes.extend_from_slice(BYTES);
    healthy(&runtime, &control_root, &bytes);
    let (staging, manifest) = staged(&runtime, &root, &bytes);
    let original_root = root.identity();
    let first = root.0.join(binding::chunk_name(&manifest, 0).unwrap());
    let kept = root.0.join("original-earlier-chunk");
    let first_bytes = fs::read(&first).unwrap();
    let original_inode = fs::metadata(&first).unwrap().ino();
    let changed = Arc::new(AtomicBool::new(false));
    let changed_job = changed.clone();
    let first_job = first.clone();
    let kept_job = kept.clone();
    let result = runtime.block_on(work_assemble::run_with(
        &staging.inner,
        &manifest,
        move |phase| {
            if phase == Phase::Copied(1) && !changed_job.swap(true, Ordering::SeqCst) {
                // The first chunk was already copied; retain it and install matching bytes at a new inode.
                fs::rename(&first_job, &kept_job).unwrap();
                fs::write(&first_job, &first_bytes).unwrap();
                fs::set_permissions(&first_job, Permissions::from_mode(0o400)).unwrap();
            }
        },
    ));
    assert!(changed.load(Ordering::SeqCst));
    assert!(matches!(result, Err(ChunkStagingError::Unconfirmed)));
    assert_eq!(fs::metadata(&kept).unwrap().ino(), original_inode);
    assert_ne!(fs::metadata(&first).unwrap().ino(), original_inode);
    assert_eq!(fs::read(&first).unwrap(), fs::read(&kept).unwrap());
    let path = output(&root, &manifest);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o7777, 0o600);
    assert_eq!(fs::metadata(&path).unwrap().nlink(), 1);
    assert_eq!(root.identity(), original_root);
    assert!(matches!(
        runtime.block_on(staging.assemble_staged_file(&manifest)),
        Err(ChunkStagingError::Conflict)
    ));
}

#[test]
fn same_bytes_output_substitution_after_original_readback_is_unconfirmed_not_adopted() {
    let root = Root::new();
    let control_root = Root::new();
    let runtime = runtime();
    healthy(&runtime, &control_root, BYTES);
    let (staging, manifest) = staged(&runtime, &root, BYTES);
    let original_root = root.identity();
    let original_sources = sources(&root, &manifest);
    let path = output(&root, &manifest);
    let kept = root.0.join("original-output");
    let path_job = path.clone();
    let kept_job = kept.clone();
    let original_inode = Arc::new(Mutex::new(None));
    let original_inode_job = original_inode.clone();
    let result = runtime.block_on(work_assemble::run_with(
        &staging.inner,
        &manifest,
        move |phase| {
            if phase == Phase::Readback {
                // The original RO descriptor is still retained and has passed actual whole-byte verification.
                *original_inode_job.lock().unwrap() = Some(fs::metadata(&path_job).unwrap().ino());
                fs::rename(&path_job, &kept_job).unwrap();
                fs::write(&path_job, BYTES).unwrap();
                fs::set_permissions(&path_job, Permissions::from_mode(0o400)).unwrap();
            }
        },
    ));
    assert!(matches!(result, Err(ChunkStagingError::Unconfirmed)));
    let inode = original_inode.lock().unwrap().unwrap();
    assert_eq!(fs::metadata(&kept).unwrap().ino(), inode);
    assert_ne!(fs::metadata(&path).unwrap().ino(), inode);
    assert_eq!(fs::read(&path).unwrap(), BYTES);
    assert_eq!(fs::read(&kept).unwrap(), BYTES);
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o7777, 0o400);
    assert_eq!(fs::metadata(&kept).unwrap().mode() & 0o7777, 0o400);
    assert_eq!(sources(&root, &manifest), original_sources);
    assert_eq!(root.identity(), original_root);
    let replacement = Stamp::of(&fs::metadata(&path).unwrap());
    assert!(matches!(
        runtime.block_on(staging.assemble_staged_file(&manifest)),
        Err(ChunkStagingError::Conflict)
    ));
    assert_eq!(Stamp::of(&fs::metadata(&path).unwrap()), replacement);
    assert_eq!(fs::metadata(&kept).unwrap().ino(), inode);
}

#[test]
fn same_inode_equal_length_output_tamper_requires_actual_readback_digest() {
    let root = Root::new();
    let control_root = Root::new();
    let runtime = runtime();
    let (staging, manifest) = staged(&runtime, &root, BYTES);
    let original_sources = sources(&root, &manifest);
    let original_root = root.identity();
    let path = output(&root, &manifest);
    let path_job = path.clone();
    let original_inode = Arc::new(Mutex::new(None));
    let inode_job = original_inode.clone();
    let changed = Arc::new(AtomicBool::new(false));
    let changed_job = changed.clone();
    let result = runtime.block_on(work_assemble::run_with(
        &staging.inner,
        &manifest,
        move |phase| match phase {
            Phase::Created => {
                *inode_job.lock().unwrap() = Some(fs::metadata(&path_job).unwrap().ino())
            }
            Phase::Copied(0) if !changed_job.swap(true, Ordering::SeqCst) => {
                // This is the copied chunk's final input EOF. Change equal-length output
                // before writtenSnapshot is captured: later metadata can match, so only
                // actual output digest readback rejects the newly corrupt bytes.
                let before = fs::metadata(&path_job).unwrap();
                assert_eq!(before.len(), BYTES.len() as u64);
                fs::OpenOptions::new()
                    .write(true)
                    .open(&path_job)
                    .unwrap()
                    .write_all(&[BYTES[0] ^ 1])
                    .unwrap();
                let after = fs::metadata(&path_job).unwrap();
                assert_eq!(
                    (before.dev(), before.ino(), before.len()),
                    (after.dev(), after.ino(), after.len())
                );
            }
            _ => (),
        },
    ));
    assert!(changed.load(Ordering::SeqCst));
    assert!(matches!(result, Err(ChunkStagingError::Unconfirmed)));
    let metadata = fs::metadata(&path).unwrap();
    assert_eq!(metadata.ino(), original_inode.lock().unwrap().unwrap());
    assert_eq!(metadata.len(), BYTES.len() as u64);
    assert_eq!(metadata.mode() & 0o7777, 0o400);
    assert_eq!(metadata.nlink(), 1);
    let bytes = fs::read(&path).unwrap();
    assert_eq!(bytes[0], BYTES[0] ^ 1);
    assert_eq!(&bytes[1..], &BYTES[1..]);
    assert!(matches!(
        crate::services::file_transfer::verify_file(&manifest, &mut fs::File::open(&path).unwrap()),
        Err(crate::services::file_transfer::FileTransferError::DigestMismatch)
    ));
    assert_eq!(sources(&root, &manifest), original_sources);
    assert_eq!(root.identity(), original_root);
    let corrupt = Stamp::of(&metadata);
    assert!(matches!(
        runtime.block_on(staging.assemble_staged_file(&manifest)),
        Err(ChunkStagingError::Conflict)
    ));
    assert_eq!(Stamp::of(&fs::metadata(&path).unwrap()), corrupt);
    healthy(&runtime, &control_root, BYTES);
}

#[test]
fn original_root_replaced_after_output_readback_is_unconfirmed_and_never_adopted() {
    let root = Root::new();
    let control_root = Root::new();
    // This retained-root guard also runs on assertion unwind, after runtime shutdown.
    let retained = Root(root.0.with_file_name(format!(
        "cyanrex-assembly-retained-{}",
        uuid::Uuid::new_v4()
    )));
    let runtime = runtime();
    let (staging, manifest) = staged(&runtime, &root, BYTES);
    let original_root = root.identity();
    let original_sources = sources(&root, &manifest);
    let path_job = root.0.clone();
    let retained_job = retained.0.clone();
    let original_inode = Arc::new(Mutex::new(None));
    let inode_job = original_inode.clone();
    let manifest_job = manifest.clone();
    let result = runtime.block_on(work_assemble::run_with(
        &staging.inner,
        &manifest,
        move |phase| {
            if phase == Phase::Readback {
                let name =
                    binding::assembled_name(binding::manifest_digest(&manifest_job).unwrap());
                *inode_job.lock().unwrap() = Some(fs::metadata(path_job.join(name)).unwrap().ino());
                fs::rename(&path_job, &retained_job).unwrap();
                fs::DirBuilder::new().mode(0o700).create(&path_job).unwrap();
            }
        },
    ));
    assert!(matches!(result, Err(ChunkStagingError::Unconfirmed)));
    assert_eq!(retained.identity(), original_root);
    assert_ne!(root.identity().1, original_root.1);
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
    assert_eq!(sources(&retained, &manifest), original_sources);
    verified_output(
        &retained,
        &manifest,
        BYTES,
        original_inode.lock().unwrap().unwrap(),
    );
    assert!(matches!(
        runtime.block_on(staging.assemble_staged_file(&manifest)),
        Err(ChunkStagingError::Unavailable)
    ));
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
    fs::remove_dir(&root.0).unwrap();
    fs::rename(&retained.0, &root.0).unwrap();
    let complete = Stamp::of(&fs::metadata(output(&root, &manifest)).unwrap());
    assert!(matches!(
        runtime.block_on(staging.assemble_staged_file(&manifest)),
        Err(ChunkStagingError::Conflict)
    ));
    assert_eq!(
        Stamp::of(&fs::metadata(output(&root, &manifest)).unwrap()),
        complete
    );
    assert_eq!(sources(&root, &manifest), original_sources);
    healthy(&runtime, &control_root, BYTES);
}
