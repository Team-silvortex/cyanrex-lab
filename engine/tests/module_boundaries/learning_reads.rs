//! F27/F29/F30: a failed read must not look like an empty classroom or reset progress.
use super::support::*;
use cyanrex_engine::services::learning_store::{LearningRunOutcome, LearningStore};
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn private_get(f: &Fixture, path: &str, cookie: &str) -> (StatusCode, Value) {
    let response = f
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn corrupt_read_recovers(path: &str, teacher_only: bool, count_pointer: &str) {
    let _guard = ENV_LOCK.lock().await;
    let f = Fixture::new();
    let student = f.cookie("network-student").await;
    let teacher = f.cookie("network-teacher").await;
    let file = f.root.join("learning.json");
    // Seed using a different store so the HTTP-facing store still has a cold snapshot.
    let seed = LearningStore::with_local_data_path(file.clone());
    let attempt = seed
        .record_run(
            "network-student",
            LearningRunOutcome {
                lab_id: "01-first-program",
                template_id: Some("xdp-pass"),
                source: SOURCE,
                run_success: false,
                stage: "compile",
                attach_expected: false,
                attach_verified: false,
            },
        )
        .await
        .unwrap();
    seed.save_teacher_feedback(
        "network-teacher",
        &cyanrex_engine::models::learning::SaveTeacherFeedbackRequest {
            username: "network-student".into(),
            attempt_id: attempt.id,
            comment: "Preserve reviewed feedback on read recovery".into(),
            expected_revision: 0,
        },
    )
    .await
    .unwrap();
    let original = std::fs::read(&file).unwrap();
    let invalid = b"private corrupted learning payload";
    std::fs::write(&file, invalid).unwrap();
    assert_eq!(
        f.request("GET", path, None, ORIGIN, Value::Null).await.0,
        StatusCode::UNAUTHORIZED
    );
    if teacher_only {
        assert_eq!(
            f.request("GET", path, Some(&student), ORIGIN, Value::Null)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }
    let cookie = if teacher_only { &teacher } else { &student };
    let (status, body) = private_get(&f, path, cookie).await;
    assert_eq!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "{path} must not claim empty success: {body}"
    );
    assert_eq!(body["ok"], false);
    assert!(!body.to_string().contains("private"));
    assert_eq!(std::fs::read(&file).unwrap(), invalid);
    assert!(f.calls.lock().unwrap().is_empty());

    std::fs::write(&file, &original).unwrap();
    let (status, recovered) = private_get(&f, path, cookie).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(recovered.pointer(count_pointer), Some(&json!(1)));
    assert_eq!(std::fs::read(&file).unwrap(), original);
    assert!(
        f.calls.lock().unwrap().is_empty(),
        "retrying a read must not execute a lab"
    );
}

#[tokio::test]
async fn history_read_failure_is_not_empty_success() {
    corrupt_read_recovers("/learning/attempts", false, "/0/teacher_feedback/revision").await;
}

#[tokio::test]
async fn progress_read_failure_is_not_zero_progress() {
    corrupt_read_recovers("/learning/labs", false, "/0/attempts").await;
}

#[tokio::test]
async fn teacher_overview_read_failure_is_not_an_empty_classroom() {
    corrupt_read_recovers("/learning/teacher/overview", true, "/active_students").await;
}

#[tokio::test]
async fn teacher_review_read_failure_is_not_empty_success() {
    corrupt_read_recovers(
        "/learning/teacher/attempts?username=network-student",
        true,
        "/attempts/0/teacher_feedback/revision",
    )
    .await;
}
