use super::{test_support::*, *};
use crate::services::file_transfer::{
    staging::{ChunkStagingError, SaveTestPhase},
    verify_file,
};
use std::{
    fs,
    future::{poll_fn, Future},
    os::unix::fs::MetadataExt,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    task::Poll,
    time::Duration,
};
use tokio::{sync::Notify, time::timeout};

#[test]
fn cancelled_observed_empty_assembly_closes_stream_while_actual_queued_work_keeps_flow() {
    let source = Root::new();
    let destination = Root::new();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let held = Barrier::new();
    let release = ReleaseOnDrop(held.clone());
    let (entered, started) = mpsc::channel();
    let blocker = held.clone();
    let external = runtime.spawn_blocking(move || {
        entered.send(()).unwrap();
        blocker.wait();
    });
    started.recv_timeout(Duration::from_secs(10)).unwrap();
    runtime.block_on(async {
        let manifest = manifest(&[]);
        let server = Server::start(&manifest, Vec::new(), false).await;
        let mut client = client(&server.origin, &source, &manifest);
        let eof = Arc::new(AtomicBool::new(false));
        Arc::get_mut(&mut client.inner)
            .unwrap()
            .manifest_eof_observer = Some(eof.clone());
        let target = FileSaveTarget::open(&destination.0, "not-saved-empty.bin").unwrap();
        let (grant, upload) = ids();
        let expected = FileVersionRef::from_manifest(&manifest);
        let (observer, mut progress) = FileDeliveryObserver::channel();
        let mut pending =
            Box::pin(client.fetch_granted_and_save_observed(
                TOKEN, grant, upload, &expected, &target, observer,
            ));
        timeout(
            Duration::from_secs(10),
            poll_fn(|cx| match pending.as_mut().poll(cx) {
                Poll::Pending if eof.load(Ordering::SeqCst) => Poll::Ready(()),
                Poll::Pending => Poll::Pending,
                Poll::Ready(_) => panic!(
                    "matched empty metadata must dispatch actual assembly behind held worker"
                ),
            }),
        )
        .await
        .unwrap();
        let last = prefix(&progress, FileDeliveryPhase::Assembling, 0, 0, 0, Some(0));
        assert_eq!(server.metadata.load(Ordering::SeqCst), 1);
        assert!(server.chunks.lock().unwrap().is_empty());
        assert_eq!(fs::read_dir(&source.0).unwrap().count(), 0);
        drop(pending);
        closed(&mut progress, last).await;
        assert_eq!(client.inner.downloads.available_permits(), 0);
        assert_eq!(
            client
                .inner
                .staging
                .assemble_staged_file(&manifest)
                .await
                .err(),
            Some(ChunkStagingError::CapacityExhausted)
        );
        assert_eq!(
            client
                .fetch_granted_and_save(TOKEN, grant, upload, &expected, &target)
                .await
                .err(),
            Some(FileGrantedSaveError::CapacityExhausted)
        );
        release.0.release();
        timeout(Duration::from_secs(10), external)
            .await
            .unwrap()
            .unwrap();
        restored(&client).await;
        verify_output(&source, &manifest, &[]);
        assert_eq!(fs::read_dir(&source.0).unwrap().count(), 1);
        assert_eq!(fs::read_dir(&destination.0).unwrap().count(), 0);
        assert_eq!(
            progress.latest(),
            Some(last),
            "orphan assembly must not report Saving or Complete"
        );
        let (observer, mut retry) = FileDeliveryObserver::channel();
        assert_eq!(
            client
                .fetch_granted_and_save_observed(TOKEN, grant, upload, &expected, &target, observer)
                .await
                .err(),
            Some(FileGrantedSaveError::Download(
                FileGrantedDownloadError::Download(FileDownloadError::Assembly(
                    ChunkStagingError::Conflict
                ))
            ))
        );
        let retry_last = prefix(&retry, FileDeliveryPhase::Assembling, 0, 0, 0, Some(0));
        closed(&mut retry, retry_last).await;
        assert_eq!(fs::read_dir(&destination.0).unwrap().count(), 0);
        assert_eq!(server.metadata.load(Ordering::SeqCst), 2);
        assert!(server.chunks.lock().unwrap().is_empty());
        assert_eq!(client.inner.downloads.available_permits(), 1);
        source.unchanged();
        destination.unchanged();
        server.finish().await;
    });
}

