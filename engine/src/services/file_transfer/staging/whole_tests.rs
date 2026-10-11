use super::super::{work_whole, ChunkStaging};
use super::*;
use crate::{models::collaboration::WorkspaceRef, services::file_transfer::inspect_stream};
use std::{
    fs::{self, Permissions},
    future::Future,
    io::{Cursor, Write},
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Condvar, Mutex,
    },
    task::{Context, Wake, Waker},
    time::Duration,
};

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("cyanrex-whole-private-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone)]
struct Barrier(Arc<(Mutex<bool>, Condvar)>);
impl Barrier {
    fn new() -> Self {
        Self(Arc::new((Mutex::new(false), Condvar::new())))
    }
    fn wait(&self) {
        let (lock, condition) = &*self.0;
        let mut released = lock.lock().unwrap();
        while !*released {
            released = condition.wait(released).unwrap();
        }
    }
    fn release(&self) {
        let (lock, condition) = &*self.0;
        *lock.lock().unwrap() = true;
        condition.notify_all();
    }
}
struct ReleaseOnDrop(Barrier);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}
struct Noop;
impl Wake for Noop {
    fn wake(self: Arc<Self>) {}
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap()
}

fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: uuid::Uuid::from_u128(1).try_into().unwrap(),
        workspace_id: uuid::Uuid::from_u128(2).try_into().unwrap(),
    }
}

fn manifest(bytes: &[u8]) -> FileTransferManifest {
    inspect_stream(
        scope(),
        uuid::Uuid::from_u128(3).try_into().unwrap(),
        uuid::Uuid::from_u128(4).try_into().unwrap(),
        "whole.bin",
        &mut Cursor::new(bytes),
    )
    .unwrap()
}

fn fixture(
    runtime: &tokio::runtime::Runtime,
) -> (Root, ChunkStaging, FileTransferManifest, [u8; 32]) {
    let root = Root::new();
    let staging = ChunkStaging::open(scope(), &root.0, 1).unwrap();
    let manifest = manifest(b"whole inert bytes");
    let key = staging.bind_upload([3; 32]);
    let proof = runtime
        .block_on(staging.stage_bound_chunk(key, &manifest, 0, Cursor::new(b"whole inert bytes")))
        .unwrap();
    drop(proof);
    (root, staging, manifest, key)
}

fn restored(runtime: &tokio::runtime::Runtime, staging: &ChunkStaging) {
    let permit = runtime.block_on(async {
        tokio::time::timeout(
            Duration::from_secs(10),
            staging.inner.workers.clone().acquire_owned(),
        )
        .await
        .unwrap()
        .unwrap()
    });
    drop(permit);
}

#[test]
fn dropped_queued_whole_waiter_keeps_actual_job_slot() {
    let runtime = runtime();
    let (_root, staging, manifest, key) = fixture(&runtime);
    let external = Barrier::new();
    let external_release = ReleaseOnDrop(external.clone());
    let held = Barrier::new();
    let held_release = ReleaseOnDrop(held.clone());
    let (external_tx, external_rx) = mpsc::channel();
    let (observed_tx, observed_rx) = mpsc::channel();
    let external_job = runtime.spawn_blocking(move || {
        external_tx.send(()).unwrap();
        external.wait();
    });
    external_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let mut observed_tx = Some(observed_tx);
    let mut waiter = Box::pin(work_whole::observe_with(
        &staging.inner,
        key,
        &manifest,
        move |_| {
            if let Some(tx) = observed_tx.take() {
                tx.send(()).unwrap();
                held.wait();
            }
        },
    ));
    let waker = Waker::from(Arc::new(Noop));
    let mut context = Context::from_waker(&waker);
    {
        let _entered = runtime.enter();
        assert!(waiter.as_mut().poll(&mut context).is_pending());
    }
    assert_eq!(staging.inner.workers.available_permits(), 0);
    assert!(matches!(
        observed_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    drop(waiter);
    assert!(matches!(
        runtime.block_on(staging.observe_bound_file(key, &manifest)),
        Err(ChunkStagingError::CapacityExhausted)
    ));
    external_release.0.release();
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(10), external_job)
            .await
            .unwrap()
            .unwrap();
    });
    observed_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(staging.inner.workers.available_permits(), 0);
    held_release.0.release();
    restored(&runtime, &staging);
    runtime
        .block_on(staging.observe_bound_file(key, &manifest))
        .unwrap();
}

