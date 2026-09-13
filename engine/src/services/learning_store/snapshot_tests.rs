use super::*;
use crate::models::learning::SaveTeacherFeedbackRequest;
use serde_json::json;

#[path = "cancellation_tests.rs"]
mod cancellation_tests;
#[path = "load_tests.rs"]
mod load_tests;
#[path = "read_failure_tests.rs"]
mod read_failure_tests;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("cyanrex-learning-snapshot-{}", Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    async fn store(&self, attempts: &[LabAttempt]) -> LearningStore {
        let path = self.0.join("attempts.json");
        tokio::fs::write(&path, serde_json::to_vec_pretty(attempts).unwrap())
            .await
            .unwrap();
        let store = LearningStore::with_local_data_path(path);
        store.load_memory().await.unwrap();
        store
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn attempt(owner: &str, index: usize) -> LabAttempt {
    LabAttempt {
        id: Uuid::from_u128(index as u128 + 1).to_string(),
        username: owner.into(),
        lab_id: "01-first-program".into(),
        template_id: Some("xdp-pass".into()),
        source: format!("// {index} 中文\n{}", "source ".repeat(1000)),
        source_sha256: format!("hash-{index}"),
        run_success: false,
        stage: "compile".into(),
        attach_expected: false,
        attach_verified: false,
        completed: false,
        feedback: vec![format!("feedback-{index}")],
        teacher_feedback: None,
        created_at: chrono::DateTime::from_timestamp(1_700_000_000 + index as i64, 0).unwrap(),
    }
}

fn outcome() -> LearningRunOutcome<'static> {
    LearningRunOutcome {
        lab_id: "01-first-program",
        template_id: Some("xdp-pass"),
        source: "invalid source",
        run_success: false,
        stage: "compile",
        attach_expected: false,
        attach_verified: false,
    }
}

#[tokio::test]
async fn snapshot_reads_share_the_collection_and_source_allocations() {
    let fixture = Fixture::new();
    let store = fixture
        .store(&[attempt("alice", 0), attempt("bob", 1)])
        .await;
    let first = store.in_memory.read().await.clone();
    let second = store.in_memory.read().await.clone();
    assert_eq!(
        first.as_ptr(),
        second.as_ptr(),
        "taking a read snapshot must not copy rows"
    );
    assert_eq!(first[0].source.as_ptr(), second[0].source.as_ptr());
}

#[tokio::test]
async fn appends_share_existing_sources_and_do_not_mutate_old_snapshots() {
    let fixture = Fixture::new();
    let store = fixture
        .store(&[attempt("alice", 0), attempt("bob", 1)])
        .await;
    let before = store.in_memory.read().await.clone();
    let recorded = store.record_run("alice", outcome()).await.unwrap();
    let after = store.in_memory.read().await.clone();
    assert_eq!(before.len(), 2);
    assert_eq!(after.len(), 3);
    assert_eq!(
        before[0].source.as_ptr(),
        after[0].source.as_ptr(),
        "append must not clone old source"
    );
    assert_eq!(after[2].id, recorded.id);
    let disk: Vec<LabAttempt> =
        serde_json::from_slice(&tokio::fs::read(&store.data_path).await.unwrap()).unwrap();
    assert_eq!(disk.len(), 3);
    assert_eq!(
        serde_json::to_value(&disk[2]).unwrap(),
        serde_json::to_value(recorded).unwrap()
    );
}

#[tokio::test]
async fn feedback_copies_only_the_changed_record_and_preserves_old_snapshots() {
    let fixture = Fixture::new();
    let store = fixture
        .store(&[attempt("alice", 0), attempt("bob", 1)])
        .await;
    let before = store.in_memory.read().await.clone();
    let feedback = store
        .save_teacher_feedback(
            "teacher",
            &SaveTeacherFeedbackRequest {
                username: "alice".into(),
                attempt_id: before[0].id.clone(),
                comment: "检查边界".into(),
                expected_revision: 0,
            },
        )
        .await
        .unwrap();
    let after = store.in_memory.read().await.clone();
    assert!(before[0].teacher_feedback.is_none());
    assert_eq!(after[0].teacher_feedback.as_ref().unwrap().revision, 1);
    assert_eq!(
        before[1].source.as_ptr(),
        after[1].source.as_ptr(),
        "feedback must not copy another record"
    );
    let reloaded = LearningStore::with_local_data_path(store.data_path.clone());
    let attempts = reloaded.attempts_for_user("alice").await.unwrap();
    assert_eq!(attempts[0].source, before[0].source);
    assert_eq!(
        serde_json::to_value(&attempts[0].teacher_feedback).unwrap(),
        json!(feedback)
    );
}

#[tokio::test]
async fn failed_writes_do_not_publish_a_new_snapshot_and_remain_retryable() {
    let fixture = Fixture::new();
    let store = fixture.store(&[attempt("alice", 0)]).await;
    let before = store.in_memory.read().await.clone();
    let original = tokio::fs::read(&store.data_path).await.unwrap();
    let backup = fixture.0.join("original.json");
    tokio::fs::rename(&store.data_path, &backup).await.unwrap();
    tokio::fs::create_dir(&store.data_path).await.unwrap();
    assert!(store.record_run("alice", outcome()).await.is_err());
    assert!(matches!(
        store
            .save_teacher_feedback(
                "teacher",
                &SaveTeacherFeedbackRequest {
                    username: "alice".into(),
                    attempt_id: before[0].id.clone(),
                    comment: "must not leak".into(),
                    expected_revision: 0,
                }
            )
            .await,
        Err(LearningFeedbackError::Storage(_))
    ));
    let after = store.in_memory.read().await.clone();
    assert_eq!(
        before.as_ptr(),
        after.as_ptr(),
        "failed persistence must not replace the snapshot"
    );
    assert!(after[0].teacher_feedback.is_none());
    assert_eq!(tokio::fs::read(&backup).await.unwrap(), original);
    tokio::fs::remove_dir(&store.data_path).await.unwrap();
    tokio::fs::rename(backup, &store.data_path).await.unwrap();
    store.record_run("alice", outcome()).await.unwrap();
    assert_eq!(store.attempts_for_user("alice").await.unwrap().len(), 2);
}

#[tokio::test]
async fn concurrent_appends_and_feedback_keep_every_success_after_reload() {
    let fixture = Fixture::new();
    let store = fixture.store(&[attempt("alice", 0)]).await;
    let request = SaveTeacherFeedbackRequest {
        username: "alice".into(),
        attempt_id: attempt("alice", 0).id,
        comment: "one winner".into(),
        expected_revision: 0,
    };
    let (first, second, added_alice, added_bob) = tokio::join!(
        store.save_teacher_feedback("teacher", &request),
        store.save_teacher_feedback("admin", &request),
        store.record_run("alice", outcome()),
        store.record_run("bob", outcome()),
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert!(
        matches!(first, Err(LearningFeedbackError::Conflict))
            || matches!(second, Err(LearningFeedbackError::Conflict))
    );
    assert!(added_alice.is_ok() && added_bob.is_ok());
    let loaded = LearningStore::with_local_data_path(store.data_path.clone());
    let alice = loaded.attempts_for_user("alice").await.unwrap();
    assert_eq!(alice.len(), 2);
    assert_eq!(
        alice
            .iter()
            .filter(|row| row.teacher_feedback.is_some())
            .count(),
        1
    );
    assert_eq!(loaded.attempts_for_user("bob").await.unwrap().len(), 1);
}

#[tokio::test]
async fn recent_pages_keep_timestamp_order_and_stable_ties_without_leaking_owners() {
    let fixture = Fixture::new();
    let mut input: Vec<_> = (0..80)
        .map(|index| attempt(if index % 3 == 0 { "bob" } else { "alice" }, index))
        .collect();
    for row in &mut input {
        row.created_at -= chrono::Duration::seconds(row.created_at.timestamp() % 7);
    }
    input.rotate_left(19);
    let store = fixture.store(&input).await;
    let mut expected: Vec<_> = input
        .into_iter()
        .filter(|row| row.username == "alice")
        .collect();
    expected.sort_by_key(|row| std::cmp::Reverse(row.created_at));
    for limit in [0, 1, 20, 50, 100] {
        let page = store
            .recent_attempts_for_user("alice", limit)
            .await
            .unwrap();
        let length = expected.len().min(limit.clamp(1, 50));
        assert_eq!(
            serde_json::to_value(&page).unwrap(),
            serde_json::to_value(&expected[..length]).unwrap()
        );
    }
    assert_eq!(
        serde_json::to_value(store.attempts_for_user("alice").await.unwrap()).unwrap(),
        json!(expected)
    );
    assert!(store.attempts_for_user("a/lice").await.unwrap().is_empty());
    assert!(store
        .recent_attempts_for_user("missing", 20)
        .await
        .unwrap()
        .is_empty());
    let mut page = store.recent_attempts_for_user("alice", 1).await.unwrap();
    page[0].source.clear();
    assert!(
        !store.recent_attempts_for_user("alice", 1).await.unwrap()[0]
            .source
            .is_empty()
    );
}

#[tokio::test]
async fn legacy_json_array_still_loads_and_writes_identical_pretty_encoding() {
    let fixture = Fixture::new();
    let path = fixture.0.join("attempts.json");
    let mut legacy = json!([attempt("alice", 0)]);
    legacy[0]
        .as_object_mut()
        .unwrap()
        .remove("teacher_feedback");
    tokio::fs::write(&path, serde_json::to_vec(&legacy).unwrap())
        .await
        .unwrap();
    let store = LearningStore::with_local_data_path(path.clone());
    assert!(store.attempts_for_user("alice").await.unwrap()[0]
        .teacher_feedback
        .is_none());
    store.record_run("alice", outcome()).await.unwrap();
    let bytes = tokio::fs::read(path).await.unwrap();
    let rows: Vec<LabAttempt> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        bytes,
        serde_json::to_vec_pretty(&rows).unwrap(),
        "no format/envelope migration"
    );
    assert_eq!(rows[0].source, legacy[0]["source"].as_str().unwrap());
}

#[tokio::test]
async fn failed_final_rename_does_not_change_the_in_memory_snapshot() {
    let fixture = Fixture::new();
    let store = fixture.store(&[attempt("alice", 0)]).await;
    let before = store.in_memory.read().await.clone();
    let backup = fixture.0.join("original.json");
    tokio::fs::rename(&store.data_path, &backup).await.unwrap();
    tokio::fs::create_dir(&store.data_path).await.unwrap();
    assert!(store
        .record_run("alice", outcome())
        .await
        .unwrap_err()
        .contains("finalize"));
    let after = store.in_memory.read().await.clone();
    assert_eq!(before.as_ptr(), after.as_ptr());
    tokio::fs::remove_dir(&store.data_path).await.unwrap();
    tokio::fs::rename(backup, &store.data_path).await.unwrap();
    store.record_run("alice", outcome()).await.unwrap();
    assert_eq!(
        LearningStore::with_local_data_path(store.data_path.clone())
            .attempts_for_user("alice")
            .await
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn progress_and_overview_stay_fresh_without_exposing_or_copying_source() {
    let fixture = Fixture::new();
    let mut first = attempt("alice", 0);
    first.completed = true;
    let completed_at = first.created_at;
    let store = fixture
        .store(&[first, attempt("alice", 1), attempt("bob", 2)])
        .await;
    let progress = store.progress_for_user("alice").await.unwrap();
    assert_eq!(
        progress[0].status,
        crate::models::learning::LabProgressStatus::Completed
    );
    assert_eq!(progress[0].attempts, 2);
    assert_eq!(progress[0].completed_at, Some(completed_at));
    assert_eq!(progress[0].latest_feedback, vec!["feedback-1"]);
    let recorded = store.record_run("alice", outcome()).await.unwrap();
    let progress = store.progress_for_user("alice").await.unwrap();
    assert_eq!(progress[0].attempts, 3);
    assert_eq!(progress[0].completed_at, Some(completed_at));
    assert_eq!(progress[0].latest_feedback, recorded.feedback);
    let overview = store.teacher_overview().await.unwrap();
    assert_eq!(overview.active_students, 2);
    assert_eq!(overview.students[0].username, "alice");
    assert_eq!(overview.students[0].total_attempts, 3);
    assert_eq!(overview.students[0].completed_labs, 1);
    assert_eq!(overview.students[1].total_attempts, 1);
    assert!(!serde_json::to_string(&overview)
        .unwrap()
        .contains("\"source\":"));
    for owner in ["missing", "a/lice"] {
        assert!(store
            .progress_for_user(owner)
            .await
            .unwrap()
            .iter()
            .all(|lab| lab.attempts == 0));
    }
}
