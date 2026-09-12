use super::support::*;
use cyanrex_engine::models::event::{Event, EventCategory, EventSeverity};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn event(owner: &str, index: usize, severity: EventSeverity) -> Event {
    Event {
        username: owner.into(),
        timestamp: chrono::Utc::now(),
        source: "boundary,\"quoted\"\nline".into(),
        event_type: format!("{owner}.{index}"),
        category: EventCategory::Kernel,
        severity,
        color: severity.color(),
        payload: json!({"index":index}),
    }
}

#[tokio::test]
async fn event_history_export_and_unread_remain_session_scoped() {
    let _guard = ENV_LOCK.lock().await;
    let f = Fixture::new();
    let alice = f.cookie("network-alice").await;
    let bob = f.cookie("network-bob").await;
    for (owner, index, severity) in [
        ("network-alice", 1, EventSeverity::Error),
        ("network-alice", 2, EventSeverity::Success),
        ("network-bob", 9, EventSeverity::Error),
    ] {
        f.state
            .event_bus
            .publish(event(owner, index, severity))
            .await;
    }
    for path in ["/events", "/events/export", "/events/unread-count"] {
        assert_eq!(
            f.request("GET", path, None, ORIGIN, Value::Null).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    let selected = f
        .get("/events?severity=error&username=network-bob", &alice)
        .await;
    assert_eq!(selected.as_array().unwrap().len(), 1);
    assert_eq!(selected[0]["username"], "network-alice");
    assert_eq!(
        f.get("/events?limit=1", &alice).await[0]["payload"]["index"],
        2
    );
    let exported = f
        .get("/events/export?format=json&severity=error", &alice)
        .await;
    assert_eq!(exported, selected);
    let response = f
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/events/export?format=csv")
                .header(header::COOKIE, &alice)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/csv; charset=utf-8"
    );
    assert!(response.headers()[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .starts_with("attachment;"));
    let csv = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(csv.contains("\"boundary,\"\"quoted\"\"\nline\""));
    assert!(!csv.contains("network-bob"));
    assert_eq!(f.get("/events/unread-count", &alice).await["unread"], 2);
    for path in ["/events/mark-read", "/events/delete?severity=error"] {
        assert_eq!(
            f.request(
                "POST",
                path,
                Some(&alice),
                "https://untrusted.example",
                Value::Null
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(f.get("/events", &alice).await.as_array().unwrap().len(), 2);
    assert_eq!(
        f.request(
            "POST",
            "/events/mark-read",
            Some(&alice),
            ORIGIN,
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(f.get("/events/unread-count", &alice).await["unread"], 0);
    assert_eq!(f.get("/events/unread-count", &bob).await["unread"], 1);
    assert_eq!(f.get("/events", &bob).await.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn filtered_event_deletion_removes_matches_not_the_complement() {
    let _guard = ENV_LOCK.lock().await;
    let f = Fixture::new();
    let alice = f.cookie("network-alice").await;
    let bob = f.cookie("network-bob").await;
    f.state
        .event_bus
        .publish(event("network-bob", 9, EventSeverity::Error))
        .await;
    let mut violations = Vec::new();
    for (query, expected_deleted, expected_remaining) in [
        ("severity=error", 1, vec![2]),
        ("category=platform", 0, vec![1, 2]),
        ("category=kernel", 2, vec![]),
    ] {
        f.state
            .event_bus
            .replace_user_events(
                "network-alice",
                vec![
                    event("network-alice", 1, EventSeverity::Error),
                    event("network-alice", 2, EventSeverity::Success),
                ],
            )
            .await;
        let (status, body) = f
            .request(
                "POST",
                &format!("/events/delete?{query}&username=network-bob"),
                Some(&alice),
                ORIGIN,
                Value::Null,
            )
            .await;
        let rows = f.get("/events", &alice).await;
        let remaining: Vec<u64> = rows
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["payload"]["index"].as_u64().unwrap())
            .collect();
        assert_eq!(f.get("/events", &bob).await.as_array().unwrap().len(), 1);
        if status != StatusCode::OK
            || body["deleted"] != expected_deleted
            || remaining != expected_remaining
        {
            violations.push(format!("{query}: status={status}, deleted={}, remaining={remaining:?}, expected_deleted={expected_deleted}, expected_remaining={expected_remaining:?}", body["deleted"]));
        }
    }
    assert!(
        violations.is_empty(),
        "filtered deletion selected the wrong side: {violations:?}"
    );
}

#[tokio::test]
async fn invalid_event_delete_filters_must_not_widen_into_delete_all() {
    let _guard = ENV_LOCK.lock().await;
    let f = Fixture::new();
    let alice = f.cookie("network-alice").await;
    let mut violations = Vec::new();
    for query in [
        "severity=errror",
        "category=kernell",
        "start=not-a-date",
        "end=not-a-date",
        "severity=",
        "category=",
        "start=",
        "end=",
        "sevrity=error",
        "since_minutes=-1",
        "since_minutes=9223372036854775807",
        "start=2026-09-09T02:00:00Z&end=2026-09-09T01:00:00Z",
    ] {
        f.state
            .event_bus
            .replace_user_events(
                "network-alice",
                vec![
                    event("network-alice", 1, EventSeverity::Success),
                    event("network-alice", 2, EventSeverity::Error),
                ],
            )
            .await;
        let (status, body) = f
            .request(
                "POST",
                &format!("/events/delete?{query}"),
                Some(&alice),
                ORIGIN,
                Value::Null,
            )
            .await;
        let remaining = f.get("/events", &alice).await.as_array().unwrap().len();
        if !status.is_client_error() || remaining != 2 {
            violations.push(format!(
                "{query}: status={status}, deleted={}, remaining={remaining}",
                body["deleted"]
            ));
        }
    }
    assert!(
        violations.is_empty(),
        "invalid destructive filters were accepted: {violations:?}"
    );
}

#[tokio::test]
async fn event_settings_http_retention_and_overflow_policy_do_not_cross_users() {
    let _guard = ENV_LOCK.lock().await;
    let f = Fixture::new();
    let alice = f.cookie("network-alice").await;
    let bob = f.cookie("network-bob").await;
    let bob_settings = f.get("/settings/events", &bob).await;
    for index in 0..55 {
        f.state
            .event_bus
            .publish(event("network-alice", index, EventSeverity::Success))
            .await;
    }
    f.state
        .event_bus
        .publish(event("network-bob", 9, EventSeverity::Success))
        .await;
    let settings = json!({"max_records":50,"overflow_policy":"drop_new","username":"network-bob"});
    assert_eq!(
        f.request(
            "POST",
            "/settings/events",
            Some(&alice),
            "https://untrusted.example",
            settings.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(f.get("/events", &alice).await.as_array().unwrap().len(), 55);
    let (status, result) = f
        .request("POST", "/settings/events", Some(&alice), ORIGIN, settings)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["settings"]["max_records"], 50);
    assert_eq!(f.get("/events", &alice).await.as_array().unwrap().len(), 50);
    assert_eq!(f.get("/events/unread-count", &alice).await["unread"], 50);
    f.state
        .event_bus
        .publish(event("network-alice", 999, EventSeverity::Success))
        .await;
    let retained = f.get("/events", &alice).await;
    assert!(retained
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["payload"]["index"] != 999));
    assert_eq!(f.get("/settings/events", &bob).await, bob_settings);
    assert_eq!(f.get("/events", &bob).await.as_array().unwrap().len(), 1);
    assert_eq!(
        f.request(
            "POST",
            "/settings/events",
            Some(&alice),
            ORIGIN,
            json!({"max_records":100,"overflow_policy":"unknown"})
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(f.get("/settings/events", &alice).await["max_records"], 50);
}
