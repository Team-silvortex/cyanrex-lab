use super::{test_support::*, *};
use crate::{
    models::collaboration::FILE_TRANSFER_CHUNK_BYTES,
    services::file_transfer::{staging::ChunkStagingError, verify_chunk},
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
async fn cancelled_observed_manifest_wait_closes_observation_without_dispatching_files() {
    let source = Root::new();
    let destination = Root::new();
    let bytes = b"seventeen bytes!!";
    let manifest = manifest(bytes);
    let server = Server::start(&manifest, bytes.to_vec(), true).await;
    let client = client(&server.origin, &source, &manifest);
    let target = FileSaveTarget::open(&destination.0, "observed.bin").unwrap();
    let (grant, upload) = ids();
    let expected = FileVersionRef::from_manifest(&manifest);
    let (observer, mut progress) = FileDeliveryObserver::channel();
    assert_eq!(progress.latest(), None);
    let mut pending = Box::pin(
        client.fetch_granted_and_save_observed(TOKEN, grant, upload, &expected, &target, observer),
    );
    timeout(Duration::from_secs(10), async {
        tokio::select! {
            result = &mut pending => panic!("actual held metadata must remain pending: {}", result.err().unwrap()),
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
    assert_eq!(server.metadata.load(Ordering::SeqCst), 1);
    assert_eq!(client.inner.manifests.available_permits(), 0);
    assert_eq!(client.inner.downloads.available_permits(), 0);
    assert_eq!(client.inner.controls.available_permits(), 1);
    assert_eq!(fs::read_dir(&source.0).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&destination.0).unwrap().count(), 0);
    assert!(server.chunks.lock().unwrap().is_empty());
    drop(pending);
    closed(&mut progress, last).await;
    assert_eq!(client.inner.manifests.available_permits(), 1);
    assert_eq!(client.inner.downloads.available_permits(), 1);
    server.release.add_permits(1);
    let (observer, mut fresh) = FileDeliveryObserver::channel();
    let saved = timeout(
        Duration::from_secs(10),
        client.fetch_granted_and_save_observed(TOKEN, grant, upload, &expected, &target, observer),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(saved.file(), &expected);
    drain_complete(&mut fresh, 17, 1).await;
    assert_eq!(
        progress.latest(),
        Some(last),
        "another call must not revive the old observation"
    );
    verify_output(&source, &manifest, bytes);
    let path = destination.0.join("observed.bin");
    let output = fs::symlink_metadata(&path).unwrap();
    assert_eq!(
        (
            output.mode() & 0o7777,
            output.nlink(),
            output.len(),
            output.uid()
        ),
        (0o400, 1, 17, fs::metadata(&destination.0).unwrap().uid())
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    crate::services::file_transfer::verify_file(&manifest, &mut fs::File::open(path).unwrap())
        .unwrap();
    assert_eq!(client.inner.downloads.available_permits(), 1);
    source.unchanged();
    destination.unchanged();
    server.finish().await;
}

#[test]
fn cancelled_observed_chunk_eof_keeps_real_queued_job_but_never_claims_unobserved_ack_prefix() {
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
        let target = FileSaveTarget::open(&destination.0, "never-saved.bin").unwrap();
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
                Poll::Ready(_) => {
                    panic!("actual HTTP EOF must dispatch queued stage and await its JoinHandle")
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
        assert_eq!(server.metadata.load(Ordering::SeqCst), 1);
        assert_eq!(*server.chunks.lock().unwrap(), vec![0]);
        assert_eq!(fs::read_dir(&source.0).unwrap().count(), 0);
        assert_eq!(fs::read_dir(&destination.0).unwrap().count(), 0);
        drop(pending);
        closed(&mut progress, last).await;
        assert_eq!(client.inner.downloads.available_permits(), 0);
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
        // Flow release can precede the enclosing stage closure's worker-permit drop.
        // Drain that tail with the real read-only verifier, retaining every non-capacity error.
        let verified = timeout(Duration::from_secs(10), async {
            loop {
                match client.inner.staging.verify_staged_chunk(&manifest, 0).await {
                    Err(ChunkStagingError::CapacityExhausted) => tokio::task::yield_now().await,
                    result => return result,
                }
            }
        })
        .await
        .expect("the original staging worker must actually finish within the bounded drain");
        assert_eq!(verified, Ok(()));
        let part = source.0.join(name(&manifest, "v1", Some(0)));
        let metadata = fs::metadata(&part).unwrap();
        assert_eq!(
            (metadata.mode() & 0o7777, metadata.len(), metadata.nlink()),
            (0o400, FILE_TRANSFER_CHUNK_BYTES, 1)
        );
        assert_eq!(
            fs::read(&part).unwrap(),
            bytes[..FILE_TRANSFER_CHUNK_BYTES as usize]
        );
        verify_chunk(&manifest, 0, &mut fs::File::open(part).unwrap()).unwrap();
        assert_eq!(fs::read_dir(&source.0).unwrap().count(), 1);
        assert!(!source.0.join(name(&manifest, "v1", Some(1))).exists());
        assert!(!source.0.join(name(&manifest, "f1", None)).exists());
        assert_eq!(fs::read_dir(&destination.0).unwrap().count(), 0);
        assert_eq!(
            progress.latest(),
            Some(last),
            "finished orphan work is not an orchestrator ACK"
        );
        assert_eq!(
            *server.chunks.lock().unwrap(),
            vec![0],
            "no dropped continuation may request the next chunk"
        );
        assert_eq!(client.inner.downloads.available_permits(), 1);
        assert_eq!(client.inner.manifests.available_permits(), 1);
        assert_eq!(client.inner.controls.available_permits(), 1);
        source.unchanged();
        destination.unchanged();
        server.finish().await;
    });
}
