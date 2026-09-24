use super::*;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    response::Response,
    Router,
};
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn with_settings<F, Fut>(check: F)
where
    F: FnOnce(Fixture, Router, String, String) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    with_fixture_timeout(StdDuration::from_secs(40), |mut f| async move {
        sqlx::query("INSERT INTO event_user_settings VALUES ('alice', 100, 'drop_oldest'), ('bob', 500, 'drop_oldest')").execute(&f.pool).await.unwrap();
        publish_range(&f.bus, "alice", 0..80).await;
        publish_range(&f.bus, "bob", 0..2).await;
        f.drain().await;
        f.reopen();
        let worker = tokio::spawn(event_bus_db::run_persist_loop(f.bus.clone(), f.pool.clone(), f.receiver.take().unwrap()));
        let (app, alice, bob) = super::reads::router(&f.bus).await;
        check(f, app, alice, bob).await;
        tokio::time::timeout(StdDuration::from_secs(2), worker).await.unwrap().unwrap();
    }).await;
}

async fn request(app: &Router, method: &str, cookie: &str, payload: serde_json::Value) -> Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri("/settings/events?username=bob")
                .header(header::COOKIE, cookie)
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

fn update() -> serde_json::Value {
    json!({"max_records":50,"overflow_policy":"drop_new"})
}

async fn body(response: Response, status: StatusCode) -> serde_json::Value {
    assert_eq!(
        response.status(),
        status,
        "settings storage failures must not be success or client errors"
    );
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok()),
        Some("no-store")
    );
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

async fn unavailable(response: Response, write: bool) {
    let expected = if write {
        json!({"ok":false,"message":"event settings update could not be confirmed; verify state before retrying","settings":null})
    } else {
        json!({"ok":false,"message":"event settings are unavailable"})
    };
    assert_eq!(
        body(response, StatusCode::SERVICE_UNAVAILABLE).await,
        expected
    );
}

