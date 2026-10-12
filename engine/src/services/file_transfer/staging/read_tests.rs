use super::super::{work_read, ChunkStaging};
use super::*;
use crate::{models::collaboration::WorkspaceRef, services::file_transfer::inspect_stream};
use std::{
    fs::{self, Permissions},
    future::Future,
    io::{Cursor, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Condvar, Mutex,
    },
    task::{Context, Wake, Waker},
    time::Duration,
};
use tokio::sync::Semaphore;

const CONTENT: &[u8] = b"verified private bytes";

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("cyanrex-buffered-read-{}", uuid::Uuid::new_v4()));
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
fn fixture(
    runtime: &tokio::runtime::Runtime,
) -> (Root, ChunkStaging, FileTransferManifest, [u8; 32]) {
    let root = Root::new();
    let path = root.0.join("chunks");
    fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
    let staging = ChunkStaging::open(scope(), &path, 1).unwrap();
    let manifest = inspect_stream(
        scope(),
        uuid::Uuid::from_u128(3).try_into().unwrap(),
        uuid::Uuid::from_u128(4).try_into().unwrap(),
        "private.bin",
        &mut Cursor::new(CONTENT),
    )
    .unwrap();
    let key = staging.bind_upload([3; 32]);
    drop(
        runtime
            .block_on(staging.stage_bound_chunk(key, &manifest, 0, Cursor::new(CONTENT)))
            .unwrap(),
    );
    (root, staging, manifest, key)
}
fn lease(responses: &Arc<Semaphore>) -> OwnedSemaphorePermit {
    responses.clone().try_acquire_owned().unwrap()
}
fn restored(runtime: &tokio::runtime::Runtime, gate: &Arc<Semaphore>) -> OwnedSemaphorePermit {
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(10), gate.clone().acquire_owned())
            .await
            .unwrap()
            .unwrap()
    })
}

