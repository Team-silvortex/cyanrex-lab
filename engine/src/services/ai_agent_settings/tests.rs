use super::*;
use std::time::Duration;
use tokio::sync::oneshot;

fn update() -> UpdateAiAgentSettingsRequest {
    UpdateAiAgentSettingsRequest {
        expected_revision: 0,
        default_profile_id: None,
        profiles: Vec::new(),
    }
}

struct Release(Option<std::sync::mpsc::Sender<()>>);
impl Drop for Release {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

#[test]
fn dispatched_write_retains_admission_after_caller_cancellation() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    let directory =
        std::env::temp_dir().join(format!("cyanrex-ai-cancel-{}", uuid::Uuid::new_v4()));
    let result = runtime.block_on(async {
        let store = AiAgentSettingsStore::new(directory.clone());
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let release = Release(Some(release_tx));
        let (started_tx, started_rx) = oneshot::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            let _ = started_tx.send(());
            release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        });
        started_rx.await.unwrap();
        let clone = store.clone();
        let caller = tokio::spawn(async move { clone.update(update()).await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while store.gate.try_lock().is_ok() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        // The queued blocking job owns admission, not the caller awaiting its JoinHandle.
        caller.abort();
        assert!(caller.await.unwrap_err().is_cancelled());
        let still_owned = store.gate.try_lock().is_err();
        let clone = store.clone();
        let subsequent = tokio::spawn(async move { clone.update(update()).await });
        tokio::task::yield_now().await;
        let still_waiting = !subsequent.is_finished();
        drop(release);
        blocker.await.unwrap();
        let result = subsequent.await.unwrap();
        (still_owned, still_waiting, result, store.get().await)
    });
    if directory.exists() {
        std::fs::remove_dir_all(&directory).unwrap();
    }
    assert!(result.0 && result.1);
    assert_eq!(result.2, Err(AiAgentSettingsError::Conflict));
    assert_eq!(result.3.unwrap().revision, 1);
}

#[tokio::test]
async fn queued_and_unpolled_updates_can_be_cancelled_without_creating_storage() {
    let directory =
        std::env::temp_dir().join(format!("cyanrex-ai-unpolled-{}", uuid::Uuid::new_v4()));
    let store = AiAgentSettingsStore::new(directory.clone());
    drop(store.update(update()));
    let guard = store.gate.clone().lock_owned().await;
    let clone = store.clone();
    let caller = tokio::spawn(async move { clone.update(update()).await });
    tokio::task::yield_now().await;
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    drop(guard);
    assert_eq!(store.get().await.unwrap().revision, 0);
    assert!(!directory.exists());
}
