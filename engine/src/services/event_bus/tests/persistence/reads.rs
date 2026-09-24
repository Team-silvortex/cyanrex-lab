use super::*;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    response::Response,
    Router,
};
use http_body_util::BodyExt;
use tower::ServiceExt;

pub(super) async fn router(bus: &EventBus) -> (Router, String, String) {
    // Only EventBus uses the explicitly disposable pool; other services see no DATABASE_URL.
    let mut state = (*crate::build_state()).clone();
    state.event_bus = bus.clone();
    let mut cookies = Vec::new();
    for owner in ["alice", "bob"] {
        state
            .auth_service
            .register(owner, "synthetic-read-password")
            .await
            .unwrap();
        let otp = state
            .auth_service
            .generate_current_totp_for_user(owner)
            .unwrap();
        let login = state
            .auth_service
            .login(owner, "synthetic-read-password", &otp)
            .await
            .unwrap();
        cookies.push(format!("cyanrex_session={}", login.token));
    }
    (
        crate::build_router(Arc::new(state)),
        cookies.remove(0),
        cookies.remove(0),
    )
}

async fn get(app: &Router, path: &str, cookie: &str) -> Response {
    app.clone()
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

async fn unavailable(response: Response) {
    assert_eq!(
        response.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "read failure must not masquerade as successful history/unread/download"
    );
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert!(!response.headers().contains_key(header::CONTENT_DISPOSITION));
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        body,
        json!({"ok": false, "message": "event storage is unavailable"})
    );
}

async fn seed(f: &mut Fixture) {
    publish_range(&f.bus, "alice", 0..2).await;
    publish_range(&f.bus, "bob", 0..1).await;
    f.drain().await;
}

