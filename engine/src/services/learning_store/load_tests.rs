use super::*;
use std::{future::Future, task::Poll, time::Duration};

fn unloaded(fixture: &Fixture, bytes: &[u8]) -> LearningStore {
    let path = fixture.0.join("attempts.json");
    std::fs::write(&path, bytes).unwrap();
    LearningStore::with_local_data_path(path)
}

#[test]
fn cancelled_queued_load_keeps_admission_and_finishes_initialization() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap()
        .block_on(async {
            let fixture = Fixture::new();
            let bytes = serde_json::to_vec(&[attempt("alice", 0)]).unwrap();
            let store = unloaded(&fixture, &bytes);
            let (started_tx, started_rx) = tokio::sync::oneshot::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                started_tx.send(()).unwrap();
                let _ = release_rx.recv_timeout(Duration::from_secs(5));
            });
            started_rx.await.unwrap();
            let mut request = Box::pin(store.load_memory());
            std::future::poll_fn(|cx| {
                assert!(request.as_mut().poll(cx).is_pending());
                Poll::Ready(())
            })
            .await;
            drop(request);
            let admission_retained = store.persist_lock.try_lock().is_err();
            release_tx.send(()).unwrap();
            blocker.await.unwrap();
            drop(
                tokio::time::timeout(Duration::from_secs(5), store.persist_lock.lock())
                    .await
                    .unwrap(),
            );
            assert!(
                admission_retained,
                "cancelled queued initialization still owns admission"
            );
            assert!(store.memory_loaded.initialized());
            assert_eq!(store.in_memory.read().await.len(), 1);
            assert_eq!(std::fs::read(&store.data_path).unwrap(), bytes);
        });
}

