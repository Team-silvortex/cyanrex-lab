use super::*;
use std::{
    fs::{self, File, Permissions},
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("cyanrex-artifact-reader-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: "00000000-0000-4000-8000-000000000011".parse().unwrap(),
        workspace_id: "00000000-0000-4000-8000-000000000012".parse().unwrap(),
    }
}
fn reader(root: &Root, workers: Arc<Semaphore>) -> ArtifactContentReader {
    let original = open_private_artifact_root(&root.0).unwrap();
    ArtifactContentReader::from_retained_root(scope(), &root.0, &original, workers).unwrap()
}

#[tokio::test]
async fn retained_reader_copies_original_root_without_inspecting_contents_or_opening_sql() {
    let root = Root::new();
    // A deliberately invalid private content file cannot matter to a metadata-only constructor.
    File::create(root.0.join("not-an-artifact")).unwrap();
    let original = open_private_artifact_root(&root.0).unwrap();
    let value = ArtifactContentReader::from_retained_root(
        scope(),
        &root.0,
        &original,
        Arc::new(Semaphore::new(1)),
    )
    .unwrap();
    drop(original);
    value.verify_root().await.unwrap();
    let owner = PrincipalRef {
        authority_id: scope().authority_id,
        principal_id: "00000000-0000-4000-8000-000000000013".parse().unwrap(),
    };
    assert_eq!(value.check_scope(scope(), owner), Ok(()));
    let foreign = WorkspaceRef {
        workspace_id: "00000000-0000-4000-8000-000000000014".parse().unwrap(),
        ..scope()
    };
    assert_eq!(
        value.check_scope(foreign, owner),
        Err(ArtifactStoreError::ScopeMismatch)
    );
}

#[tokio::test]
async fn retained_reader_refuses_replaced_roots_and_accepts_only_original_restoration() {
    let root = Root::new();
    let original = open_private_artifact_root(&root.0).unwrap();
    let value = reader(&root, Arc::new(Semaphore::new(1)));
    let hidden = root.0.with_extension("original");
    fs::rename(&root.0, &hidden).unwrap();
    fs::create_dir(&root.0).unwrap();
    fs::set_permissions(&root.0, Permissions::from_mode(0o700)).unwrap();
    assert!(matches!(
        ArtifactContentReader::from_retained_root(
            scope(),
            &root.0,
            &original,
            Arc::new(Semaphore::new(1)),
        ),
        Err(ArtifactStoreError::BlobUnavailable)
    ));
    assert_eq!(
        value.verify_root().await,
        Err(ArtifactStoreError::BlobUnavailable)
    );
    fs::remove_dir(&root.0).unwrap();
    fs::rename(&hidden, &root.0).unwrap();
    value.verify_root().await.unwrap();
}

#[tokio::test]
async fn host_reader_has_no_unbounded_fallback_when_worker_admission_is_full() {
    let root = Root::new();
    let value = reader(&root, Arc::new(Semaphore::new(0)));
    let ran = Arc::new(AtomicBool::new(false));
    let observed = ran.clone();
    assert_eq!(
        value
            .blocking(move |_| {
                observed.store(true, Ordering::SeqCst);
                Ok(())
            })
            .await,
        Err(ArtifactStoreError::StorageUnavailable)
    );
    assert!(!ran.load(Ordering::SeqCst));
}

#[tokio::test]
async fn cancelled_reader_keeps_worker_permit_until_the_actual_blocking_job_finishes() {
    let root = Root::new();
    let workers = Arc::new(Semaphore::new(1));
    let value = reader(&root, workers.clone());
    let (started, observed) = tokio::sync::oneshot::channel();
    let (release, stopped) = std::sync::mpsc::channel();
    let task = tokio::spawn(async move {
        value
            .blocking(move |_| {
                let _ = started.send(());
                stopped
                    .recv()
                    .map_err(|_| ArtifactStoreError::BlobUnavailable)
            })
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), observed)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workers.available_permits(), 0);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(workers.clone().try_acquire_owned().is_err());
    release.send(()).unwrap();
    let permit = tokio::time::timeout(Duration::from_secs(2), workers.acquire_owned())
        .await
        .unwrap()
        .unwrap();
    drop(permit);
}
