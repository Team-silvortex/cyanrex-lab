use super::test_support::*;
use crate::services::file_transfer::staging::{ChunkStagingError, SaveTestPhase};
use std::{
    fs,
    os::unix::fs::MetadataExt,
    sync::{atomic::Ordering, mpsc, Arc, Mutex},
    time::Duration,
};
use tokio::{sync::Notify, time::timeout};

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap()
}

async fn still_held(
    client: &FileChunkClient,
    manifest: &super::super::FileTransferManifest,
    target: &FileSaveTarget,
) {
    let (grant, upload) = ids();
    let expected = FileVersionRef::from_manifest(manifest);
    assert_eq!(client.inner.downloads.available_permits(), 0);
    assert_eq!(client.inner.manifests.available_permits(), 1);
    assert_eq!(client.inner.controls.available_permits(), 1);
    assert_eq!(
        client
            .fetch_granted_and_save(TOKEN, grant, upload, &expected, target)
            .await
            .err(),
        Some(FileGrantedSaveError::CapacityExhausted)
    );
    assert_eq!(
        client
            .fetch_granted_and_assemble(TOKEN, grant, upload, &expected)
            .await
            .err(),
        Some(FileGrantedDownloadError::CapacityExhausted)
    );
    assert_eq!(
        client
            .inner
            .staging
            .verify_staged_chunk(manifest, 0)
            .await
            .err(),
        Some(ChunkStagingError::CapacityExhausted)
    );
}

#[test]
fn cancelled_public_delivery_queued_save_keeps_actual_job_flow_and_original_source_after_ack_drop()
{
    let source = Root::new();
    let destination = Root::new();
    let control_source = Root::new();
    let control_destination = Root::new();
    let runtime = runtime();
    let held = Barrier::new();
    let release = ReleaseOnDrop(held.clone());
    runtime.block_on(async {
        let bytes = b"seventeen bytes!!";
        let manifest = manifest(bytes);
        let server = Server::start(&manifest, bytes.to_vec(), false).await;
        let client = client(&server.origin, &source, &manifest);
        let mut target = FileSaveTarget::open(&destination.0, LEAF).unwrap();
        let dispatched = Arc::new(Notify::new());
        let observed = dispatched.clone();
        let external_job = Arc::new(Mutex::new(None));
        let job_slot = external_job.clone();
        let source_before = Arc::new(Mutex::new(None));
        let snapshot = source_before.clone();
        let source_path = source.0.clone();
        let blocker = held.clone();
        target.set_test_observer(Arc::new(move |phase| {
            if phase == SaveTestPhase::BeforeDispatch {
                *snapshot.lock().unwrap() = Some(tree(&source_path));
                let (entered, started) = mpsc::channel();
                let blocker = blocker.clone();
                let job = tokio::runtime::Handle::current().spawn_blocking(move || {
                    entered.send(()).unwrap();
                    blocker.wait();
                });
                started.recv_timeout(Duration::from_secs(10)).unwrap();
                *job_slot.lock().unwrap() = Some(job);
                observed.notify_one();
            }
        }));
        let (grant, upload) = ids();
        let expected = FileVersionRef::from_manifest(&manifest);
        let mut pending = Box::pin(client.fetch_granted_and_save(TOKEN, grant, upload, &expected, &target));
        timeout(Duration::from_secs(10), async {
            tokio::select! {
                result = &mut pending => panic!("actual exclusive save must be queued: {}", result.err().unwrap()),
                _ = dispatched.notified() => (),
            }
        }).await.unwrap();
        // The unique blocking thread is occupied before real public delivery submits its save.
        assert_eq!(server.metadata.load(Ordering::SeqCst), 1);
        assert_eq!(*server.chunks.lock().unwrap(), vec![0]);
        assert!(tree(&destination.0).is_empty());
        verify_output(&source, &manifest, bytes);
        assert_eq!(fs::read_dir(&source.0).unwrap().count(), 2);
        drop(pending); // Drops the inner original AssembledFile; only the actual save job pins it.
        still_held(&client, &manifest, &target).await;
        drop(target);
        assert_eq!(server.metadata.load(Ordering::SeqCst), 1);
        assert_eq!(*server.chunks.lock().unwrap(), vec![0]);
        release.0.release();
        let external = external_job.lock().unwrap().take().unwrap();
        timeout(Duration::from_secs(10), external).await.unwrap().unwrap();
        restored(&client).await;
        assert_eq!(tree(&source.0), *source_before.lock().unwrap().as_ref().unwrap());
        saved(&source, &destination, &manifest, bytes, None);
        assert_eq!(client.inner.staging.verify_staged_chunk(&manifest, 0).await, Ok(()));
        assert_eq!(client.inner.downloads.available_permits(), 1);
        let target = FileSaveTarget::open(&destination.0, LEAF).unwrap();
        let before = tree(&destination.0);
        assert_eq!(client.fetch_granted_and_save(TOKEN, grant, upload, &expected, &target).await.err(),
            Some(FileGrantedSaveError::Download(FileGrantedDownloadError::Download(FileDownloadError::Chunk {
                index: 0,
                error: super::super::FileChunkClientError::Receive(
                    super::super::FileChunkReceiveError::Staging(ChunkStagingError::Conflict)),
            }))));
        assert_eq!(tree(&destination.0), before);
        assert_eq!(tree(&source.0), *source_before.lock().unwrap().as_ref().unwrap());
        positive(&server.origin, &control_source, &control_destination, &manifest, bytes).await;
        assert_eq!(*server.chunks.lock().unwrap(), vec![0, 0, 0]);
        server.finish().await;
    });
}