#[test]
fn dropped_running_whole_waiter_keeps_actual_job_slot() {
    let runtime = runtime();
    let (_root, staging, manifest, key) = fixture(&runtime);
    let held = Barrier::new();
    let release = ReleaseOnDrop(held.clone());
    let (observed_tx, observed_rx) = mpsc::channel();
    let mut observed_tx = Some(observed_tx);
    let mut waiter = Box::pin(work_whole::observe_with(
        &staging.inner,
        key,
        &manifest,
        move |_| {
            if let Some(tx) = observed_tx.take() {
                tx.send(()).unwrap();
                held.wait();
            }
        },
    ));
    let waker = Waker::from(Arc::new(Noop));
    let mut context = Context::from_waker(&waker);
    {
        let _entered = runtime.enter();
        assert!(waiter.as_mut().poll(&mut context).is_pending());
    }
    observed_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    drop(waiter);
    assert_eq!(staging.inner.workers.available_permits(), 0);
    assert!(matches!(
        runtime.block_on(staging.verify_staged_chunk(&manifest, 0)),
        Err(ChunkStagingError::CapacityExhausted)
    ));
    release.0.release();
    restored(&runtime, &staging);
    runtime
        .block_on(staging.observe_bound_file(key, &manifest))
        .unwrap();
}

#[test]
fn earlier_chunk_changed_during_later_scan_is_rejected() {
    let runtime = runtime();
    let root = Root::new();
    let staging = ChunkStaging::open(scope(), &root.0, 1).unwrap();
    let bytes = vec![7; crate::models::collaboration::FILE_TRANSFER_CHUNK_BYTES as usize + 1];
    let manifest = manifest(&bytes);
    let key = staging.bind_upload([4; 32]);
    for index in 0..manifest.chunk_sha256().len() {
        let spec = manifest.chunk(index as u32).unwrap();
        let bytes = bytes[spec.offset as usize..(spec.offset + spec.byte_length) as usize].to_vec();
        drop(
            runtime
                .block_on(staging.stage_bound_chunk(
                    key,
                    &manifest,
                    index as u32,
                    Cursor::new(bytes),
                ))
                .unwrap(),
        );
    }
    // This exact staged layout must pass before the deterministic mid-scan mutation.
    drop(
        runtime
            .block_on(staging.observe_bound_file(key, &manifest))
            .unwrap(),
    );
    let (name, _) = bound_chunk_name(key, &manifest, 0).unwrap();
    let first = root.0.join(name);
    let first_for_callback = first.clone();
    let changed = Arc::new(AtomicBool::new(false));
    let changed_in_callback = Arc::clone(&changed);
    let result = runtime.block_on(work_whole::observe_with(
        &staging.inner,
        key,
        &manifest,
        move |index| {
            if index == 1 {
                // A real same-length write after the first chunk's hash/snapshot and the later
                // chunk's read, not an elapsed-time race or a simulated metadata value.
                fs::set_permissions(&first_for_callback, Permissions::from_mode(0o600)).unwrap();
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .open(&first_for_callback)
                    .unwrap();
                file.write_all(&[8]).unwrap();
                drop(file);
                fs::set_permissions(&first_for_callback, Permissions::from_mode(0o400)).unwrap();
                changed_in_callback.store(true, Ordering::SeqCst);
            }
        },
    ));
    assert!(matches!(result, Err(ChunkStagingError::Unavailable)));
    assert!(changed.load(Ordering::SeqCst));
    restored(&runtime, &staging);
    // The real changed original remains with the same length; it was not fixed or removed.
    assert_eq!(
        fs::metadata(&first).unwrap().len(),
        crate::models::collaboration::FILE_TRANSFER_CHUNK_BYTES
    );
    let mut first_byte = [0];
    File::open(first)
        .unwrap()
        .read_exact(&mut first_byte)
        .unwrap();
    assert_eq!(first_byte, [8]);
    let (last_name, _) = bound_chunk_name(key, &manifest, 1).unwrap();
    assert_eq!(fs::read(root.0.join(last_name)).unwrap(), vec![7]);
}
