use super::*;
use crate::models::collaboration::FileTransferManifest;
use axum::body::Body;
use sha2::{Digest, Sha256};
use std::{
    fs,
    future::{poll_fn, Future},
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::PathBuf,
    sync::{mpsc, Condvar, Mutex},
    task::Poll,
};
use tokio::{runtime::Builder, time::timeout};
use uuid::Uuid;

#[path = "flow_work_tests.rs"]
mod flow_tests;

struct Root(PathBuf, (u64, u64));
impl Root {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("cyanrex-receiver-unit-{}", Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let meta = fs::metadata(&root).unwrap();
        Self(root, (meta.dev(), meta.ino()))
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove only the exclusive synthetic receiver root");
    }
}

struct Barrier {
    released: Mutex<bool>,
    wake: Condvar,
}
impl Barrier {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            released: Mutex::new(false),
            wake: Condvar::new(),
        })
    }
    fn wait(&self) {
        let mut released = self.released.lock().unwrap();
        while !*released {
            released = self.wake.wait(released).unwrap();
        }
    }
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.wake.notify_all();
    }
}
struct ReleaseOnDrop(Arc<Barrier>);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}

fn manifest(seed: u128) -> FileTransferManifest {
    let scope = WorkspaceRef {
        authority_id: Uuid::from_u128(1).try_into().unwrap(),
        workspace_id: Uuid::from_u128(2).try_into().unwrap(),
    };
    let digest: Sha256Digest = format!("{:x}", Sha256::digest(data())).parse().unwrap();
    FileTransferManifest::new(
        scope,
        Uuid::from_u128(10 + seed).try_into().unwrap(),
        Uuid::from_u128(20 + seed).try_into().unwrap(),
        "received.bin",
        17,
        vec![digest.clone()],
        digest,
    )
    .unwrap()
}
fn data() -> Vec<u8> {
    (0..17).collect()
}
fn response(manifest: &FileTransferManifest) -> Response<Body> {
    let spec = manifest.chunk(0).unwrap();
    let mut builder = Response::builder().status(200);
    for (name, value) in [
        ("content-type", "application/octet-stream".to_owned()),
        (
            "content-disposition",
            "attachment; filename=\"chunk.bin\"".to_owned(),
        ),
        ("content-length", spec.byte_length.to_string()),
        ("x-cyanrex-file-transfer-version", "1".to_owned()),
        ("x-cyanrex-file-id", manifest.file_id().to_string()),
        (
            "x-cyanrex-file-version-id",
            manifest.version_id().to_string(),
        ),
        ("x-cyanrex-chunk-index", "0".to_owned()),
        ("x-cyanrex-chunk-sha256", spec.sha256.to_string()),
        ("x-cyanrex-file-sha256", manifest.sha256().to_string()),
        ("cache-control", "no-store".to_owned()),
        ("pragma", "no-cache".to_owned()),
        ("x-content-type-options", "nosniff".to_owned()),
    ] {
        builder = builder.header(name, value);
    }
    builder.body(Body::from(data())).unwrap()
}
fn receiver(root: &Root, manifest: &FileTransferManifest) -> FileChunkReceiver {
    FileChunkReceiver::with_limits(
        ChunkStaging::open(manifest.workspace(), &root.0, 1).unwrap(),
        1,
        Duration::from_secs(5),
        Duration::from_secs(60),
    )
    .unwrap()
}
async fn full(receiver: &FileChunkReceiver, manifest: &FileTransferManifest) {
    assert_eq!(receiver.controls.available_permits(), 0);
    assert_eq!(
        receiver
            .receive_and_stage(manifest, 0, response(manifest))
            .await
            .err(),
        Some(FileChunkReceiveError::CapacityExhausted)
    );
    assert_eq!(
        receiver
            .staging
            .verify_staged_chunk(manifest, 0)
            .await
            .err(),
        Some(ChunkStagingError::CapacityExhausted)
    );
}
async fn restored(
    receiver: &FileChunkReceiver,
    root: &Root,
    original: &FileTransferManifest,
    created_inode: Option<(u64, u64)>,
) {
    let slot = timeout(
        Duration::from_secs(10),
        receiver.controls.clone().acquire_owned(),
    )
    .await
    .expect("actual staging job must eventually drop its receive lease")
    .unwrap();
    drop(slot);
    timeout(Duration::from_secs(10), async {
        loop {
            match receiver.staging.verify_staged_chunk(original, 0).await {
                Ok(()) => break,
                Err(ChunkStagingError::CapacityExhausted) => tokio::task::yield_now().await,
                Err(error) => {
                    panic!("cancelled waiter cannot erase the completed original chunk: {error}")
                }
            }
        }
    })
    .await
    .expect("original worker must finish and its retained chunk must verify");
    let entries = fs::read_dir(&root.0)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(entries.len(), 1);
    let file = entries[0].path();
    assert_eq!(fs::read(&file).unwrap(), data());
    let before = fs::metadata(&file).unwrap();
    if let Some(created) = created_inode {
        assert_eq!(
            (before.dev(), before.ino()),
            created,
            "completion preserves the originally created inode"
        );
    }
    assert_eq!(before.mode() & 0o7777, 0o400);
    assert_eq!(before.nlink(), 1);
    let root_meta = fs::metadata(&root.0).unwrap();
    assert_eq!((root_meta.dev(), root_meta.ino()), root.1);
    assert_eq!(root_meta.mode() & 0o7777, 0o700);
    assert_eq!(before.uid(), root_meta.uid());
    assert_eq!(
        receiver
            .receive_and_stage(original, 0, response(original))
            .await
            .err(),
        Some(FileChunkReceiveError::Staging(ChunkStagingError::Conflict))
    );
    let after = fs::metadata(&file).unwrap();
    assert_eq!(
        (
            after.dev(),
            after.ino(),
            after.len(),
            after.mtime(),
            after.mtime_nsec(),
            after.ctime(),
            after.ctime_nsec()
        ),
        (
            before.dev(),
            before.ino(),
            before.len(),
            before.mtime(),
            before.mtime_nsec(),
            before.ctime(),
            before.ctime_nsec()
        )
    );
    assert_eq!(fs::read(&file).unwrap(), data());
    let healthy = manifest(2);
    let ack = receiver
        .receive_and_stage(&healthy, 0, response(&healthy))
        .await
        .unwrap();
    assert_eq!(ack.file(), &FileVersionRef::from_manifest(&healthy));
    assert_eq!(ack.chunk_index(), 0);
    assert_eq!(ack.byte_length(), 17);
    assert_eq!(ack.chunk_sha256(), &healthy.chunk(0).unwrap().sha256);
}

