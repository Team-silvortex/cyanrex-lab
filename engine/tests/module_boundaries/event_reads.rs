use super::support::*;
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn read(f: &Fixture, path: &str, cookie: &str) -> axum::response::Response {
    f.app
        .clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn event_reads_reject_invalid_filters_without_widening() {
    let _guard = ENV_LOCK.lock().await;
    let f = Fixture::new();
    let cookie = f.cookie("network-alice").await;
    let mut violations = Vec::new();
    for base in [
        "/events?",
        "/events/export?format=json&",
        "/events/export?format=csv&",
    ] {
        for query in [
            "category=typo",
            "severity=typo",
            "category=",
            "severity=",
            "start=",
            "end=",
            "start=not-a-date",
            "end=not-a-date",
            "since_minutes=-1",
            "sevrity=error",
            "start=2026-09-24T02:00:00Z&end=2026-09-24T01:00:00Z",
        ] {
            let path = format!("{base}{query}");
            let status = read(&f, &path, &cookie).await.status();
            if status != StatusCode::BAD_REQUEST {
                violations.push((path, status));
            }
        }
    }
    for format in ["xml", "", "../csv"] {
        let path = format!("/events/export?format={format}");
        let status = read(&f, &path, &cookie).await.status();
        if status != StatusCode::BAD_REQUEST {
            violations.push((path, status));
        }
    }
    assert!(
        violations.is_empty(),
        "invalid read filters were silently accepted: {violations:?}"
    );
    assert!(f.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn event_read_time_overflow_is_bad_request_not_panic() {
    let _guard = ENV_LOCK.lock().await;
    let f = Fixture::new();
    let cookie = f.cookie("network-alice").await;
    // Ensure the in-memory filter actually visits a retained bucket.
    f.state
        .event_bus
        .publish(cyanrex_engine::models::event::Event {
            username: "network-alice".into(),
            timestamp: chrono::Utc::now(),
            source: "fixture".into(),
            event_type: "fixture".into(),
            category: cyanrex_engine::models::event::EventCategory::Kernel,
            severity: cyanrex_engine::models::event::EventSeverity::Success,
            color: cyanrex_engine::models::event::EventColor::Green,
            payload: json!({"seq": 1}),
        })
        .await;
    let mut violations = Vec::new();
    for base in [
        "/events?",
        "/events/export?format=json&",
        "/events/export?format=csv&",
    ] {
        for minutes in ["9223372036854775807", "1000000000000"] {
            let app = f.app.clone();
            let request = Request::builder()
                .uri(format!("{base}since_minutes={minutes}"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap();
            let result =
                tokio::spawn(async move { app.oneshot(request).await.unwrap().status() }).await;
            if !matches!(result, Ok(StatusCode::BAD_REQUEST)) {
                violations.push(format!("{base}{minutes}: {result:?}"));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "overflow must be a checked client error: {violations:?}"
    );
}

#[tokio::test]
async fn event_read_responses_are_private_and_keep_legacy_ignored_owner_fields() {
    let _guard = ENV_LOCK.lock().await;
    let f = Fixture::new();
    let cookie = f.cookie("network-alice").await;
    for path in [
        "/events?limit=0&username=network-bob&format=csv",
        "/events?category=%20KERNEL%20&severity=SUCCESS&since_minutes=0",
        "/events/export?format=JSON&username=network-bob&limit=1",
        "/events/export?format=csv",
        "/events/unread-count",
    ] {
        let response = read(&f, path, &cookie).await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|v| v.to_str().ok()),
            Some("no-store"),
            "{path}"
        );
    }
    for path in [
        "/events?limit=not-a-number",
        "/events?limit=-1",
        "/events?since_minutes=abc",
        "/events?severity=error&severity=warning",
        "/events/export?unknown=true",
    ] {
        let response = read(&f, path, &cookie).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path}");
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["ok"], false);
    }
}
