use super::support::*;
use cyanrex_engine::services::learning_store::LearningStore;

#[tokio::test]
async fn saved_source_flows_through_runner_events_feedback_and_owner_bound_resume() {
    let _guard = ENV_LOCK.lock().await;
    let f = Fixture::new();
    let student = f.cookie("network-student").await;
    let other = f.cookie("network-other").await;
    let teacher = f.cookie("network-teacher").await;
    let (status, saved) = f
        .request(
            "POST",
            "/scripts/save",
            Some(&student),
            ORIGIN,
            json!({"title":"boundary submission", "script":SOURCE}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        f.request(
            "POST",
            "/modules/c-headers/select",
            Some(&teacher),
            ORIGIN,
            json!({"id":"bpf_helpers", "selected":true})
        )
        .await
        .0,
        StatusCode::OK
    );
    let run = json!({"code": saved["record"]["script"], "lab_id":"01-first-program", "template_id":"xdp-pass", "enable_kernel_stream":false});
    assert_eq!(
        f.request(
            "POST",
            "/ebpf/run",
            Some(&student),
            "https://untrusted.example",
            run.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert!(f.calls.lock().unwrap().is_empty());
    let (_, result) = f
        .request("POST", "/ebpf/run", Some(&student), ORIGIN, run)
        .await;
    assert_eq!(result["success"], false);
    assert_eq!(result["stage"], "compile");
    assert_eq!(
        *f.calls.lock().unwrap(),
        vec![(
            "network-student".into(),
            SOURCE.into(),
            vec!["bpf_helpers".into()]
        )]
    );
    assert_eq!(
        f.get("/runner/status", &student).await["active_for_current_user"],
        0
    );
    let attempts = f.get("/learning/attempts", &student).await;
    assert_eq!(attempts.as_array().unwrap().len(), 1);
    assert_eq!(attempts[0]["source"], SOURCE);
    assert_eq!(attempts[0]["completed"], false);
    let id = attempts[0]["id"].as_str().unwrap();
    let events = f.get("/events", &student).await;
    assert!(events
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["event_type"] == "ebpf.run_started"));
    assert!(events
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["event_type"] == "learning.lab_attempted"
            && row["payload"]["attempt_id"] == id));
    assert!(f
        .get("/events", &other)
        .await
        .as_array()
        .unwrap()
        .is_empty());
    assert!(f
        .get("/learning/attempts", &other)
        .await
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        f.request(
            "GET",
            &format!("/learning/attempt?attempt_id={id}"),
            Some(&other),
            ORIGIN,
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let feedback = json!({"username":"network-student", "attempt_id":id, "comment":"check the guard", "expected_revision":0});
    assert_eq!(
        f.request(
            "POST",
            "/learning/teacher/feedback",
            Some(&student),
            ORIGIN,
            feedback.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, review) = f
        .request(
            "POST",
            "/learning/teacher/feedback",
            Some(&teacher),
            ORIGIN,
            feedback,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(review["reviewer"], "network-teacher");
    let resumed = f
        .get(&format!("/learning/attempt?attempt_id={id}"), &student)
        .await;
    assert_eq!(resumed["source"], SOURCE);
    assert_eq!(resumed["teacher_feedback"]["comment"], "check the guard");
    assert_eq!(
        f.calls.lock().unwrap().len(),
        1,
        "reading a historical submission must never execute it"
    );
    let reloaded = LearningStore::with_local_data_path(f.root.join("learning.json"));
    let preserved = reloaded.attempts_for_user("network-student").await;
    assert_eq!(
        preserved[0].teacher_feedback.as_ref().unwrap().comment,
        "check the guard"
    );
}
