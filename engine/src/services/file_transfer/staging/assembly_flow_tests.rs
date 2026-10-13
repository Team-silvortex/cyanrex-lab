use super::{assemble::Phase, assembly_test_support::*, work_assemble, ChunkStagingError};
use std::{
    fs,
    future::Future,
    os::unix::fs::MetadataExt,
    sync::{mpsc, Arc},
    task::Context,
    time::Duration,
};
use tokio::sync::Semaphore;

#[test]
fn cancelled_running_assembly_retains_shared_download_flow_until_original_inode_completion() {
    let root = Root::new();
    let control = Root::new();
    let runtime = runtime();
    let bytes = b"whole inert bytes";
    let (staging, manifest) = staged(&runtime, &root, bytes);
    let original_root = root.identity();
    let original_sources = sources(&root, &manifest);
    let downloads = Arc::new(Semaphore::new(1));
    let parent = Arc::new(downloads.clone().try_acquire_owned().unwrap());
    let held = Barrier::new();
    let release = ReleaseOnDrop(held.clone());
    let (entered, started) = mpsc::channel();
    let mut entered = Some(entered);
    let mut pending = Box::pin(work_assemble::run_with_flow(
        &staging.inner,
        &manifest,
        move |phase| {
            if phase == Phase::Created {
                if let Some(entered) = entered.take() {
                    entered.send(()).unwrap();
                    held.wait();
                }
            }
        },
        Some(parent.clone()),
    ));
    let waker = waker();
    let mut context = Context::from_waker(&waker);
    {
        let _entered = runtime.enter();
        assert!(pending.as_mut().poll(&mut context).is_pending());
    }
    started.recv_timeout(Duration::from_secs(10)).unwrap();
    let created = fs::metadata(output(&root, &manifest)).unwrap();
    assert_eq!(created.len(), 0);
    assert_eq!(created.mode() & 0o7777, 0o600);
    assert_eq!(created.nlink(), 1);
    drop(parent);
    drop(pending);
    assert_eq!(downloads.available_permits(), 0);
    assert!(downloads.clone().try_acquire_owned().is_err());
    assert_eq!(staging.inner.workers.available_permits(), 0);
    assert!(matches!(
        runtime.block_on(staging.assemble_staged_file(&manifest)),
        Err(ChunkStagingError::CapacityExhausted)
    ));
    release.0.release();
    let flow_slot = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(10), downloads.clone().acquire_owned())
            .await
            .unwrap()
            .unwrap()
    });
    drop(flow_slot);
    restored(&runtime, &staging);
    verified_output(&root, &manifest, bytes, created.ino());
    assert_eq!(sources(&root, &manifest), original_sources);
    assert_eq!(root.identity(), original_root);
    let before = Stamp::of(&fs::metadata(output(&root, &manifest)).unwrap());
    assert!(matches!(
        runtime.block_on(staging.assemble_staged_file(&manifest)),
        Err(ChunkStagingError::Conflict)
    ));
    assert_eq!(
        Stamp::of(&fs::metadata(output(&root, &manifest)).unwrap()),
        before
    );
    assert_eq!(downloads.available_permits(), 1);
    healthy(&runtime, &control, bytes);
}