#[tokio::test]
async fn cancelled_decoded_load_publishes_once_and_does_not_reread_changed_input() {
    let fixture = Fixture::new();
    let store = unloaded(
        &fixture,
        &serde_json::to_vec(&[attempt("alice", 0)]).unwrap(),
    );
    let reader = store.in_memory.read().await;
    let worker_store = store.clone();
    let request = tokio::spawn(async move { worker_store.load_memory().await });
    tokio::time::timeout(Duration::from_secs(5), async {
        // Tokio's fair RwLock rejects new readers once final publication is waiting for our reader.
        while store.in_memory.try_read().is_ok() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    let admission_retained = store.persist_lock.try_lock().is_err();
    std::fs::write(&store.data_path, b"invalid replacement in test fixture").unwrap();
    drop(reader);
    drop(
        tokio::time::timeout(Duration::from_secs(5), store.persist_lock.lock())
            .await
            .unwrap(),
    );
    assert!(
        admission_retained,
        "parsed initialization must survive caller cancellation"
    );
    assert!(store.memory_loaded.initialized());
    store.load_memory().await.unwrap();
    assert_eq!(store.attempts_for_user("alice").await.len(), 1);
}

#[tokio::test]
async fn only_not_found_is_initialized_as_empty_and_path_errors_remain_retryable() {
    let fixture = Fixture::new();
    let missing = LearningStore::with_local_data_path(fixture.0.join("missing/attempts.json"));
    missing.load_memory().await.unwrap();
    assert!(missing.memory_loaded.initialized());
    assert!(
        !missing.data_path.exists(),
        "loading must not create a file"
    );
    assert!(missing.in_memory.read().await.is_empty());

    let parent = fixture.0.join("not-a-directory");
    std::fs::write(&parent, b"owned test file").unwrap();
    let store = LearningStore::with_local_data_path(parent.join("attempts.json"));
    assert!(
        store.load_memory().await.is_err(),
        "ENOTDIR is not a missing store"
    );
    assert!(!store.memory_loaded.initialized());
    std::fs::remove_file(&parent).unwrap();
    std::fs::create_dir(&parent).unwrap();
    std::fs::write(
        &store.data_path,
        serde_json::to_vec(&[attempt("alice", 0)]).unwrap(),
    )
    .unwrap();
    store.load_memory().await.unwrap();
    assert_eq!(store.attempts_for_user("alice").await.len(), 1);

    let directory = LearningStore::with_local_data_path(fixture.0.clone());
    assert!(directory.load_memory().await.is_err());
    assert!(!directory.memory_loaded.initialized());
}

#[tokio::test]
async fn cold_decoder_matches_legacy_serde_and_never_publishes_partial_input() {
    let fixture = Fixture::new();
    let mut legacy = json!(attempt("alice", 0));
    legacy.as_object_mut().unwrap().remove("teacher_feedback");
    legacy["future_field"] = json!({"values": ["忽略未知字段", 1, true]});
    let valid = serde_json::to_vec(&vec![legacy]).unwrap();
    let row_sequence = json!([[
        "id",
        "alice",
        "01-first-program",
        null,
        "源码\n",
        "hash",
        false,
        "compile",
        false,
        false,
        false,
        [],
        null,
        "2026-09-08T00:00:00Z"
    ]]);
    let mut invalid_tail = valid.clone();
    invalid_tail.extend_from_slice(b" trailing");
    let mut truncated = valid.clone();
    truncated.pop();
    let mut partial = valid.clone();
    partial.pop();
    partial.extend_from_slice(b",null]");
    let mut invalid_unknown = b"[{\"unknown\":\"\xff\",".to_vec();
    invalid_unknown.extend_from_slice(&valid[2..]);
    for bytes in [
        valid,
        b" \r\n[]\t".to_vec(),
        serde_json::to_vec(&row_sequence).unwrap(),
        invalid_tail,
        truncated,
        partial,
        invalid_unknown,
        b"".to_vec(),
        b"null".to_vec(),
        b"{}".to_vec(),
        b"[{}]".to_vec(),
        b"[\xff]".to_vec(),
        b"[1,2]".to_vec(),
    ] {
        // The previous loader validated the entire file as UTF-8 before parsing, including ignored fields.
        let expected = std::str::from_utf8(&bytes)
            .map_err(|error| error.to_string())
            .and_then(|text| {
                serde_json::from_str::<Vec<LabAttempt>>(text).map_err(|error| error.to_string())
            });
        let store = unloaded(&fixture, &bytes);
        let result = store.load_memory().await;
        assert_eq!(result.is_ok(), expected.is_ok());
        assert_eq!(store.memory_loaded.initialized(), expected.is_ok());
        let snapshot = store.in_memory.read().await.clone();
        if let Ok(rows) = expected {
            let actual: Vec<_> = snapshot.iter().map(Arc::as_ref).collect();
            assert_eq!(json!(actual), json!(rows));
        } else {
            assert!(
                snapshot.is_empty(),
                "no prefix of corrupt input may be published"
            );
            std::fs::write(&store.data_path, b"[]").unwrap();
            store.load_memory().await.unwrap();
            assert!(
                store.memory_loaded.initialized(),
                "decode failure must remain retryable"
            );
        }
        assert!(!store.data_path.with_extension("json.tmp").exists());
    }
}

#[tokio::test]
async fn concurrent_cold_readers_share_one_published_snapshot() {
    let fixture = Fixture::new();
    let rows: Vec<_> = (0..100).map(|n| attempt("alice", n)).collect();
    let store = unloaded(&fixture, &serde_json::to_vec(&rows).unwrap());
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let store = store.clone();
        tasks.push(tokio::spawn(async move {
            store.load_memory().await.unwrap();
            let snapshot = store.in_memory.read().await.clone();
            snapshot
        }));
    }
    let snapshot = tasks.pop().unwrap().await.unwrap();
    for task in tasks {
        assert!(Arc::ptr_eq(&snapshot, &task.await.unwrap()));
    }
    std::fs::remove_file(&store.data_path).unwrap();
    store.load_memory().await.unwrap();
    assert!(Arc::ptr_eq(&snapshot, &*store.in_memory.read().await));
}

#[tokio::test]
async fn concurrent_cold_appends_and_feedback_preserve_initial_and_new_rows() {
    let fixture = Fixture::new();
    let row = attempt("alice", 0);
    let update = SaveTeacherFeedbackRequest {
        username: "alice".into(),
        attempt_id: row.id.clone(),
        comment: "首读并发".into(),
        expected_revision: 0,
    };
    let store = unloaded(&fixture, &serde_json::to_vec(&[row]).unwrap());
    let mut writers = Vec::new();
    for _ in 0..8 {
        let store = store.clone();
        writers.push(tokio::spawn(async move {
            store.record_run("bob", outcome()).await.unwrap()
        }));
    }
    assert_eq!(
        store
            .save_teacher_feedback("teacher", &update)
            .await
            .unwrap()
            .revision,
        1
    );
    for writer in writers {
        writer.await.unwrap();
    }
    let loaded = LearningStore::with_local_data_path(store.data_path.clone());
    assert_eq!(loaded.attempts_for_user("bob").await.len(), 8);
    let alice = loaded.attempts_for_user("alice").await;
    assert_eq!(alice.len(), 1);
    assert_eq!(alice[0].teacher_feedback.as_ref().unwrap().revision, 1);
}
