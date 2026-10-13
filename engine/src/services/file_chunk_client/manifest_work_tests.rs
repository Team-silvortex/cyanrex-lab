use super::{
    download_test_support::{
        client, ids, manifest, name, restored, verify_output, Barrier, ReleaseOnDrop, Root, TOKEN,
    },
    manifest_work_support::{positive, Server},
    *,
};
use crate::{
    models::collaboration::FILE_TRANSFER_CHUNK_BYTES, services::file_transfer::verify_chunk,
};
use std::{
    fs,
    future::{poll_fn, Future},
    os::unix::fs::MetadataExt,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    task::Poll,
};
use tokio::time::timeout;

#[tokio::test]
async fn cancelled_manifest_gap_releases_shared_metadata_and_parent_flow_without_dispatching_files()
{
    let root = Root::new();
    let bytes = b"seventeen bytes!!";
    let manifest = manifest(bytes);
    let server = Server::start(&manifest, bytes.to_vec(), true).await;
    let client = client(&server.origin, &root, &manifest);
    let clone = client.clone();
    let (grant, upload) = ids();
    let expected = FileVersionRef::from_manifest(&manifest);
    let mut pending = Box::pin(client.fetch_granted_and_assemble(TOKEN, grant, upload, &expected));
    timeout(Duration::from_secs(10), async {
        tokio::select! {
            result = &mut pending => panic!("real held manifest response must remain pending: {}", result.err().unwrap()),
            _ = server.entered.notified() => (),
        }
    }).await.unwrap();
    assert_eq!(server.metadata.load(Ordering::SeqCst), 1);
    assert_eq!(client.inner.manifests.available_permits(), 0);
    assert_eq!(client.inner.downloads.available_permits(), 0);
    assert_eq!(client.inner.controls.available_permits(), 1);
    assert_eq!(
        clone
            .fetch_manifest(TOKEN, grant, upload, &expected)
            .await
            .err(),
        Some(FileManifestClientError::CapacityExhausted)
    );
    assert_eq!(
        clone
            .fetch_granted_and_assemble(TOKEN, grant, upload, &expected)
            .await
            .err(),
        Some(FileGrantedDownloadError::CapacityExhausted)
    );
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
    assert!(server.chunks.lock().unwrap().is_empty());
    drop(pending);
    assert_eq!(client.inner.manifests.available_permits(), 1);
    assert_eq!(client.inner.downloads.available_permits(), 1);
    server.release.add_permits(1);
    assert_eq!(
        client
            .fetch_manifest(TOKEN, grant, upload, &expected)
            .await
            .unwrap(),
        manifest
    );
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
    let ack = client
        .fetch_granted_and_assemble(TOKEN, grant, upload, &expected)
        .await
        .unwrap();
    assert_eq!(ack.file(), &expected);
    verify_output(&root, &manifest, bytes);
    assert_eq!(*server.chunks.lock().unwrap(), vec![0]);
    assert_eq!(client.inner.manifests.available_permits(), 1);
    assert_eq!(client.inner.downloads.available_permits(), 1);
    server.finish().await;
}

