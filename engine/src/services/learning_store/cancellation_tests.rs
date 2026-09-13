use super::*;
use std::{future::Future, task::Poll, time::Duration};

async fn changed_file(path: &std::path::Path, original: &[u8]) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if tokio::fs::read(path).await.unwrap() != original {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .expect("writer must finish renaming while memory publication is blocked");
}

async fn assert_disk_matches_memory(store: &LearningStore) {
    let snapshot = store.in_memory.read().await.clone();
    let rows: Vec<_> = snapshot.iter().map(Arc::as_ref).collect();
    let disk = tokio::fs::read(&store.data_path).await.unwrap();
    assert_eq!(disk, serde_json::to_vec_pretty(&rows).unwrap());
}

#[tokio::test]
async fn cancelled_append_finishes_publication_before_releasing_the_writer_lock() {
    let fixture = Fixture::new();
    let store = fixture.store(&[attempt("alice", 0)]).await;
    let original = tokio::fs::read(&store.data_path).await.unwrap();
    // Permit the writer's snapshot read, but deterministically stop its final publication.
    let reader = store.in_memory.read().await;
    let writer_store = store.clone();
    let request = tokio::spawn(async move { writer_store.record_run("alice", outcome()).await });
    changed_file(&store.data_path, &original).await;
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    let lock_retained = store.persist_lock.try_lock().is_err();
    drop(reader);
    drop(
        tokio::time::timeout(Duration::from_secs(5), store.persist_lock.lock())
            .await
            .unwrap(),
    );
    assert!(
        lock_retained,
        "cancellation must not release the commit's lock"
    );
    assert_disk_matches_memory(&store).await;
    assert_eq!(store.attempts_for_user("alice").await.unwrap().len(), 2);
    store.record_run("bob", outcome()).await.unwrap();
    let loaded = LearningStore::with_local_data_path(store.data_path.clone());
    assert_eq!(loaded.attempts_for_user("alice").await.unwrap().len(), 2);
    assert_eq!(loaded.attempts_for_user("bob").await.unwrap().len(), 1);
}

#[tokio::test]
async fn cancelled_feedback_keeps_the_saved_revision_and_rejects_a_stale_retry() {
    let fixture = Fixture::new();
    let row = attempt("alice", 0);
    let update = SaveTeacherFeedbackRequest {
        username: "alice".into(),
        attempt_id: row.id.clone(),
        comment: "保存后取消".into(),
        expected_revision: 0,
    };
    let store = fixture.store(&[row]).await;
    let original = tokio::fs::read(&store.data_path).await.unwrap();
    let reader = store.in_memory.read().await;
    let writer_store = store.clone();
    let request_update = SaveTeacherFeedbackRequest {
        username: update.username.clone(),
        attempt_id: update.attempt_id.clone(),
        comment: update.comment.clone(),
        expected_revision: update.expected_revision,
    };
    let request = tokio::spawn(async move {
        writer_store
            .save_teacher_feedback("teacher", &request_update)
            .await
    });
    changed_file(&store.data_path, &original).await;
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    let lock_retained = store.persist_lock.try_lock().is_err();
    drop(reader);
    drop(
        tokio::time::timeout(Duration::from_secs(5), store.persist_lock.lock())
            .await
            .unwrap(),
    );
    assert!(
        lock_retained,
        "feedback publication still owns the writer lock"
    );
    assert_disk_matches_memory(&store).await;
    assert!(matches!(
        store.save_teacher_feedback("teacher", &update).await,
        Err(LearningFeedbackError::Conflict)
    ));
    let next = SaveTeacherFeedbackRequest {
        expected_revision: 1,
        ..update
    };
    assert_eq!(
        store
            .save_teacher_feedback("teacher", &next)
            .await
            .unwrap()
            .revision,
        2
    );
    assert_disk_matches_memory(&store).await;
}

#[tokio::test]
async fn cancelling_queued_writers_does_not_change_disk_or_memory() {
    let fixture = Fixture::new();
    let row = attempt("alice", 0);
    let update = SaveTeacherFeedbackRequest {
        username: "alice".into(),
        attempt_id: row.id.clone(),
        comment: "尚未开始".into(),
        expected_revision: 0,
    };
    let store = fixture.store(&[row]).await;
    let original = tokio::fs::read(&store.data_path).await.unwrap();
    let writer_lock = store.persist_lock.lock().await;
    let mut append = Box::pin(store.record_run("alice", outcome()));
    let mut feedback = Box::pin(store.save_teacher_feedback("teacher", &update));
    std::future::poll_fn(|cx| {
        assert!(append.as_mut().poll(cx).is_pending());
        assert!(feedback.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(append);
    drop(feedback);
    drop(writer_lock);
    assert_eq!(tokio::fs::read(&store.data_path).await.unwrap(), original);
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
    assert_disk_matches_memory(&store).await;
    assert_eq!(
        store.record_run("bob", outcome()).await.unwrap().username,
        "bob"
    );
    assert_eq!(store.attempts_for_user("alice").await.unwrap().len(), 1);
}

async fn cancelled_blocking_queue_commit(fail_write: bool) {
    let fixture = Fixture::new();
    let store = fixture.store(&[attempt("alice", 0)]).await;
    let original = std::fs::read(&store.data_path).unwrap();
    let backup = fixture.0.join("original.json");
    if fail_write {
        std::fs::rename(&store.data_path, &backup).unwrap();
        std::fs::create_dir(&store.data_path).unwrap();
    }
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let blocker = tokio::task::spawn_blocking(move || {
        started_tx.send(()).unwrap();
        // Bound cleanup even if a test assertion fails while this occupies the only blocking thread.
        let _ = release_rx.recv_timeout(Duration::from_secs(5));
    });
    started_rx.await.unwrap();
    let mut request = Box::pin(store.record_run("alice", outcome()));
    std::future::poll_fn(|cx| {
        assert!(request.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(request);
    let admission_retained = store.persist_lock.try_lock().is_err();
    let before_start = std::fs::read(if fail_write {
        &backup
    } else {
        &store.data_path
    })
    .unwrap();
    release_tx.send(()).unwrap();
    blocker.await.unwrap();
    drop(
        tokio::time::timeout(Duration::from_secs(5), store.persist_lock.lock())
            .await
            .unwrap(),
    );
    assert!(
        admission_retained,
        "a detached queued worker still owns admission"
    );
    assert_eq!(
        before_start, original,
        "queued work has not touched the file"
    );
    if fail_write {
        assert_eq!(std::fs::read(&backup).unwrap(), original);
        assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 2);
        std::fs::remove_dir(&store.data_path).unwrap();
        std::fs::rename(&backup, &store.data_path).unwrap();
    }
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
    assert_disk_matches_memory(&store).await;
    assert_eq!(
        store.attempts_for_user("alice").await.unwrap().len(),
        if fail_write { 1 } else { 2 }
    );
    store.record_run("bob", outcome()).await.unwrap();
    assert_disk_matches_memory(&store).await;
}

#[test]
fn cancelled_request_does_not_drop_an_admitted_but_not_started_worker() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap()
        .block_on(cancelled_blocking_queue_commit(false));
}

#[test]
fn cancelled_failing_worker_preserves_old_data_and_releases_admission() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap()
        .block_on(cancelled_blocking_queue_commit(true));
}
