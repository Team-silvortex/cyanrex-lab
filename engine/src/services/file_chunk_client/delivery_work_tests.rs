use super::test_support::*;
use crate::{
    models::collaboration::FILE_TRANSFER_CHUNK_BYTES,
    services::{
        file_chunk_client::FileChunkClientError,
        file_chunk_receiver::FileChunkReceiveError,
        file_transfer::{staging::ChunkStagingError, verify_chunk},
    },
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
async fn cancelled_delivery_manifest_gap_releases_shared_flow_without_dispatching_or_saving() {
    let source = Root::new();
    let destination = Root::new();
    let bytes = b"seventeen bytes!!";
    let manifest = manifest(bytes);
    let server = Server::start(&manifest, bytes.to_vec(), true).await;
    let client = client(&server.origin, &source, &manifest);
    let clone = client.clone();
    let target = FileSaveTarget::open(&destination.0, LEAF).unwrap();
    let same_root = FileSaveTarget::open(&source.0, "never-created.bin").unwrap();
    let (grant, upload) = ids();
    let expected = FileVersionRef::from_manifest(&manifest);
    let mut pending =
        Box::pin(client.fetch_granted_and_save(TOKEN, grant, upload, &expected, &target));
    timeout(Duration::from_secs(10), async {
        tokio::select! {
            result = &mut pending => panic!("real held metadata must stay pending: {}", result.err().unwrap()),
            _ = server.entered.notified() => (),
        }
    }).await.unwrap();
    assert_eq!(server.metadata.load(Ordering::SeqCst), 1);
    assert_eq!(client.inner.manifests.available_permits(), 0);
    assert_eq!(client.inner.downloads.available_permits(), 0);
    assert_eq!(client.inner.controls.available_permits(), 1);
    assert_eq!(
        clone
            .fetch_granted_and_save(TOKEN, grant, upload, &expected, &target)
            .await
            .err(),
        Some(FileGrantedSaveError::CapacityExhausted)
    );
    assert_eq!(
        clone
            .fetch_granted_and_assemble(TOKEN, grant, upload, &expected)
            .await
            .err(),
        Some(FileGrantedDownloadError::CapacityExhausted)
    );
    assert_eq!(
        clone
            .fetch_granted_and_save(TOKEN, grant, upload, &expected, &same_root)
            .await
            .err(),
        Some(FileGrantedSaveError::InvalidInput),
        "pure root input rejection precedes a full flow gate"
    );
    assert_eq!(
        clone
            .fetch_granted_and_save(
                "00000000-0000-0000-0000-000000000000",
                grant,
                upload,
                &expected,
                &target
            )
            .await
            .err(),
        Some(FileGrantedSaveError::InvalidInput)
    );
    assert_eq!(server.metadata.load(Ordering::SeqCst), 1);
    assert!(server.chunks.lock().unwrap().is_empty());
    assert!(tree(&source.0).is_empty());
    assert!(tree(&destination.0).is_empty());
    drop(pending);
    assert_eq!(client.inner.manifests.available_permits(), 1);
    assert_eq!(client.inner.downloads.available_permits(), 1);
    server.release.add_permits(1);
    let ack = timeout(
        Duration::from_secs(10),
        client.fetch_granted_and_save(TOKEN, grant, upload, &expected, &target),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(ack.file(), &expected);
    assert_eq!(client.inner.downloads.available_permits(), 1);
    assert_eq!(server.metadata.load(Ordering::SeqCst), 2);
    assert_eq!(*server.chunks.lock().unwrap(), vec![0]);
    verify_output(&source, &manifest, bytes);
    saved(&source, &destination, &manifest, bytes, None);
    server.finish().await;
}

#[test]
fn cancelled_delivery_chunk_zero_after_real_eof_retains_job_and_never_requests_or_saves_next() {
    let source = Root::new();
    let destination = Root::new();
    let control_source = Root::new();
    let control_destination = Root::new();
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
        let target = FileSaveTarget::open(&destination.0, LEAF).unwrap();
        let (grant, upload) = ids();
        let expected = FileVersionRef::from_manifest(&manifest);
        let mut pending =
            Box::pin(client.fetch_granted_and_save(TOKEN, grant, upload, &expected, &target));
        timeout(
            Duration::from_secs(10),
            poll_fn(|cx| match pending.as_mut().poll(cx) {
                Poll::Pending if eof.load(Ordering::SeqCst) => Poll::Ready(()),
                Poll::Pending => Poll::Pending,
                Poll::Ready(_) => panic!("real chunk EOF must queue the actual stage JoinHandle"),
            }),
        )
        .await
        .unwrap();
        assert_eq!(server.metadata.load(Ordering::SeqCst), 1);
        assert_eq!(*server.chunks.lock().unwrap(), vec![0]);
        assert!(tree(&source.0).is_empty());
        assert!(tree(&destination.0).is_empty());
        assert_eq!(client.inner.downloads.available_permits(), 0);
        drop(pending);
        assert_eq!(
            client
                .fetch_granted_and_save(TOKEN, grant, upload, &expected, &target)
                .await
                .err(),
            Some(FileGrantedSaveError::CapacityExhausted)
        );
        assert_eq!(
            client
                .fetch_and_assemble(TOKEN, grant, upload, &manifest)
                .await
                .err(),
            Some(FileDownloadError::CapacityExhausted)
        );
        assert_eq!(
            client
                .fetch_and_stage(TOKEN, grant, upload, &manifest, 0)
                .await
                .err(),
            Some(FileChunkClientError::CapacityExhausted)
        );
        release.0.release();
        timeout(Duration::from_secs(10), external)
            .await
            .unwrap()
            .unwrap();
        restored(&client).await;
        timeout(Duration::from_secs(10), async {
            loop {
                match client.inner.staging.verify_staged_chunk(&manifest, 0).await {
                    Ok(()) => break,
                    Err(ChunkStagingError::CapacityExhausted) => tokio::task::yield_now().await,
                    Err(error) => panic!("original orphan stage must finish exactly: {error}"),
                }
            }
        })
        .await
        .unwrap();
        let part = source.0.join(name(&manifest, "v1", Some(0)));
        let before = tree(&source.0);
        let metadata = fs::metadata(&part).unwrap();
        assert_eq!((metadata.mode() & 0o7777, metadata.nlink()), (0o400, 1));
        assert_eq!(
            fs::read(&part).unwrap(),
            bytes[..FILE_TRANSFER_CHUNK_BYTES as usize]
        );
        verify_chunk(&manifest, 0, &mut fs::File::open(part).unwrap()).unwrap();
        assert_eq!(fs::read_dir(&source.0).unwrap().count(), 1);
        assert!(!source.0.join(name(&manifest, "v1", Some(1))).exists());
        assert!(!source.0.join(name(&manifest, "f1", None)).exists());
        assert!(tree(&destination.0).is_empty());
        assert_eq!(
            *server.chunks.lock().unwrap(),
            vec![0],
            "no orphan continuation may fetch or save"
        );
        assert_eq!(
            client
                .fetch_granted_and_save(TOKEN, grant, upload, &expected, &target)
                .await
                .err(),
            Some(FileGrantedSaveError::Download(
                FileGrantedDownloadError::Download(FileDownloadError::Chunk {
                    index: 0,
                    error: FileChunkClientError::Receive(FileChunkReceiveError::Staging(
                        ChunkStagingError::Conflict
                    )),
                })
            ))
        );
        assert_eq!(tree(&source.0), before);
        assert!(tree(&destination.0).is_empty());
        positive(
            &server.origin,
            &control_source,
            &control_destination,
            &manifest,
            &bytes,
        )
        .await;
        assert_eq!(*server.chunks.lock().unwrap(), vec![0, 0, 0, 1]);
        source.unchanged();
        destination.unchanged();
        server.finish().await;
    });
}