#[test]
fn cancelled_public_delivery_created_save_retains_flow_and_same_inode_through_real_readback() {
    let source = Root::new();
    let destination = Root::new();
    let control_source = Root::new();
    let control_destination = Root::new();
    let runtime = runtime();
    let held = Barrier::new();
    let release = ReleaseOnDrop(held.clone());
    runtime.block_on(async {
        let bytes = b"seventeen bytes!!";
        let manifest = manifest(bytes);
        let server = Server::start(&manifest, bytes.to_vec(), false).await;
        let client = client(&server.origin, &source, &manifest);
        let mut target = FileSaveTarget::open(&destination.0, LEAF).unwrap();
        let created = Arc::new(Notify::new());
        let observed = created.clone();
        let source_before = Arc::new(Mutex::new(None));
        let snapshot = source_before.clone();
        let source_path = source.0.clone();
        let blocker = held.clone();
        target.set_test_observer(Arc::new(move |phase| match phase {
            SaveTestPhase::BeforeDispatch => *snapshot.lock().unwrap() = Some(tree(&source_path)),
            SaveTestPhase::Created => { observed.notify_one(); blocker.wait(); },
            _ => (),
        }));
        let (grant, upload) = ids();
        let expected = FileVersionRef::from_manifest(&manifest);
        let mut pending = Box::pin(client.fetch_granted_and_save(TOKEN, grant, upload, &expected, &target));
        timeout(Duration::from_secs(10), async {
            tokio::select! {
                result = &mut pending => panic!("real Created phase must hold its running save: {}", result.err().unwrap()),
                _ = created.notified() => (),
            }
        }).await.unwrap();
        let original = fs::symlink_metadata(destination.0.join(LEAF)).unwrap();
        assert!(original.is_file());
        assert_eq!((original.len(), original.mode() & 0o7777, original.nlink()), (0, 0o600, 1));
        let source_file = fs::metadata(source.0.join(name(&manifest, "f1", None))).unwrap();
        assert_ne!((original.dev(), original.ino()), (source_file.dev(), source_file.ino()));
        assert_eq!(server.metadata.load(Ordering::SeqCst), 1);
        assert_eq!(*server.chunks.lock().unwrap(), vec![0]);
        drop(pending);
        still_held(&client, &manifest, &target).await;
        drop(target);
        assert_eq!(fs::metadata(destination.0.join(LEAF)).unwrap().len(), 0);
        release.0.release();
        restored(&client).await;
        saved(&source, &destination, &manifest, bytes, Some(original.ino()));
        assert_eq!(tree(&source.0), *source_before.lock().unwrap().as_ref().unwrap());
        assert_eq!(client.inner.staging.verify_staged_chunk(&manifest, 0).await, Ok(()));
        assert_eq!(*server.chunks.lock().unwrap(), vec![0], "the detached original job makes no HTTP calls");
        positive(&server.origin, &control_source, &control_destination, &manifest, bytes).await;
        assert_eq!(*server.chunks.lock().unwrap(), vec![0, 0]);
        server.finish().await;
    });
}
