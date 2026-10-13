use super::test_support::*;
use crate::services::file_transfer::{
    staging::{ChunkStagingError, SaveTestPhase},
    verify_file,
};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    path::PathBuf,
    sync::{atomic::Ordering, Arc, Mutex},
    time::Duration,
};
use tokio::{sync::Notify, time::timeout};

struct UserParent(PathBuf, (u64, u64, u32, u32, u32));
impl UserParent {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("cyanrex-user-delivery-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        let meta = fs::metadata(&path).unwrap();
        Self(
            path,
            (meta.dev(), meta.ino(), meta.uid(), meta.gid(), meta.mode()),
        )
    }
    fn unchanged_security(&self) {
        let meta = fs::metadata(&self.0).unwrap();
        assert_eq!(
            (meta.dev(), meta.ino(), meta.uid(), meta.gid(), meta.mode()),
            self.1
        );
    }
}
impl Drop for UserParent {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cancelled_public_user_directory_delivery_keeps_real_created_save_flow_and_original_inode() {
    let source = Root::new();
    let destination = UserParent::new();
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
        let mut target = FileSaveTarget::open_user_directory(&destination.0, LEAF).unwrap();
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
                result = &mut pending => panic!("actual user-directory Created job must remain held: {}", result.err().unwrap()),
                _ = created.notified() => (),
            }
        }).await.unwrap();
        let original = fs::symlink_metadata(destination.0.join(LEAF)).unwrap();
        assert!(original.is_file());
        assert_eq!((original.len(), original.mode() & 0o7777, original.nlink()), (0, 0o600, 1));
        assert_eq!(server.metadata.load(Ordering::SeqCst), 1);
        assert_eq!(*server.chunks.lock().unwrap(), vec![0]);
        drop(pending);
        assert_eq!(client.inner.downloads.available_permits(), 0);
        assert_eq!(client.inner.manifests.available_permits(), 1);
        assert_eq!(client.inner.controls.available_permits(), 1);
        assert_eq!(client.fetch_granted_and_save(TOKEN, grant, upload, &expected, &target).await.err(),
            Some(FileGrantedSaveError::CapacityExhausted));
        assert_eq!(client.fetch_granted_and_assemble(TOKEN, grant, upload, &expected).await.err(),
            Some(FileGrantedDownloadError::CapacityExhausted));
        assert_eq!(client.inner.staging.verify_staged_chunk(&manifest, 0).await.err(),
            Some(ChunkStagingError::CapacityExhausted));
        drop(target);
        destination.unchanged_security();
        assert_eq!(fs::metadata(destination.0.join(LEAF)).unwrap().len(), 0);
        release.0.release();
        restored(&client).await;
        let path = destination.0.join(LEAF);
        let completed = fs::symlink_metadata(&path).unwrap();
        let assembled = fs::metadata(source.0.join(name(&manifest, "f1", None))).unwrap();
        assert_eq!((completed.dev(), completed.ino()), (original.dev(), original.ino()));
        assert_ne!((completed.dev(), completed.ino()), (assembled.dev(), assembled.ino()));
        assert_eq!((completed.mode() & 0o7777, completed.len(), completed.nlink(), completed.uid()),
            (0o400, bytes.len() as u64, 1, fs::metadata(&destination.0).unwrap().uid()));
        assert_eq!(fs::read(&path).unwrap(), bytes);
        verify_file(&manifest, &mut fs::File::open(&path).unwrap()).unwrap();
        assert_eq!(fs::read_dir(&destination.0).unwrap().count(), 1);
        assert_eq!(tree(&source.0), *source_before.lock().unwrap().as_ref().unwrap());
        verify_output(&source, &manifest, bytes);
        destination.unchanged_security();
        assert_eq!(client.inner.staging.verify_staged_chunk(&manifest, 0).await, Ok(()));
        assert_eq!(client.inner.downloads.available_permits(), 1);
        assert_eq!(*server.chunks.lock().unwrap(), vec![0], "the orphan original job adds no later HTTP");
        let target = FileSaveTarget::open_user_directory(&destination.0, LEAF).unwrap();
        let before = tree(&destination.0);
        assert_eq!(client.fetch_granted_and_save(TOKEN, grant, upload, &expected, &target).await.err(),
            Some(FileGrantedSaveError::Download(FileGrantedDownloadError::Download(FileDownloadError::Chunk {
                index: 0, error: super::super::FileChunkClientError::Receive(
                    super::super::FileChunkReceiveError::Staging(ChunkStagingError::Conflict)),
            }))));
        assert_eq!(tree(&destination.0), before);
        destination.unchanged_security();
        assert_eq!(*server.chunks.lock().unwrap(), vec![0, 0]);
        server.finish().await;
    });
}