#[test]
fn dropped_queued_buffered_waiter_keeps_worker_and_response_until_orphan_job_finishes() {
    let runtime = runtime();
    let (_root, staging, manifest, key) = fixture(&runtime);
    let responses = Arc::new(Semaphore::new(1));
    let external = Barrier::new();
    let external_release = ReleaseOnDrop(external.clone());
    let held = Barrier::new();
    let release = ReleaseOnDrop(held.clone());
    let (external_tx, external_rx) = mpsc::channel();
    let (began_tx, began_rx) = mpsc::channel();
    let external_job = runtime.spawn_blocking(move || {
        external_tx.send(()).unwrap();
        external.wait();
    });
    external_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let mut began_tx = Some(began_tx);
    let mut waiter = Box::pin(work_read::run_with(
        &staging.inner,
        key,
        &manifest,
        0,
        lease(&responses),
        move |point| {
            if matches!(point, ReadPoint::Opened) {
                began_tx.take().unwrap().send(()).unwrap();
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
    // Manual polling reaches the actual admission/submission; the sole blocking thread is held.
    assert_eq!(staging.inner.workers.available_permits(), 0);
    assert_eq!(responses.available_permits(), 0);
    assert!(matches!(
        began_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    drop(waiter);
    assert_eq!(responses.available_permits(), 0);
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
    // Now the abandoned queued job is demonstrably running and still owns both permits.
    began_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(staging.inner.workers.available_permits(), 0);
    assert_eq!(responses.available_permits(), 0);
    release.0.release();
    drop(restored(&runtime, &staging.inner.workers));
    drop(restored(&runtime, &responses));
    let result = runtime
        .block_on(staging.read_bound_chunk(key, &manifest, 0, lease(&responses)))
        .unwrap();
    assert_eq!(result.bytes(), CONTENT);
    drop(result);
    assert_eq!(responses.available_permits(), 1);
}

#[test]
fn dropped_running_buffered_waiter_keeps_both_permits_until_actual_read_finishes() {
    let runtime = runtime();
    let (_root, staging, manifest, key) = fixture(&runtime);
    let responses = Arc::new(Semaphore::new(1));
    let held = Barrier::new();
    let release = ReleaseOnDrop(held.clone());
    let (began_tx, began_rx) = mpsc::channel();
    let mut began_tx = Some(began_tx);
    let mut waiter = Box::pin(work_read::run_with(
        &staging.inner,
        key,
        &manifest,
        0,
        lease(&responses),
        move |point| {
            if matches!(point, ReadPoint::Opened) {
                began_tx.take().unwrap().send(()).unwrap();
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
    began_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    drop(waiter);
    assert_eq!(staging.inner.workers.available_permits(), 0);
    assert_eq!(responses.available_permits(), 0);
    release.0.release();
    drop(restored(&runtime, &staging.inner.workers));
    drop(restored(&runtime, &responses));
    let result = runtime
        .block_on(staging.read_bound_chunk(key, &manifest, 0, lease(&responses)))
        .unwrap();
    assert_eq!(result.bytes(), CONTENT);
}

#[test]
fn completed_unclaimed_and_returned_buffered_results_hold_response_capacity_until_drop() {
    let runtime = runtime();
    let (_root, staging, manifest, key) = fixture(&runtime);
    let responses = Arc::new(Semaphore::new(1));
    let held = Barrier::new();
    let release = ReleaseOnDrop(held.clone());
    let (copied_tx, copied_rx) = mpsc::channel();
    let mut copied_tx = Some(copied_tx);
    let mut waiter = Box::pin(work_read::run_with(
        &staging.inner,
        key,
        &manifest,
        0,
        lease(&responses),
        move |point| {
            if matches!(point, ReadPoint::Copied) {
                copied_tx.take().unwrap().send(()).unwrap();
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
    copied_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    release.0.release();
    // Actual worker-slot restoration proves closure completion. The waiter is NOT repolled;
    // its unclaimed completed result still owns the separate response slot.
    let completed = restored(&runtime, &staging.inner.workers);
    assert_eq!(responses.available_permits(), 0);
    drop(waiter);
    drop(restored(&runtime, &responses));
    drop(completed);
    let result = runtime
        .block_on(staging.read_bound_chunk(key, &manifest, 0, lease(&responses)))
        .unwrap();
    assert_eq!(result.bytes(), CONTENT);
    assert_eq!(staging.inner.workers.available_permits(), 1);
    assert_eq!(responses.available_permits(), 0);
    assert!(responses.clone().try_acquire_owned().is_err());
    drop(result);
    assert_eq!(responses.available_permits(), 1);
}

#[test]
fn private_copy_rejects_changed_digest_and_identical_entry_or_root_replacement() {
    for mode in ["digest", "entry", "root"] {
        let runtime = runtime();
        let (root, staging, manifest, key) = fixture(&runtime);
        let responses = Arc::new(Semaphore::new(1));
        let healthy = runtime
            .block_on(staging.read_bound_chunk(key, &manifest, 0, lease(&responses)))
            .unwrap();
        assert_eq!(healthy.bytes(), CONTENT);
        drop(healthy);
        let (name, _) = bound_chunk_name(key, &manifest, 0).unwrap();
        let directory = root.0.join("chunks");
        let path = directory.join(&name);
        let original_inode = fs::metadata(&path).unwrap().ino();
        let moved_entry = root.0.join("original-chunk");
        let moved_root = root.0.join("original-root");
        let changed = Arc::new(AtomicBool::new(false));
        let changed_in_job = Arc::clone(&changed);
        let copied_path = path.clone();
        let copied_directory = directory.clone();
        let copied_entry = moved_entry.clone();
        let copied_root = moved_root.clone();
        let result = runtime.block_on(work_read::run_with(
            &staging.inner,
            key,
            &manifest,
            0,
            lease(&responses),
            move |point| {
                match (mode, point) {
                    ("digest", ReadPoint::Opened) => {
                        fs::set_permissions(&copied_path, Permissions::from_mode(0o600)).unwrap();
                        let mut writer = fs::OpenOptions::new()
                            .write(true)
                            .open(&copied_path)
                            .unwrap();
                        writer.write_all(&[CONTENT[0] ^ 1]).unwrap();
                        drop(writer);
                        fs::set_permissions(&copied_path, Permissions::from_mode(0o400)).unwrap();
                    }
                    ("entry", ReadPoint::Copied) => {
                        fs::rename(&copied_path, &copied_entry).unwrap();
                        let mut writer = fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&copied_path)
                            .unwrap();
                        writer.write_all(CONTENT).unwrap();
                        drop(writer);
                        fs::set_permissions(&copied_path, Permissions::from_mode(0o400)).unwrap();
                    }
                    ("root", ReadPoint::Copied) => {
                        fs::rename(&copied_directory, &copied_root).unwrap();
                        fs::DirBuilder::new()
                            .mode(0o700)
                            .create(&copied_directory)
                            .unwrap();
                    }
                    _ => return,
                }
                changed_in_job.store(true, Ordering::SeqCst);
            },
        ));
        assert!(changed.load(Ordering::SeqCst));
        assert!(matches!(result, Err(ChunkStagingError::Unavailable)));
        assert_eq!(staging.inner.workers.available_permits(), 1);
        assert_eq!(responses.available_permits(), 1);
        match mode {
            "digest" => {
                let mut expected = CONTENT.to_vec();
                expected[0] ^= 1;
                assert_eq!(fs::read(path).unwrap(), expected);
            }
            "entry" => {
                assert_ne!(fs::metadata(&path).unwrap().ino(), original_inode);
                assert_eq!(fs::read(path).unwrap(), CONTENT);
                assert_eq!(fs::read(moved_entry).unwrap(), CONTENT);
            }
            _ => {
                assert_eq!(fs::read_dir(directory).unwrap().count(), 0);
                assert_eq!(fs::read(moved_root.join(name)).unwrap(), CONTENT);
            }
        }
    }
}