#[test]
fn cancelled_queued_staging_waiter_retains_received_buffer_and_control_until_real_job_finishes() {
    let root = Root::new();
    let runtime = Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let barrier = Barrier::new();
    let _release = ReleaseOnDrop(barrier.clone());
    let (entered, started) = mpsc::channel();
    let held = barrier.clone();
    let external = runtime.spawn_blocking(move || {
        entered.send(()).unwrap();
        held.wait();
    });
    started
        .recv_timeout(Duration::from_secs(10))
        .expect("external job really owns the only blocking thread");
    runtime.block_on(async {
        let original = manifest(1);
        let receiver = receiver(&root, &original);
        let reader = receiver
            .receive_checked(&original, 0, response(&original))
            .await
            .unwrap();
        let mut queued = Box::pin(receiver.stage_checked(&original, 0, reader));
        poll_fn(|cx| match queued.as_mut().poll(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("a real job behind the held blocking worker must stay queued"),
        })
        .await;
        full(&receiver, &original).await;
        assert_eq!(
            fs::read_dir(&root.0).unwrap().count(),
            0,
            "queued work has not begun file IO"
        );
        drop(queued);
        full(&receiver, &original).await;
        barrier.release();
        timeout(Duration::from_secs(10), external)
            .await
            .unwrap()
            .unwrap();
        restored(&receiver, &root, &original, None).await;
    });
}