#[test]
fn cancelled_observed_public_created_save_keeps_real_flow_but_cannot_publish_complete_later() {
    let source = Root::new();
    let destination = Root::new();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let held = Barrier::new();
    let release = ReleaseOnDrop(held.clone());
    runtime.block_on(async {
        let bytes = b"seventeen bytes!!";
        let manifest = manifest(bytes);
        let server = Server::start(&manifest, bytes.to_vec(), false).await;
        let client = client(&server.origin, &source, &manifest);
        let mut target = FileSaveTarget::open(&destination.0, "observed-save.bin").unwrap();
        let created = Arc::new(Notify::new());
        let signal = created.clone();
        let blocker = held.clone();
        target.set_test_observer(Arc::new(move |phase| {
            if phase == SaveTestPhase::Created { signal.notify_one(); blocker.wait(); }
        }));
        let (grant, upload) = ids();
        let expected = FileVersionRef::from_manifest(&manifest);
        let (observer, mut progress) = FileDeliveryObserver::channel();
        let mut pending = Box::pin(client.fetch_granted_and_save_observed(TOKEN, grant, upload, &expected, &target, observer));
        timeout(Duration::from_secs(10), async {
            tokio::select! {
                result = &mut pending => panic!("real Created save must remain held: {}", result.err().unwrap()),
                _ = created.notified() => (),
            }
        }).await.unwrap();
        let last = prefix(&progress, FileDeliveryPhase::Saving, 1, 17, 17, Some(1));
        let path = destination.0.join("observed-save.bin");
        let original = fs::symlink_metadata(&path).unwrap();
        assert_eq!((original.len(), original.mode() & 0o7777, original.nlink()), (0, 0o600, 1));
        verify_output(&source, &manifest, bytes);
        let original_source = tree(&source.0);
        drop(pending);
        closed(&mut progress, last).await;
        assert_eq!(client.inner.downloads.available_permits(), 0);
        assert_eq!(client.inner.manifests.available_permits(), 1);
        assert_eq!(client.inner.controls.available_permits(), 1);
        assert_eq!(client.fetch_granted_and_save(TOKEN, grant, upload, &expected, &target).await.err(),
            Some(FileGrantedSaveError::CapacityExhausted));
        assert_eq!(client.inner.staging.verify_staged_chunk(&manifest, 0).await, Err(ChunkStagingError::CapacityExhausted));
        drop(target);
        release.0.release();
        restored(&client).await;
        let completed = fs::symlink_metadata(&path).unwrap();
        assert_eq!((completed.dev(), completed.ino()), (original.dev(), original.ino()));
        assert_eq!((completed.mode() & 0o7777, completed.len(), completed.nlink(), completed.uid()),
            (0o400, 17, 1, fs::metadata(&destination.0).unwrap().uid()));
        let source_file = fs::metadata(source.0.join(name(&manifest, "f1", None))).unwrap();
        assert_ne!((completed.dev(), completed.ino()), (source_file.dev(), source_file.ino()));
        assert_eq!(fs::read(&path).unwrap(), bytes);
        verify_file(&manifest, &mut fs::File::open(&path).unwrap()).unwrap();
        assert_eq!(tree(&source.0), original_source);
        assert_eq!(progress.latest(), Some(last), "completion without the waiter is not a Complete observation");
        assert_eq!(*server.chunks.lock().unwrap(), vec![0]);
        assert_eq!(client.inner.staging.verify_staged_chunk(&manifest, 0).await, Ok(()));
        assert_eq!(client.inner.downloads.available_permits(), 1);
        assert_eq!(fs::read_dir(&destination.0).unwrap().count(), 1);
        source.unchanged(); destination.unchanged();
        server.finish().await;
    });
}
