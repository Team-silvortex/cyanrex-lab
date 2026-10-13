use super::{test_support::*, *};
use crate::{
    models::collaboration::FILE_TRANSFER_CHUNK_BYTES,
    services::file_transfer::{staging::ChunkStagingError, verify_chunk, verify_file},
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
use tokio::time::timeout;

#[tokio::test]
async fn requested_metadata_cancellation_returns_typed_result_closes_observer_and_restores_no_job_capacity(
) {
    let source = Root::new();
    let destination = Root::new();
    let bytes = b"seventeen bytes!!";
    let manifest = manifest(bytes);
    let server = Server::start(&manifest, bytes.to_vec(), true).await;
    let client = client(&server.origin, &source, &manifest);
    let target = FileSaveTarget::open(&destination.0, "controlled.bin").unwrap();
    let (grant, upload) = ids();
    let expected = FileVersionRef::from_manifest(&manifest);
    let (observer, mut progress) = FileDeliveryObserver::channel();
    let (handle, cancellation) = FileDeliveryCancelHandle::channel();
    let mut pending = Box::pin(client.fetch_granted_and_save_controlled(
        TOKEN,
        grant,
        upload,
        &expected,
        &target,
        observer,
        cancellation,
    ));
    timeout(Duration::from_secs(10), async {
        tokio::select! {
            result = &mut pending => panic!("actual held metadata must stay pending: {}", result.err().unwrap()),
            _ = server.entered.notified() => (),
        }
    }).await.unwrap();
    let last = prefix(
        &progress,
        FileDeliveryPhase::FetchingManifest,
        0,
        0,
        17,
        None,
    );
    assert_eq!(client.inner.downloads.available_permits(), 0);
    assert_eq!(client.inner.manifests.available_permits(), 0);
    assert_eq!(server.metadata.load(Ordering::SeqCst), 1);
    assert!(server.chunks.lock().unwrap().is_empty());
    assert!(tree(&source.0).is_empty() && tree(&destination.0).is_empty());
    handle.cancel();
    assert_eq!(
        timeout(Duration::from_secs(10), &mut pending)
            .await
            .unwrap()
            .err(),
        Some(FileControlledSaveError::Cancelled)
    );
    drop(pending);
    closed(&mut progress, last).await;
    assert_eq!(client.inner.downloads.available_permits(), 1);
    assert_eq!(client.inner.manifests.available_permits(), 1);
    assert_eq!(client.inner.controls.available_permits(), 1);
    assert!(tree(&source.0).is_empty() && tree(&destination.0).is_empty());
    server.release.add_permits(1);
    // Handle drop is not implicit cancellation; the actual same-client path must still save.
    let (handle, cancellation) = FileDeliveryCancelHandle::channel();
    drop(handle);
    let (observer, mut fresh) = FileDeliveryObserver::channel();
    let saved = timeout(
        Duration::from_secs(10),
        client.fetch_granted_and_save_controlled(
            TOKEN,
            grant,
            upload,
            &expected,
            &target,
            observer,
            cancellation,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(saved.file(), &expected);
    let complete = prefix(&fresh, FileDeliveryPhase::Complete, 1, 17, 17, Some(1));
    closed(&mut fresh, complete).await;
    verify_output(&source, &manifest, bytes);
    let path = destination.0.join("controlled.bin");
    let metadata = fs::symlink_metadata(&path).unwrap();
    assert_eq!(
        (
            metadata.mode() & 0o7777,
            metadata.len(),
            metadata.nlink(),
            metadata.uid()
        ),
        (0o400, 17, 1, fs::metadata(&destination.0).unwrap().uid())
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    verify_file(&manifest, &mut fs::File::open(&path).unwrap()).unwrap();
    assert_eq!(progress.latest(), Some(last));
    assert_eq!(server.metadata.load(Ordering::SeqCst), 2);
    assert_eq!(*server.chunks.lock().unwrap(), vec![0]);
    assert_eq!(client.inner.downloads.available_permits(), 1);
    source.unchanged();
    destination.unchanged();
    server.finish().await;
}

#[test]
fn requested_chunk_eof_cancellation_keeps_actual_queued_stage_and_never_requests_next_chunk() {
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
        let mut bytes = vec![7; FILE_TRANSFER_CHUNK_BYTES as usize];
        bytes.extend_from_slice(b"seventeen bytes!!");
        let manifest = manifest(&bytes);
        let server = Server::start(&manifest, bytes.clone(), false).await;
        let mut client = client(&server.origin, &source, &manifest);
        let eof = Arc::new(AtomicBool::new(false));
        Arc::get_mut(&mut client.inner).unwrap().eof_observer = Some(eof.clone());
        let target = FileSaveTarget::open(&destination.0, "never-controlled.bin").unwrap();
        let (grant, upload) = ids();
        let expected = FileVersionRef::from_manifest(&manifest);
        let (observer, mut progress) = FileDeliveryObserver::channel();
        let (handle, cancellation) = FileDeliveryCancelHandle::channel();
        let mut pending = Box::pin(client.fetch_granted_and_save_controlled(
            TOKEN,
            grant,
            upload,
            &expected,
            &target,
            observer,
            cancellation,
        ));
        timeout(
            Duration::from_secs(10),
            poll_fn(|cx| match pending.as_mut().poll(cx) {
                Poll::Pending if eof.load(Ordering::SeqCst) => Poll::Ready(()),
                Poll::Pending => Poll::Pending,
                Poll::Ready(_) => {
                    panic!("real HTTP EOF must submit stage behind the actually held worker")
                }
            }),
        )
        .await
        .unwrap();
        let last = prefix(
            &progress,
            FileDeliveryPhase::StagingChunks,
            0,
            0,
            manifest.byte_length(),
            Some(2),
        );
        assert_eq!(*server.chunks.lock().unwrap(), vec![0]);
        assert!(tree(&source.0).is_empty() && tree(&destination.0).is_empty());
        assert_eq!(
            client.inner.staging.verify_staged_chunk(&manifest, 0).await,
            Err(ChunkStagingError::CapacityExhausted)
        );
        handle.cancel();
        assert_eq!(
            timeout(Duration::from_secs(10), &mut pending)
                .await
                .unwrap()
                .err(),
            Some(FileControlledSaveError::Cancelled)
        );
        drop(pending);
        closed(&mut progress, last).await;
        assert_eq!(client.inner.downloads.available_permits(), 0);
        assert_eq!(client.inner.controls.available_permits(), 0);
        assert_eq!(
            client
                .fetch_granted_and_save(TOKEN, grant, upload, &expected, &target)
                .await
                .err(),
            Some(FileGrantedSaveError::CapacityExhausted)
        );
        assert_eq!(
            client.inner.staging.verify_staged_chunk(&manifest, 0).await,
            Err(ChunkStagingError::CapacityExhausted)
        );
        release.0.release();
        timeout(Duration::from_secs(10), external)
            .await
            .unwrap()
            .unwrap();
        restored(&client).await;
        assert_eq!(
            verify_after_worker_drain(&client, &manifest, 0).await,
            Ok(())
        );
        let path = source.0.join(name(&manifest, "v1", Some(0)));
        let metadata = fs::metadata(&path).unwrap();
        assert_eq!(
            (metadata.mode() & 0o7777, metadata.len(), metadata.nlink()),
            (0o400, FILE_TRANSFER_CHUNK_BYTES, 1)
        );
        assert_eq!(
            fs::read(&path).unwrap(),
            bytes[..FILE_TRANSFER_CHUNK_BYTES as usize]
        );
        verify_chunk(&manifest, 0, &mut fs::File::open(&path).unwrap()).unwrap();
        assert_eq!(tree(&source.0).len(), 1);
        assert!(tree(&destination.0).is_empty());
        assert!(!source.0.join(name(&manifest, "v1", Some(1))).exists());
        assert!(!source.0.join(name(&manifest, "f1", None)).exists());
        assert_eq!(progress.latest(), Some(last));
        assert_eq!(
            *server.chunks.lock().unwrap(),
            vec![0],
            "cancelled continuation cannot ask for chunk one"
        );
        assert_eq!(client.inner.downloads.available_permits(), 1);
        assert_eq!(client.inner.controls.available_permits(), 1);
        assert_eq!(client.inner.manifests.available_permits(), 1);
        source.unchanged();
        destination.unchanged();
        server.finish().await;
    });
}