#[test]
fn cancelled_granted_download_after_real_chunk_eof_keeps_queued_flow_and_never_requests_next() {
    let root = Root::new();
    let control = Root::new();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let held = Barrier::new();
    let _release = ReleaseOnDrop(held.clone());
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
        let mut client = client(&server.origin, &root, &manifest);
        let eof = Arc::new(AtomicBool::new(false));
        Arc::get_mut(&mut client.inner).unwrap().eof_observer = Some(eof.clone());
        let (grant, upload) = ids();
        let expected = FileVersionRef::from_manifest(&manifest);
        let mut pending =
            Box::pin(client.fetch_granted_and_assemble(TOKEN, grant, upload, &expected));
        timeout(
            Duration::from_secs(10),
            poll_fn(|cx| match pending.as_mut().poll(cx) {
                Poll::Pending if eof.load(Ordering::SeqCst) => Poll::Ready(()),
                Poll::Pending => Poll::Pending,
                Poll::Ready(_) => {
                    panic!("actual HTTP EOF and queued save JoinHandle must remain pending")
                }
            }),
        )
        .await
        .unwrap();
        assert_eq!(server.metadata.load(Ordering::SeqCst), 1);
        assert_eq!(*server.chunks.lock().unwrap(), vec![0]);
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
        assert_eq!(client.inner.downloads.available_permits(), 0);
        drop(pending);
        assert_eq!(
            client
                .fetch_granted_and_assemble(TOKEN, grant, upload, &expected)
                .await
                .err(),
            Some(FileGrantedDownloadError::CapacityExhausted)
        );
        // Metadata observation is independent of the orphan chunk's Client/save and flow gates.
        assert_eq!(
            client
                .fetch_manifest(TOKEN, grant, upload, &expected)
                .await
                .unwrap(),
            manifest
        );
        assert_eq!(
            client
                .fetch_and_stage(TOKEN, grant, upload, &manifest, 0)
                .await
                .err(),
            Some(FileChunkClientError::CapacityExhausted)
        );
        assert_eq!(*server.chunks.lock().unwrap(), vec![0]);
        held.release();
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
                    Err(error) => {
                        panic!("orphan real stage must finish exact original chunk: {error}")
                    }
                }
            }
        })
        .await
        .unwrap();
        let part = root.0.join(name(&manifest, "v1", Some(0)));
        let before = fs::metadata(&part).unwrap();
        assert_eq!(before.mode() & 0o7777, 0o400);
        assert_eq!(before.nlink(), 1);
        assert_eq!(
            fs::read(&part).unwrap(),
            bytes[..FILE_TRANSFER_CHUNK_BYTES as usize]
        );
        verify_chunk(&manifest, 0, &mut fs::File::open(&part).unwrap()).unwrap();
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1);
        assert!(!root.0.join(name(&manifest, "v1", Some(1))).exists());
        assert!(!root.0.join(name(&manifest, "f1", None)).exists());
        assert_eq!(*server.chunks.lock().unwrap(), vec![0]);
        root.unchanged();
        assert_eq!(
            client
                .fetch_granted_and_assemble(TOKEN, grant, upload, &expected)
                .await
                .err(),
            Some(FileGrantedDownloadError::Download(
                FileDownloadError::Chunk {
                    index: 0,
                    error: FileChunkClientError::Receive(FileChunkReceiveError::Staging(
                        ChunkStagingError::Conflict
                    ))
                }
            ))
        );
        let after = fs::metadata(&part).unwrap();
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
        positive(&server.origin, &control, &manifest, &bytes).await;
        assert_eq!(*server.chunks.lock().unwrap(), vec![0, 0, 0, 1]);
        server.finish().await;
    });
}

#[test]
fn cancelled_granted_empty_download_observes_remote_manifest_then_keeps_actual_queued_assembly() {
    let root = Root::new();
    let control = Root::new();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let held = Barrier::new();
    let _release = ReleaseOnDrop(held.clone());
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
        let mut client = client(&server.origin, &root, &manifest);
        let eof = Arc::new(AtomicBool::new(false));
        Arc::get_mut(&mut client.inner)
            .unwrap()
            .manifest_eof_observer = Some(eof.clone());
        let (grant, upload) = ids();
        let expected = FileVersionRef::from_manifest(&manifest);
        let mut pending =
            Box::pin(client.fetch_granted_and_assemble(TOKEN, grant, upload, &expected));
        timeout(
            Duration::from_secs(10),
            poll_fn(|cx| match pending.as_mut().poll(cx) {
                Poll::Pending if eof.load(Ordering::SeqCst) => Poll::Ready(()),
                Poll::Pending => Poll::Pending,
                Poll::Ready(_) => {
                    panic!("actual manifest EOF must enter queued empty assembly JoinHandle")
                }
            }),
        )
        .await
        .unwrap();
        assert_eq!(server.metadata.load(Ordering::SeqCst), 1);
        assert!(server.chunks.lock().unwrap().is_empty());
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
        assert_eq!(client.inner.manifests.available_permits(), 1);
        assert_eq!(client.inner.downloads.available_permits(), 0);
        drop(pending);
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
                .assemble_staged_file(&manifest)
                .await
                .err(),
            Some(ChunkStagingError::CapacityExhausted)
        );
        assert_eq!(server.metadata.load(Ordering::SeqCst), 1);
        held.release();
        timeout(Duration::from_secs(10), external)
            .await
            .unwrap()
            .unwrap();
        restored(&client).await;
        verify_output(&root, &manifest, &[]);
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1);
        assert_eq!(
            client
                .fetch_granted_and_assemble(TOKEN, grant, upload, &expected)
                .await
                .err(),
            Some(FileGrantedDownloadError::Download(
                FileDownloadError::Assembly(ChunkStagingError::Conflict)
            ))
        );
        positive(&server.origin, &control, &manifest, &[]).await;
        assert!(server.chunks.lock().unwrap().is_empty());
        assert_eq!(server.metadata.load(Ordering::SeqCst), 3);
        server.finish().await;
    });
}
