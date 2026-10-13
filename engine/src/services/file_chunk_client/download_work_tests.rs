use super::{download_test_support::*, *};
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

#[test]
fn cancelled_whole_download_after_http_eof_keeps_queued_chunk_flow_and_never_fetches_next() {
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
    let external_barrier = held.clone();
    let external = runtime.spawn_blocking(move || {
        entered.send(()).unwrap();
        external_barrier.wait();
    });
    started.recv_timeout(Duration::from_secs(10)).unwrap();
    runtime.block_on(async {
        let mut bytes = vec![7; FILE_TRANSFER_CHUNK_BYTES as usize];
        bytes.extend_from_slice(b"last inert bytes!");
        let manifest = manifest(&bytes);
        let server = Server::start(&manifest, bytes.clone()).await;
        let mut client = client(&server.origin, &root, &manifest);
        let eof = Arc::new(AtomicBool::new(false));
        Arc::get_mut(&mut client.inner).unwrap().eof_observer = Some(eof.clone());
        let (grant, upload) = ids();
        let mut pending = Box::pin(client.fetch_and_assemble(TOKEN, grant, upload, &manifest));
        timeout(
            Duration::from_secs(10),
            poll_fn(|cx| match pending.as_mut().poll(cx) {
                Poll::Pending if eof.load(Ordering::SeqCst) => Poll::Ready(()),
                Poll::Pending => Poll::Pending,
                Poll::Ready(_) => {
                    panic!("actual HTTP EOF followed by save JoinHandle must be queued")
                }
            }),
        )
        .await
        .unwrap();
        assert_eq!(*server.hits.lock().unwrap(), vec![0]);
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
        assert_eq!(client.inner.downloads.available_permits(), 0);
        drop(pending);
        assert_eq!(
            timeout(
                Duration::from_secs(10),
                client.fetch_and_assemble(TOKEN, grant, upload, &manifest)
            )
            .await
            .unwrap()
            .err(),
            Some(FileDownloadError::CapacityExhausted)
        );
        assert_eq!(
            timeout(
                Duration::from_secs(10),
                client.fetch_and_stage(TOKEN, grant, upload, &manifest, 0)
            )
            .await
            .unwrap()
            .err(),
            Some(FileChunkClientError::CapacityExhausted)
        );
        assert_eq!(*server.hits.lock().unwrap(), vec![0]);
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
                    Err(error) => panic!("cancelled queued chunk must finish unchanged: {error}"),
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
        assert_eq!(
            *server.hits.lock().unwrap(),
            vec![0],
            "orphan work must not continue async download"
        );
        root.unchanged();
        assert_eq!(
            timeout(
                Duration::from_secs(10),
                client.fetch_and_assemble(TOKEN, grant, upload, &manifest)
            )
            .await
            .unwrap()
            .err(),
            Some(FileDownloadError::Chunk {
                index: 0,
                error: FileChunkClientError::Receive(FileChunkReceiveError::Staging(
                    ChunkStagingError::Conflict
                ))
            })
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
        assert_eq!(*server.hits.lock().unwrap(), vec![0, 0]);
        positive(&server.origin, &control, &manifest, &bytes).await;
        assert_eq!(*server.hits.lock().unwrap(), vec![0, 0, 0, 1]);
        server.finish().await;
    });
}

#[test]
fn cancelled_empty_download_keeps_actual_queued_assembly_flow_with_zero_http() {
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
    let external_barrier = held.clone();
    let external = runtime.spawn_blocking(move || {
        entered.send(()).unwrap();
        external_barrier.wait();
    });
    started.recv_timeout(Duration::from_secs(10)).unwrap();
    runtime.block_on(async {
        let manifest = manifest(&[]);
        let server = Server::start(&manifest, Vec::new()).await;
        let client = client(&server.origin, &root, &manifest);
        let (grant, upload) = ids();
        let mut pending = Box::pin(client.fetch_and_assemble(TOKEN, grant, upload, &manifest));
        poll_fn(|cx| match pending.as_mut().poll(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => {
                panic!("empty whole flow must submit actual assembly behind held blocking worker")
            }
        })
        .await;
        assert_eq!(client.inner.downloads.available_permits(), 0);
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
        drop(pending);
        assert_eq!(
            timeout(
                Duration::from_secs(10),
                client.fetch_and_assemble(TOKEN, grant, upload, &manifest)
            )
            .await
            .unwrap()
            .err(),
            Some(FileDownloadError::CapacityExhausted)
        );
        assert_eq!(
            timeout(
                Duration::from_secs(10),
                client.inner.staging.assemble_staged_file(&manifest)
            )
            .await
            .unwrap()
            .err(),
            Some(ChunkStagingError::CapacityExhausted)
        );
        assert!(server.hits.lock().unwrap().is_empty());
        held.release();
        timeout(Duration::from_secs(10), external)
            .await
            .unwrap()
            .unwrap();
        restored(&client).await;
        verify_output(&root, &manifest, &[]);
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1);
        assert!(server.hits.lock().unwrap().is_empty());
        assert_eq!(
            client
                .fetch_and_assemble(TOKEN, grant, upload, &manifest)
                .await
                .err(),
            Some(FileDownloadError::Assembly(ChunkStagingError::Conflict))
        );
        positive(&server.origin, &control, &manifest, &[]).await;
        assert!(server.hits.lock().unwrap().is_empty());
        server.finish().await;
    });
}