async fn sql_settings(f: &Fixture, owner: &str) -> (i64, String) {
    let row = sqlx::query(
        "SELECT max_records, overflow_policy FROM event_user_settings WHERE username=$1",
    )
    .bind(owner)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    (row.get("max_records"), row.get("overflow_policy"))
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_settings_read_failure_is_not_cached_or_default_success() {
    for cold in [false, true] {
        with_settings(move |f, app, alice, _| async move {
            if cold {
                f.bus.settings.write().await.clear();
            }
            sqlx::query("ALTER TABLE event_user_settings RENAME TO unavailable_settings")
                .execute(&f.pool)
                .await
                .unwrap();
            assert_eq!(
                request(&app, "GET", "", json!(null)).await.status(),
                StatusCode::UNAUTHORIZED
            );
            unavailable(request(&app, "GET", &alice, json!(null)).await, false).await;
            assert!(f.bus.active_pool().is_some());
            assert_eq!(f.bus.settings.read().await.contains_key("alice"), !cold);
            sqlx::query("ALTER TABLE unavailable_settings RENAME TO event_user_settings")
                .execute(&f.pool)
                .await
                .unwrap();
            assert_eq!(
                body(
                    request(&app, "GET", &alice, json!(null)).await,
                    StatusCode::OK
                )
                .await,
                json!({"max_records":100,"overflow_policy":"drop_oldest"})
            );
        })
        .await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_settings_write_failure_is_private_and_atomic() {
    for trim_failure in [false, true] {
        with_settings(move |f, app, alice, _| async move {
            if trim_failure {
                sqlx::query("CREATE FUNCTION reject_fixture_trim() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic-private-storage-detail'; END; $$").execute(&f.pool).await.unwrap();
                sqlx::query("CREATE TRIGGER reject_trim BEFORE DELETE ON event_records FOR EACH STATEMENT EXECUTE FUNCTION reject_fixture_trim()").execute(&f.pool).await.unwrap();
            } else {
                sqlx::query("ALTER TABLE event_user_settings ADD CONSTRAINT reject_fixture_settings CHECK (max_records > 50)").execute(&f.pool).await.unwrap();
            }
            unavailable(request(&app, "POST", &alice, update()).await, true).await;
            assert_eq!(sql_settings(&f, "alice").await, (100, "drop_oldest".into()));
            assert_eq!(f.count("alice").await, 80);
            assert_eq!(f.bus.settings.read().await["alice"].max_records, 100);
            assert_eq!(f.bus.history.read().await["alice"].len(), 80);
            assert_eq!(f.bus.unread.read().await["alice"], 80);
            assert!(f.bus.active_pool().is_some());
            let repair = if trim_failure { "DROP TRIGGER reject_trim ON event_records" }
                else { "ALTER TABLE event_user_settings DROP CONSTRAINT reject_fixture_settings" };
            sqlx::query(repair).execute(&f.pool).await.unwrap();
            let response = body(request(&app, "POST", &alice, update()).await, StatusCode::OK).await;
            assert_eq!(response["ok"], true);
            assert_eq!(response["settings"], update());
            assert_eq!(f.count("alice").await, 50);
            assert_eq!(sql_settings(&f, "alice").await, (50, "drop_new".into()));
            assert_eq!(f.count("bob").await, 2);
            assert_eq!(sql_settings(&f, "bob").await, (500, "drop_oldest".into()));
        }).await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_settings_prior_fallback_does_not_publish_or_trim_memory() {
    with_settings(|f, app, alice, _| async move {
        f.bus.disable_db("synthetic prior write failure");
        unavailable(request(&app, "GET", &alice, json!(null)).await, false).await;
        unavailable(request(&app, "POST", &alice, update()).await, true).await;
        assert_eq!(f.bus.settings.read().await["alice"].max_records, 100);
        assert_eq!(f.bus.history.read().await["alice"].len(), 80);
        assert_eq!(f.bus.unread.read().await["alice"], 80);
        assert_eq!(sql_settings(&f, "alice").await, (100, "drop_oldest".into()));
        assert_eq!(f.count("alice").await, 80);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_settings_failed_barrier_cannot_publish_or_trim_policy() {
    with_settings(|f, app, alice, _| async move {
        sqlx::query("ALTER TABLE event_records ADD CHECK (event_type <> 'rejected-fixture')")
            .execute(&f.pool)
            .await
            .unwrap();
        let mut rejected = event("alice", 80);
        rejected.event_type = "rejected-fixture".into();
        f.bus.publish(rejected).await;
        unavailable(request(&app, "POST", &alice, update()).await, true).await;
        assert_eq!(f.bus.settings.read().await["alice"].max_records, 100);
        assert_eq!(f.bus.history.read().await["alice"].len(), 81);
        assert_eq!(f.bus.unread.read().await["alice"], 81);
        assert_eq!(f.count("alice").await, 80);
        assert_eq!(sql_settings(&f, "alice").await, (100, "drop_oldest".into()));
        assert!(f.bus.active_pool().is_none());
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_settings_corrupt_values_are_not_default_or_panicking_reads() {
    with_settings(|f, app, alice, _| async move {
        f.bus.settings.write().await.clear();
        for corrupt in [
            "UPDATE event_user_settings SET overflow_policy='unknown' WHERE username='alice'",
            "UPDATE event_user_settings SET overflow_policy='drop_new',max_records=0 WHERE username='alice'",
            "UPDATE event_user_settings SET max_records=50001 WHERE username='alice'",
            "ALTER TABLE event_user_settings ALTER COLUMN max_records TYPE TEXT USING max_records::text",
        ] {
            sqlx::query(corrupt).execute(&f.pool).await.unwrap();
            unavailable(request(&app, "GET", &alice, json!(null)).await, false).await;
            assert!(f.bus.active_pool().is_some());
            assert!(!f.bus.settings.read().await.contains_key("alice"));
        }
        sqlx::query("ALTER TABLE event_user_settings ALTER COLUMN max_records TYPE BIGINT USING max_records::bigint").execute(&f.pool).await.unwrap();
        sqlx::query("UPDATE event_user_settings SET max_records=100 WHERE username='alice'").execute(&f.pool).await.unwrap();
        assert_eq!(body(request(&app, "GET", &alice, json!(null)).await, StatusCode::OK).await,
            json!({"max_records":100,"overflow_policy":"drop_new"}));
    }).await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_settings_commit_wait_is_bounded_without_claiming_rollback() {
    with_settings(|f, app, alice, _| async move {
        let cache = f.bus.settings.write().await;
        let response = tokio::time::timeout(
            StdDuration::from_secs(12),
            request(&app, "POST", &alice, update()),
        )
        .await;
        drop(cache);
        unavailable(
            response.expect("settings caller waiting must be bounded"),
            true,
        )
        .await;
        let _admission =
            tokio::time::timeout(StdDuration::from_secs(2), f.bus.mutation_admission("alice"))
                .await
                .unwrap();
        assert_eq!(sql_settings(&f, "alice").await, (50, "drop_new".into()));
        assert_eq!(f.bus.settings.read().await["alice"].max_records, 50);
        assert_eq!(f.bus.history.read().await["alice"].len(), 50);
        assert_eq!(f.bus.unread.read().await["alice"], 50);
        assert_eq!(f.count("alice").await, 50);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_settings_read_deadline_and_cancellation_remain_retryable() {
    with_settings(|f, app, alice, _| async move {
        let mut held = Vec::new();
        for _ in 0..5 {
            held.push(f.pool.acquire().await.unwrap());
        }
        assert!(tokio::time::timeout(
            StdDuration::from_millis(50),
            request(&app, "GET", &alice, json!(null))
        )
        .await
        .is_err());
        let response = tokio::time::timeout(
            StdDuration::from_secs(12),
            request(&app, "GET", &alice, json!(null)),
        )
        .await;
        drop(held);
        unavailable(
            response.expect("settings read waiting must be bounded"),
            false,
        )
        .await;
        assert!(f.bus.active_pool().is_some());
        assert_eq!(
            body(
                request(&app, "GET", &alice, json!(null)).await,
                StatusCode::OK
            )
            .await,
            json!({"max_records":100,"overflow_policy":"drop_oldest"})
        );
        assert_eq!(f.count("alice").await, 80);
    })
    .await;
}

async fn wait_for_blocked_trim(f: &Fixture) {
    tokio::time::timeout(StdDuration::from_secs(2), async {
        loop {
            let row = sqlx::query("SELECT count(*) AS count FROM pg_stat_activity WHERE wait_event_type='Lock' AND query LIKE 'DELETE FROM event_records WHERE id IN (%'")
                .fetch_one(&f.pool).await.unwrap();
            if row.get::<i64, _>("count") > 0 { break; }
            tokio::time::sleep(StdDuration::from_millis(5)).await;
        }
    }).await.unwrap();
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_settings_cancelled_update_orders_later_events_under_new_policy() {
    with_settings(|f, app, alice, _| async move {
        let mut blocker = f.pool.begin().await.unwrap();
        sqlx::query("LOCK TABLE event_records IN SHARE MODE")
            .execute(&mut *blocker)
            .await
            .unwrap();
        let task = tokio::spawn(async move { request(&app, "POST", &alice, update()).await });
        wait_for_blocked_trim(&f).await;
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        let later_bus = f.bus.clone();
        let later = tokio::spawn(async move { later_bus.publish(event("alice", 80)).await });
        tokio::time::timeout(StdDuration::from_secs(1), f.bus.publish(event("bob", 2)))
            .await
            .unwrap();
        tokio::time::sleep(StdDuration::from_millis(50)).await;
        assert!(
            !later.is_finished(),
            "caller cancellation cannot release admitted SQL ordering"
        );
        blocker.rollback().await.unwrap();
        tokio::time::timeout(StdDuration::from_secs(2), later)
            .await
            .unwrap()
            .unwrap();
        f.bus.flush_persistence().await;
        assert_eq!(sql_settings(&f, "alice").await, (50, "drop_new".into()));
        assert_eq!(
            sequences(&f.bus.snapshot_for_user("alice").await),
            (30..80).collect::<Vec<_>>()
        );
        assert_eq!(f.bus.history.read().await["alice"].len(), 50);
        assert_eq!(f.count("alice").await, 50);
        assert_eq!(f.count("bob").await, 3);
        assert_eq!(sql_settings(&f, "bob").await, (500, "drop_oldest".into()));
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_settings_sql_timeout_preserves_uncommitted_policy_and_history() {
    with_settings(|f, app, alice, _| async move {
        let mut blocker = f.pool.begin().await.unwrap();
        sqlx::query("LOCK TABLE event_records IN SHARE MODE")
            .execute(&mut *blocker)
            .await
            .unwrap();
        let task = tokio::spawn(async move { request(&app, "POST", &alice, update()).await });
        wait_for_blocked_trim(&f).await;
        unavailable(
            tokio::time::timeout(StdDuration::from_secs(12), task)
                .await
                .unwrap()
                .unwrap(),
            true,
        )
        .await;
        let admission =
            tokio::time::timeout(StdDuration::from_secs(2), f.bus.mutation_admission("alice"))
                .await
                .unwrap();
        blocker.rollback().await.unwrap();
        // This fixture blocks DELETE before commit. It is not evidence that every 503 rolls back.
        assert_eq!(sql_settings(&f, "alice").await, (100, "drop_oldest".into()));
        assert_eq!(f.count("alice").await, 80);
        assert_eq!(f.bus.settings.read().await["alice"].max_records, 100);
        assert_eq!(f.bus.history.read().await["alice"].len(), 80);
        assert_eq!(f.bus.unread.read().await["alice"], 80);
        assert!(f.bus.active_pool().is_none());
        drop(admission);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_settings_success_reloads_and_preserves_read_flags_and_capacity() {
    with_settings(|f, app, alice, bob| async move {
        sqlx::query("DELETE FROM event_user_settings WHERE username='alice'").execute(&f.pool).await.unwrap();
        assert_eq!(body(request(&app, "GET", &alice, json!(null)).await, StatusCode::OK).await,
            json!({"max_records":500,"overflow_policy":"drop_oldest"}));
        // Missing durable settings are a valid default, without overwriting the runtime cache.
        assert_eq!(f.bus.settings.read().await["alice"].max_records, 100);
        f.bus.acknowledge_all_read_for_user("alice").await.unwrap();
        publish_range(&f.bus, "alice", 80..100).await;
        f.bus.drop_new_admitted.write().await.insert("alice".into(), 999);
        f.bus.drop_new_count_cache.write().await.insert("alice".into(), (999, Instant::now()));
        assert_eq!(body(request(&app, "POST", &alice, update()).await, StatusCode::OK).await["settings"], update());
        assert_eq!(f.bus.unread.read().await["alice"], 20);
        assert!(!f.bus.drop_new_admitted.read().await.contains_key("alice"));
        assert!(!f.bus.drop_new_count_cache.read().await.contains_key("alice"));
        let row = sqlx::query("SELECT count(*) FILTER (WHERE is_read) AS read, count(*) FILTER (WHERE NOT is_read) AS unread FROM event_records WHERE username='alice'").fetch_one(&f.pool).await.unwrap();
        assert_eq!((row.get::<i64, _>("read"), row.get::<i64, _>("unread")), (30, 20));
        let mut fresh = memory_bus();
        fresh.db_pool = Some(f.pool.clone());
        fresh.db_disabled.store(false, Ordering::Relaxed);
        let reloaded = fresh.read_settings_for_user("alice").await.unwrap();
        assert_eq!(reloaded.max_records, 50);
        assert_eq!(reloaded.overflow_policy, EventOverflowPolicy::DropNew);
        assert!(fresh.settings.read().await.is_empty());
        assert_eq!(body(request(&app, "GET", &bob, json!(null)).await, StatusCode::OK).await,
            json!({"max_records":500,"overflow_policy":"drop_oldest"}));
        f.bus.publish(event("alice", 100)).await;
        f.bus.flush_persistence().await;
        assert_eq!(f.count("alice").await, 50);
        assert_eq!(sequences(&f.bus.snapshot_for_user("alice").await), (50..100).collect::<Vec<_>>());
    }).await;
}
