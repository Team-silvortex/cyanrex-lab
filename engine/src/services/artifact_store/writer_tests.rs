use super::*;
use std::{
    fs::{self, Permissions},
    future::{poll_fn, Future},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    task::Poll,
};
use tokio::sync::Semaphore;

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("cyanrex-artifact-writer-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
    fn files(&self) -> Vec<PathBuf> {
        let mut files = fs::read_dir(&self.0)
            .unwrap()
            .map(|item| item.unwrap().path())
            .collect::<Vec<_>>();
        files.sort();
        files
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: "00000000-0000-4000-8000-000000000021".parse().unwrap(),
        workspace_id: "00000000-0000-4000-8000-000000000022".parse().unwrap(),
    }
}
fn writer(root: &Root, workers: Arc<Semaphore>) -> ArtifactContentWriter {
    let original = open_private_artifact_root(&root.0).unwrap();
    ArtifactContentWriter::from_reader(
        ArtifactContentReader::from_retained_root(scope(), &root.0, &original, workers).unwrap(),
    )
}
fn revision(bytes: &[u8]) -> ArtifactRevision {
    ArtifactRevision {
        schema_version: CoreSchemaVersion,
        reference: ArtifactRef {
            workspace: scope(),
            artifact_id: "00000000-0000-4000-8000-000000000023".parse().unwrap(),
            revision_id: "00000000-0000-4000-8000-000000000024".parse().unwrap(),
            sha256: blob::digest(bytes),
        },
        owner: PrincipalRef {
            authority_id: scope().authority_id,
            principal_id: "00000000-0000-4000-8000-000000000025".parse().unwrap(),
        },
        sequence: 1.try_into().unwrap(),
        parent: None,
        kind: "cyanrex.task-text".parse().unwrap(),
        title: "Discussion notes".into(),
        media_type: "text/plain".into(),
        byte_length: bytes.len() as u64,
        created_at: chrono::Utc::now(),
    }
}

#[tokio::test]
async fn writer_keeps_original_root_and_never_adopts_a_replacement() {
    let root = Root::new();
    let value = writer(&root, Arc::new(Semaphore::new(1)));
    assert_eq!(value.scope(), scope());
    value.verify_root().await.unwrap();
    let hidden = root.0.with_extension("original");
    fs::rename(&root.0, &hidden).unwrap();
    fs::create_dir(&root.0).unwrap();
    fs::set_permissions(&root.0, Permissions::from_mode(0o700)).unwrap();
    let refused = value.verify_root().await;
    assert!(root.files().is_empty());
    fs::remove_dir(&root.0).unwrap();
    fs::rename(&hidden, &root.0).unwrap();
    assert_eq!(refused, Err(ArtifactStoreError::BlobUnavailable));
    value.verify_root().await.unwrap();
}

#[tokio::test]
async fn host_writer_exclusive_publication_syncs_and_reads_back_actual_bytes() {
    let root = Root::new();
    let value = writer(&root, Arc::new(Semaphore::new(1)));
    let bytes = "Notes, not executable code.\n讨论\t".as_bytes().to_vec();
    let expected = revision(&bytes);
    value.write_blob(&expected, bytes.clone()).await.unwrap();
    assert_eq!(value.reader.read_blob(&expected).await.unwrap(), bytes);
    let files = root.files();
    assert_eq!(files.len(), 1);
    let meta = fs::metadata(&files[0]).unwrap();
    assert!(meta.is_file());
    assert_eq!(meta.mode() & 0o7777, 0o400);
    assert_eq!(meta.nlink(), 1);
    assert_eq!(meta.len(), expected.byte_length);
    assert_eq!(fs::read(&files[0]).unwrap(), bytes);
    assert_eq!(
        value.write_blob(&expected, bytes.clone()).await,
        Err(ArtifactStoreError::Conflict)
    );
    assert_eq!(root.files(), files);
    assert_eq!(fs::read(&files[0]).unwrap(), bytes);
}

