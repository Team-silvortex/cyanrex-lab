use super::*;
use std::fs;

#[tokio::test]
async fn cold_snapshot_with_another_owner_is_not_exposed_or_overwritten() {
    let f = Fixture::new();
    f.store
        .save_for_user("bob", "private", "source")
        .await
        .unwrap();
    let bytes = fs::read(f.store.data_dir.join("bob.json")).unwrap();
    let path = f.store.data_dir.join("alice.json");
    fs::write(&path, &bytes).unwrap();
    assert!(f.store.list_for_user("alice").await.is_empty());
    assert!(f
        .store
        .save_for_user("alice", "new", "source")
        .await
        .is_err());
    assert_eq!(fs::read(path).unwrap(), bytes);
    assert!(f.store.in_memory.read().await.get("alice").is_none());
    assert_eq!(f.store.list_for_user("bob").await.len(), 1);
}

struct Fixture {
    root: PathBuf,
    store: ScriptStore,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("cyanrex-script-atomic-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let store = ScriptStore {
            data_dir: root.join("scripts"),
            db_pool: None,
            ..ScriptStore::default()
        };
        Self { root, store }
    }
    fn reload(&self) -> ScriptStore {
        ScriptStore {
            data_dir: self.store.data_dir.clone(),
            db_pool: None,
            ..ScriptStore::default()
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[tokio::test]
async fn concurrent_local_commits_survive_reload_without_lost_updates() {
    let f = Fixture::new();
    let first = f
        .store
        .save_for_user("alice", "first", "original")
        .await
        .unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    for index in 0..20 {
        let store = f.store.clone();
        tasks.spawn(async move {
            store
                .save_for_user("alice", &format!("script {index}"), "source")
                .await
                .unwrap();
        });
    }
    let store = f.store.clone();
    tasks.spawn(async move {
        store.delete_for_user("alice", &first.id).await.unwrap();
    });
    while let Some(result) = tasks.join_next().await {
        result.unwrap();
    }
    let memory = f.store.list_for_user("alice").await;
    let disk = f.reload().list_for_user("alice").await;
    assert_eq!(memory.len(), 20);
    assert_eq!(
        serde_json::to_value(&memory).unwrap(),
        serde_json::to_value(&disk).unwrap()
    );
    assert_eq!(fs::read_dir(&f.store.data_dir).unwrap().count(), 1);
}

#[tokio::test]
async fn cancelled_commit_retains_admission_until_disk_and_cache_are_published() {
    let f = Fixture::new();
    f.store
        .save_for_user("alice", "before", "before")
        .await
        .unwrap();
    // Allow the worker's snapshot read, but stop memory publication after the atomic rename.
    let reader = f.store.in_memory.read().await;
    let store = f.store.clone();
    let task = tokio::spawn(async move { store.save_for_user("alice", "after", "after").await });
    let path = f.store.data_dir.join("alice.json");
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let records: Vec<UserScript> =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            if records.len() == 2 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("worker must reach disk commit before memory publication");
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(reader["alice"].len(), 1);
    assert!(f.store.local_writer.try_lock().is_err());
    drop(reader);
    let _finished = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        f.store.local_writer.lock(),
    )
    .await
    .unwrap();
    assert_eq!(f.store.list_for_user("alice").await.len(), 2);
    assert_eq!(f.reload().list_for_user("alice").await.len(), 2);
}

#[tokio::test]
async fn cancelling_a_queued_commit_does_not_write_or_publish() {
    let f = Fixture::new();
    f.store
        .save_for_user("alice", "before", "before")
        .await
        .unwrap();
    let before = fs::read(f.store.data_dir.join("alice.json")).unwrap();
    let admission = f.store.local_writer.lock().await;
    let store = f.store.clone();
    let task =
        tokio::spawn(async move { store.save_for_user("alice", "cancelled", "cancelled").await });
    tokio::task::yield_now().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    drop(admission);
    assert_eq!(
        fs::read(f.store.data_dir.join("alice.json")).unwrap(),
        before
    );
    assert_eq!(f.store.list_for_user("alice").await.len(), 1);
}

#[tokio::test]
async fn malformed_cold_snapshot_is_preserved_and_can_be_retried_after_repair() {
    let f = Fixture::new();
    fs::create_dir(&f.store.data_dir).unwrap();
    let path = f.store.data_dir.join("alice.json");
    fs::write(&path, "[truncated").unwrap();
    assert!(f
        .store
        .save_for_user("alice", "fail", "source")
        .await
        .is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "[truncated");
    assert!(f.store.in_memory.read().await.get("alice").is_none());
    fs::write(&path, "[]").unwrap();
    f.store
        .save_for_user("alice", "repaired", "source")
        .await
        .unwrap();
    assert_eq!(f.reload().list_for_user("alice").await.len(), 1);
}

#[tokio::test]
async fn failed_rename_keeps_old_snapshot_and_removes_only_its_temporary_file() {
    let f = Fixture::new();
    let record = f
        .store
        .save_for_user("alice", "keep", "source")
        .await
        .unwrap();
    let path = f.store.data_dir.join("alice.json");
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(f.store.delete_for_user("alice", &record.id).await.is_err());
    assert_eq!(f.store.list_for_user("alice").await.len(), 1);
    assert_eq!(fs::read_dir(&f.store.data_dir).unwrap().count(), 1);
    fs::remove_dir(&path).unwrap();
    f.store.delete_for_user("alice", &record.id).await.unwrap();
    assert!(f.reload().list_for_user("alice").await.is_empty());
}

#[tokio::test]
async fn persisted_scripts_have_private_file_permissions() {
    let f = Fixture::new();
    f.store
        .save_for_user("alice", "private", "source")
        .await
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(f.store.data_dir.join("alice.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&f.store.data_dir)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
}