async fn read_failure(path: &'static str) {
    for cold in [false, true] {
        with_fixture_timeout(StdDuration::from_secs(30), move |mut f| async move {
            seed(&mut f).await;
            if cold {
                f.bus.history.write().await.clear();
                f.bus.unread.write().await.clear();
            }
            let (app, alice, bob) = router(&f.bus).await;
            sqlx::query("ALTER TABLE event_records RENAME TO unavailable_records")
                .execute(&f.pool)
                .await
                .unwrap();
            assert_eq!(get(&app, path, "").await.status(), StatusCode::UNAUTHORIZED);
            unavailable(get(&app, path, &alice).await).await;
            assert!(
                f.bus.active_pool().is_some(),
                "read failure must not disable future persistence"
            );
            sqlx::query("ALTER TABLE unavailable_records RENAME TO event_records")
                .execute(&f.pool)
                .await
                .unwrap();
            let response = get(&app, path, &alice).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            if path.contains("unread-count") {
                assert_eq!(body, json!({"unread": 2}));
            } else {
                assert_eq!(body.as_array().unwrap().len(), 2);
                assert!(body
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|e| e["username"] == "alice"));
            }
            let body: serde_json::Value = serde_json::from_slice(
                &get(&app, "/events", &bob)
                    .await
                    .into_body()
                    .collect()
                    .await
                    .unwrap()
                    .to_bytes(),
            )
            .unwrap();
            assert_eq!(body.as_array().unwrap().len(), 1);
            // Reading failed, but subsequent writes must still reach SQL after repair.
            f.reopen();
            f.bus.publish(event("alice", 2)).await;
            f.drain().await;
            assert_eq!(f.count("alice").await, 3);
        })
        .await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_history_read_failure_is_unavailable_and_retryable() {
    read_failure("/events?username=bob").await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_export_read_failure_is_not_a_successful_download() {
    read_failure("/events/export?format=json&username=bob").await;
    with_fixture_timeout(StdDuration::from_secs(30), |mut f| async move {
        seed(&mut f).await;
        let (app, alice, _) = router(&f.bus).await;
        sqlx::query("ALTER TABLE event_records RENAME TO unavailable_records")
            .execute(&f.pool)
            .await
            .unwrap();
        unavailable(get(&app, "/events/export?format=csv", &alice).await).await;
        sqlx::query("ALTER TABLE unavailable_records RENAME TO event_records")
            .execute(&f.pool)
            .await
            .unwrap();
        let response = get(&app, "/events/export?format=csv", &alice).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert!(response.headers().contains_key(header::CONTENT_DISPOSITION));
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_unread_read_failure_is_unavailable_not_zero() {
    read_failure("/events/unread-count").await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_schema_read_failure_is_retryable_without_memory_success() {
    with_fixture_timeout(StdDuration::from_secs(30), |mut f| async move {
        seed(&mut f).await;
        f.bus.schema_ready = Arc::new(OnceCell::new());
        sqlx::query("DROP INDEX idx_event_records_user_unread")
            .execute(&f.pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE event_records RENAME COLUMN is_read TO unavailable_read")
            .execute(&f.pool)
            .await
            .unwrap();
        let (app, alice, _) = router(&f.bus).await;
        unavailable(get(&app, "/events", &alice).await).await;
        assert!(f.bus.active_pool().is_some());
        sqlx::query("ALTER TABLE event_records RENAME COLUMN unavailable_read TO is_read")
            .execute(&f.pool)
            .await
            .unwrap();
        assert_eq!(get(&app, "/events", &alice).await.status(), StatusCode::OK);
        assert_eq!(f.count("alice").await, 2);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_read_timeout_releases_waiting_without_disabling_storage() {
    with_fixture_timeout(StdDuration::from_secs(30), |mut f| async move {
        seed(&mut f).await;
        let (app, alice, _) = router(&f.bus).await;
        let mut connections = Vec::new();
        for _ in 0..5 {
            connections.push(f.pool.acquire().await.unwrap());
        }
        assert!(
            tokio::time::timeout(StdDuration::from_millis(50), get(&app, "/events", &alice))
                .await
                .is_err()
        );
        assert!(
            f.bus.active_pool().is_some(),
            "abandoned reads must not poison storage"
        );
        let result = tokio::time::timeout(StdDuration::from_secs(12), async {
            tokio::join!(
                get(&app, "/events", &alice),
                get(&app, "/events/export", &alice),
                get(&app, "/events/unread-count", &alice)
            )
        })
        .await;
        drop(connections);
        let (history, export, unread) = result.expect("selected reads must have a bounded wait");
        unavailable(history).await;
        unavailable(export).await;
        unavailable(unread).await;
        assert!(f.bus.active_pool().is_some());
        assert_eq!(get(&app, "/events", &alice).await.status(), StatusCode::OK);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_latched_fallback_reads_stay_unavailable_until_fresh_state() {
    with_fixture_timeout(StdDuration::from_secs(30), |mut f| async move {
        seed(&mut f).await;
        f.bus
            .disable_db("synthetic previously latched write failure");
        f.bus.publish(event("alice", 99)).await;
        let (app, alice, _) = router(&f.bus).await;
        for path in [
            "/events",
            "/events/export?format=json",
            "/events/export?format=csv",
            "/events/unread-count",
        ] {
            unavailable(get(&app, path, &alice).await).await;
        }
        let mut fresh = memory_bus();
        fresh.db_pool = Some(f.pool.clone());
        fresh.db_disabled.store(false, Ordering::Relaxed);
        let (restarted, alice, _) = router(&fresh).await;
        let bytes = get(&restarted, "/events", &alice)
            .await
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let rows: Vec<Event> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            sequences(&rows),
            vec![0, 1],
            "volatile publications are not automatically reconciled on restart"
        );
    })
    .await;
}

async fn decode_failure(column: &'static str, restore_type: &'static str) {
    with_fixture_timeout(StdDuration::from_secs(30), move |mut f| async move {
        seed(&mut f).await;
        sqlx::query(&format!("ALTER TABLE event_records ALTER COLUMN {column} TYPE TEXT USING {column}::text")).execute(&f.pool).await.unwrap();
        let (app, alice, _) = router(&f.bus).await;
        unavailable(get(&app, "/events", &alice).await).await;
        assert!(f.bus.active_pool().is_some());
        sqlx::query(&format!("ALTER TABLE event_records ALTER COLUMN {column} TYPE {restore_type} USING {column}::{restore_type}")).execute(&f.pool).await.unwrap();
        assert_eq!(get(&app, "/events", &alice).await.status(), StatusCode::OK);
    }).await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_payload_decode_failure_is_not_a_fabricated_empty_object() {
    decode_failure("payload", "JSONB").await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_timestamp_decode_failure_is_unavailable_not_a_panic() {
    decode_failure("timestamp", "TIMESTAMPTZ").await;
}
