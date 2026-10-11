//! Real queued blocking work, not a timing-based guess that a spawned waiter was dispatched.
use super::*;
use crate::services::file_transfer::inspect_stream;
use std::{
    fs,
    future::Future,
    io::{self, Cursor},
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
    sync::{mpsc, Condvar, Mutex},
    task::{Context, Wake, Waker},
    time::Duration,
};

struct Root(PathBuf);

impl Root {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("cyanrex-staging-queued-{}", uuid::Uuid::new_v4()));
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

struct HeldReader {
    began: Option<mpsc::Sender<()>>,
    barrier: Barrier,
    bytes: Cursor<&'static [u8]>,
}

impl Read for HeldReader {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if let Some(began) = self.began.take() {
            began.send(()).unwrap();
            self.barrier.wait();
        }
        self.bytes.read(bytes)
    }
}

struct Noop;
impl Wake for Noop {
    fn wake(self: Arc<Self>) {}
}

fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: uuid::Uuid::from_u128(1).try_into().unwrap(),
        workspace_id: uuid::Uuid::from_u128(2).try_into().unwrap(),
    }
}

fn manifest() -> FileTransferManifest {
    inspect_stream(
        scope(),
        uuid::Uuid::from_u128(3).try_into().unwrap(),
        uuid::Uuid::from_u128(4).try_into().unwrap(),
        "inert.bin",
        &mut Cursor::new(b"inert bytes"),
    )
    .unwrap()
}

#[test]
fn dropped_queued_waiter_retains_slot_until_actual_blocking_job_finishes() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let first_root = Root::new();
    let queued_root = Root::new();
    let first = ChunkStaging::open(scope(), &first_root.0, 1).unwrap();
    let queued = ChunkStaging::open(scope(), &queued_root.0, 1).unwrap();
    let manifest = manifest();
    let first_barrier = Barrier::new();
    let queued_barrier = Barrier::new();
    let first_release = ReleaseOnDrop(first_barrier.clone());
    let queued_release = ReleaseOnDrop(queued_barrier.clone());
    let (first_tx, first_rx) = mpsc::channel();
    let (queued_tx, queued_rx) = mpsc::channel();
    let mut first_waiter = Box::pin(first.stage_chunk(
        &manifest,
        0,
        HeldReader {
            began: Some(first_tx),
            barrier: first_barrier,
            bytes: Cursor::new(b"inert bytes"),
        },
    ));
    let waker = Waker::from(Arc::new(Noop));
    let mut context = Context::from_waker(&waker);
    {
        let _entered = runtime.enter();
        assert!(first_waiter.as_mut().poll(&mut context).is_pending());
    }
    // An actual reader callback proves the sole blocking thread is occupied, without a sleep.
    first_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let mut queued_waiter = Box::pin(queued.stage_chunk(
        &manifest,
        0,
        HeldReader {
            began: Some(queued_tx),
            barrier: queued_barrier,
            bytes: Cursor::new(b"inert bytes"),
        },
    ));
    {
        let _entered = runtime.enter();
        // Polling this owned future through Pending executes admission and blocking submission.
        assert!(queued_waiter.as_mut().poll(&mut context).is_pending());
    }
    assert_eq!(queued.inner.workers.available_permits(), 0);
    assert!(matches!(
        queued_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    drop(queued_waiter);
    assert_eq!(
        runtime.block_on(queued.stage_chunk(&manifest, 0, Cursor::new(b"inert bytes"))),
        Err(ChunkStagingError::CapacityExhausted)
    );
    first_release.0.release();
    runtime.block_on(first_waiter).unwrap();
    // Now the cancelled waiter's real job runs, and retains its permit during a second barrier.
    queued_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(queued.inner.workers.available_permits(), 0);
    assert_eq!(
        runtime.block_on(queued.verify_staged_chunk(&manifest, 0)),
        Err(ChunkStagingError::CapacityExhausted)
    );
    queued_release.0.release();
    let permit = runtime.block_on(async {
        tokio::time::timeout(
            Duration::from_secs(10),
            queued.inner.workers.clone().acquire_owned(),
        )
        .await
        .unwrap()
        .unwrap()
    });
    drop(permit);
    runtime
        .block_on(queued.verify_staged_chunk(&manifest, 0))
        .unwrap();
}