#[tokio::test]
async fn host_writer_full_admission_never_dispatches_or_creates_a_file() {
    let root = Root::new();
    let value = writer(&root, Arc::new(Semaphore::new(0)));
    let bytes = b"bounded publication".to_vec();
    assert_eq!(
        value.write_blob(&revision(&bytes), bytes).await,
        Err(ArtifactStoreError::StorageUnavailable)
    );
    assert!(root.files().is_empty());
}

#[tokio::test]
async fn writer_rejects_inconsistent_bytes_without_creating_or_adopting_a_file() {
    let root = Root::new();
    let value = writer(&root, Arc::new(Semaphore::new(1)));
    for bytes in [b"short".to_vec(), vec![b'x'; b"expected publication".len()]] {
        let expected = revision(b"expected publication");
        assert_eq!(
            value.write_blob(&expected, bytes).await,
            Err(ArtifactStoreError::InvalidInput)
        );
        assert!(root.files().is_empty());
    }
}

#[tokio::test]
async fn retained_and_original_store_writers_preserve_distinct_lock_and_worker_policies() {
    let root = Root::new();
    let retained = writer(&root, Arc::new(Semaphore::new(0)));
    assert!(!retained.lock_head);
    let pool = crate::sqlx_compat::PgPoolOptions::new()
        .connect_lazy("postgresql://unconnected@localhost/unused_writer_fixture")
        .unwrap();
    pool.close().await;
    let store = ArtifactStore::open(pool, scope(), &root.0).unwrap();
    let original = ArtifactContentWriter::from_store(&store);
    assert!(original.lock_head);
    let bytes = b"original store has no retained-host worker gate".to_vec();
    let expected = revision(&bytes);
    assert_eq!(
        retained.write_blob(&expected, bytes.clone()).await,
        Err(ArtifactStoreError::StorageUnavailable)
    );
    assert!(root.files().is_empty());
    original.write_blob(&expected, bytes.clone()).await.unwrap();
    assert_eq!(original.reader.read_blob(&expected).await.unwrap(), bytes);
}

struct Release(Option<std::sync::mpsc::Sender<()>>);
impl Release {
    fn send(&mut self) {
        self.0.take().unwrap().send(()).unwrap();
    }
}
impl Drop for Release {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

#[test]
fn cancelled_queued_writer_keeps_capacity_until_real_file_work_finishes() {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .max_blocking_threads(1)
        .build()
        .unwrap()
        .block_on(async {
            let root = Root::new();
            let workers = Arc::new(Semaphore::new(1));
            let value = writer(&root, workers.clone());
            let bytes = b"worker-owned output after observer cancellation".to_vec();
            let expected = revision(&bytes);
            let (started, observed) = tokio::sync::oneshot::channel();
            let (release, held) = std::sync::mpsc::channel();
            let mut release = Release(Some(release));
            let blocker = tokio::task::spawn_blocking(move || {
                let _ = started.send(());
                held.recv().unwrap();
            });
            tokio::time::timeout(Duration::from_secs(2), observed)
                .await
                .unwrap()
                .unwrap();
            let mut waiting = Box::pin(value.write_blob(&expected, bytes.clone()));
            poll_fn(|cx| {
                assert!(matches!(waiting.as_mut().poll(cx), Poll::Pending));
                Poll::Ready(())
            })
            .await;
            assert_eq!(workers.available_permits(), 0);
            assert!(root.files().is_empty());
            drop(waiting);
            assert!(workers.clone().try_acquire_owned().is_err());
            release.send();
            blocker.await.unwrap();
            let permit = tokio::time::timeout(Duration::from_secs(2), workers.acquire_owned())
                .await
                .unwrap()
                .unwrap();
            drop(permit);
            let files = root.files();
            assert_eq!(files.len(), 1);
            assert_eq!(fs::read(&files[0]).unwrap(), bytes);
            assert_eq!(value.reader.read_blob(&expected).await.unwrap(), bytes);
        });
}