#[test]
fn cancelled_running_staging_waiter_retains_received_buffer_and_control_until_real_job_finishes() {
    let root = Root::new();
    let runtime = Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let barrier = Barrier::new();
    let _release = ReleaseOnDrop(barrier.clone());
    let (entered, started) = mpsc::channel();
    runtime.block_on(async {
        let original = manifest(1);
        let receiver = receiver(&root, &original);
        let mut reader = receiver
            .receive_checked(&original, 0, response(&original))
            .await
            .unwrap();
        let held = barrier.clone();
        reader.observe_first_read(move || {
            entered.send(()).unwrap();
            held.wait();
        });
        let target = receiver.clone();
        let input = original.clone();
        let running = runtime.spawn(async move { target.stage_checked(&input, 0, reader).await });
        started
            .recv_timeout(Duration::from_secs(10))
            .expect("first Read happens inside the actual staging job");
        let created = fs::read_dir(&root.0)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            created.len(),
            1,
            "the actual stage job created an exclusive destination before first Read"
        );
        let metadata = created[0].metadata().unwrap();
        assert!(metadata.is_file());
        assert_eq!(metadata.len(), 0);
        assert_eq!(metadata.mode() & 0o7777, 0o600);
        assert_eq!(metadata.nlink(), 1);
        let created_inode = (metadata.dev(), metadata.ino());
        full(&receiver, &original).await;
        running.abort();
        assert!(running
            .await
            .err()
            .is_some_and(|error| error.is_cancelled()));
        full(&receiver, &original).await;
        barrier.release();
        restored(&receiver, &root, &original, Some(created_inode)).await;
    });
}

#[test]
fn cancelled_running_staging_waiter_also_retains_optional_caller_flow_until_original_inode_completes(
) {
    let root = Root::new();
    let runtime = Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let barrier = Barrier::new();
    let _release = ReleaseOnDrop(barrier.clone());
    let (entered, started) = mpsc::channel();
    runtime.block_on(async {
        let original = manifest(1);
        let receiver = receiver(&root, &original);
        let caller = Arc::new(Semaphore::new(1));
        let lease = caller.clone().try_acquire_owned().unwrap();
        let mut reader = receiver
            .receive_checked(&original, 0, response(&original))
            .await
            .unwrap();
        // This is the same private attachment used by receive_and_stage_leased, never a raw
        // public buffer/permit constructor or a replacement for the actual staging operation.
        reader.retain_caller(Some(lease));
        let held = barrier.clone();
        reader.observe_first_read(move || {
            entered.send(()).unwrap();
            held.wait();
        });
        let target = receiver.clone();
        let input = original.clone();
        let running = runtime.spawn(async move { target.stage_checked(&input, 0, reader).await });
        started
            .recv_timeout(Duration::from_secs(10))
            .expect("first Read occurs in the actual optional-caller staging job");
        let files = fs::read_dir(&root.0)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(files.len(), 1);
        let created = files[0].metadata().unwrap();
        assert_eq!(created.len(), 0);
        assert_eq!(created.mode() & 0o7777, 0o600);
        assert_eq!(created.nlink(), 1);
        let inode = (created.dev(), created.ino());
        full(&receiver, &original).await;
        assert!(caller.clone().try_acquire_owned().is_err());
        running.abort();
        assert!(running
            .await
            .err()
            .is_some_and(|error| error.is_cancelled()));
        full(&receiver, &original).await;
        assert_eq!(
            caller.available_permits(),
            0,
            "cancelling the waiter cannot release the caller flow lease"
        );
        barrier.release();
        let restored_caller = timeout(Duration::from_secs(10), caller.clone().acquire_owned())
            .await
            .unwrap()
            .unwrap();
        drop(restored_caller);
        restored(&receiver, &root, &original, Some(inode)).await;
        assert_eq!(caller.available_permits(), 1);
    });
}
