use super::*;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    response::Response,
    Router,
};
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn with_http<F, Fut>(check: F)
where
    F: FnOnce(Fixture, Router, String, String) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    with_fixture_timeout(StdDuration::from_secs(30), |mut f| async move {
        publish_range(&f.bus, "alice", 0..4).await;
        publish_range(&f.bus, "bob", 0..2).await;
        f.drain().await;
        f.reopen();
        let worker = tokio::spawn(event_bus_db::run_persist_loop(
            f.bus.clone(),
            f.pool.clone(),
            f.receiver.take().unwrap(),
        ));
        let (app, alice, bob) = super::reads::router(&f.bus).await;
        check(f, app, alice, bob).await;
        tokio::time::timeout(StdDuration::from_secs(2), worker)
            .await
            .unwrap()
            .unwrap();
    })
    .await;
}

async fn post(app: &Router, path: &str, cookie: &str) -> Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header(header::COOKIE, cookie)
                .header(header::ORIGIN, "http://localhost:3000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn body(response: Response, status: StatusCode) -> serde_json::Value {
    assert_eq!(
        response.status(),
        status,
        "mutation must not claim unconfirmed success"
    );
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

async fn unconfirmed(response: Response) {
    assert_eq!(
        body(response, StatusCode::SERVICE_UNAVAILABLE).await,
        json!({"ok": false, "message": "event mutation could not be confirmed; verify state before retrying"})
    );
}

async fn unread_sql(f: &Fixture, owner: &str) -> i64 {
    sqlx::query("SELECT count(*) AS count FROM event_records WHERE username = $1 AND NOT is_read")
        .bind(owner)
        .fetch_one(&f.pool)
        .await
        .unwrap()
        .get("count")
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_mark_read_failure_preserves_unread_and_can_be_retried() {
    with_http(|f, app, alice, _| async move {
        sqlx::query("ALTER TABLE event_records RENAME TO unavailable_records")
            .execute(&f.pool)
            .await
            .unwrap();
        assert_eq!(
            post(&app, "/events/mark-read", "").await.status(),
            StatusCode::UNAUTHORIZED
        );
        unconfirmed(post(&app, "/events/mark-read", &alice).await).await;
        assert_eq!(f.bus.unread.read().await["alice"], 4);
        assert!(
            f.bus.active_pool().is_some(),
            "a definite query error is retryable"
        );
        sqlx::query("ALTER TABLE unavailable_records RENAME TO event_records")
            .execute(&f.pool)
            .await
            .unwrap();
        assert_eq!(
            body(
                post(&app, "/events/mark-read?username=bob", &alice).await,
                StatusCode::OK
            )
            .await,
            json!({"ok": true})
        );
        assert_eq!(unread_sql(&f, "alice").await, 0);
        assert_eq!(unread_sql(&f, "bob").await, 2);
        assert_eq!(f.bus.unread.read().await["alice"], 0);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_delete_failure_preserves_history_and_can_be_retried() {
    for (path, count, survivors) in [
        ("/events/delete?username=bob", 4, vec![]),
        ("/events/delete?category=kernel&username=bob", 2, vec![1, 3]),
        ("/events/delete?severity=error", 0, vec![0, 1, 2, 3]),
    ] {
        with_http(move |f, app, alice, _| async move {
            sqlx::query("ALTER TABLE event_records RENAME TO unavailable_records")
                .execute(&f.pool)
                .await
                .unwrap();
            unconfirmed(post(&app, path, &alice).await).await;
            assert_eq!(
                sequences(
                    &f.bus.history.read().await["alice"]
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                ),
                vec![0, 1, 2, 3]
            );
            assert_eq!(f.bus.unread.read().await["alice"], 4);
            assert!(f.bus.active_pool().is_some());
            sqlx::query("ALTER TABLE unavailable_records RENAME TO event_records")
                .execute(&f.pool)
                .await
                .unwrap();
            assert_eq!(
                body(post(&app, path, &alice).await, StatusCode::OK).await,
                json!({"ok":true,"deleted":count})
            );
            assert_eq!(f.count("alice").await, 4 - count);
            assert_eq!(
                sequences(&f.bus.snapshot_for_user("alice").await),
                survivors
            );
            assert_eq!(f.count("bob").await, 2);
            assert_eq!(unread_sql(&f, "bob").await, 2);
        })
        .await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_mutations_reject_latched_fallback_without_memory_changes() {
    with_http(|f, app, alice, _| async move {
        f.bus.disable_db("synthetic prior write failure");
        for path in [
            "/events/mark-read",
            "/events/delete?category=kernel",
            "/events/delete",
        ] {
            unconfirmed(post(&app, path, &alice).await).await;
            assert_eq!(f.bus.history.read().await["alice"].len(), 4);
            assert_eq!(f.bus.unread.read().await["alice"], 4);
        }
        assert_eq!(f.count("alice").await, 4);
        assert_eq!(unread_sql(&f, "alice").await, 4);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_failed_mutation_barrier_does_not_acknowledge_volatile_changes() {
    with_http(|f, app, alice, _| async move {
        sqlx::query("ALTER TABLE event_records ADD CHECK (event_type <> 'rejected-fixture')")
            .execute(&f.pool)
            .await
            .unwrap();
        let mut rejected = event("alice", 9);
        rejected.event_type = "rejected-fixture".into();
        f.bus.publish(rejected).await;
        for path in ["/events/mark-read", "/events/delete"] {
            unconfirmed(post(&app, path, &alice).await).await;
            assert_eq!(f.bus.history.read().await["alice"].len(), 5);
            assert_eq!(f.bus.unread.read().await["alice"], 5);
        }
        assert!(f.bus.active_pool().is_none());
        assert_eq!(f.count("alice").await, 4);
        assert_eq!(unread_sql(&f, "alice").await, 4);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_mutation_wait_deadline_does_not_claim_rollback_or_success() {
    for path in ["/events/mark-read", "/events/delete"] {
        with_http(move |f, app, alice, _| async move {
            // SQL may commit before local publication. The caller's deadline is not rollback.
            let unread = f.bus.unread.write().await;
            let response =
                tokio::time::timeout(StdDuration::from_secs(12), post(&app, path, &alice)).await;
            drop(unread);
            unconfirmed(response.expect("HTTP mutation waiting must be bounded")).await;
            let admission =
                tokio::time::timeout(StdDuration::from_secs(2), f.bus.mutation_admission("alice"))
                    .await
                    .unwrap();
            assert_eq!(f.bus.unread.read().await["alice"], 0);
            if path.ends_with("delete") {
                assert_eq!(f.count("alice").await, 0);
            } else {
                assert_eq!(unread_sql(&f, "alice").await, 0);
            }
            assert_eq!(f.count("bob").await, 2);
            drop(admission);
        })
        .await;
    }
}

async fn wait_for_blocked_write(f: &Fixture) {
    tokio::time::timeout(StdDuration::from_secs(2), async {
        loop {
            let row = sqlx::query("SELECT count(*) AS count FROM pg_stat_activity WHERE wait_event_type = 'Lock' AND (query LIKE 'UPDATE event_records%' OR query LIKE 'DELETE FROM event_records%')")
                .fetch_one(&f.pool).await.unwrap();
            if row.get::<i64, _>("count") > 0 { break; }
            tokio::time::sleep(StdDuration::from_millis(5)).await;
        }
    }).await.unwrap();
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_cancelled_confirmed_mutation_finishes_before_later_publication() {
    for path in ["/events/mark-read", "/events/delete"] {
        with_http(move |f, app, alice, _| async move {
            let mut blocker = f.pool.begin().await.unwrap();
            sqlx::query("LOCK TABLE event_records IN SHARE MODE")
                .execute(&mut *blocker)
                .await
                .unwrap();
            let task = tokio::spawn(async move { post(&app, path, &alice).await });
            wait_for_blocked_write(&f).await;
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
            let later_bus = f.bus.clone();
            let later = tokio::spawn(async move { later_bus.publish(event("alice", 9)).await });
            tokio::time::timeout(
                StdDuration::from_millis(100),
                f.bus.publish(event("bob", 9)),
            )
            .await
            .unwrap();
            assert!(!later.is_finished());
            blocker.rollback().await.unwrap();
            tokio::time::timeout(StdDuration::from_secs(2), later)
                .await
                .unwrap()
                .unwrap();
            let (sender, receiver) = tokio::sync::oneshot::channel();
            f.bus
                .persist_sender
                .as_ref()
                .unwrap()
                .send(event_bus_db::PersistMessage::Barrier(sender))
                .await
                .unwrap();
            assert!(tokio::time::timeout(StdDuration::from_secs(2), receiver)
                .await
                .unwrap()
                .unwrap());
            assert_eq!(unread_sql(&f, "alice").await, 1);
            assert_eq!(f.bus.unread.read().await["alice"], 1);
            assert_eq!(
                f.count("alice").await,
                if path.ends_with("delete") { 1 } else { 5 }
            );
            assert_eq!(f.count("bob").await, 3);
        })
        .await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_mutation_sql_timeout_never_falls_back_to_memory_success() {
    for path in ["/events/mark-read", "/events/delete"] {
        with_http(move |f, app, alice, _| async move {
            let mut blocker = f.pool.begin().await.unwrap();
            sqlx::query("LOCK TABLE event_records IN SHARE MODE")
                .execute(&mut *blocker)
                .await
                .unwrap();
            let response =
                tokio::time::timeout(StdDuration::from_secs(12), post(&app, path, &alice))
                    .await
                    .unwrap();
            unconfirmed(response).await;
            // Keep SQL blocked until the worker's own deadline, not merely the caller's deadline.
            let admission =
                tokio::time::timeout(StdDuration::from_secs(2), f.bus.mutation_admission("alice"))
                    .await
                    .unwrap();
            assert!(f.bus.active_pool().is_none());
            assert_eq!(f.bus.unread.read().await["alice"], 4);
            assert_eq!(f.bus.history.read().await["alice"].len(), 4);
            drop(admission);
            blocker.rollback().await.unwrap();
            // The server may have received the statement: deliberately do not claim SQL rollback.
        })
        .await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_mutation_schema_failure_preserves_memory_and_recovers() {
    with_http(|mut f, app, _, _| async move {
        drop(app);
        f.bus.schema_ready = Arc::new(OnceCell::new());
        sqlx::query("DROP INDEX idx_event_records_user_unread")
            .execute(&f.pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE event_records RENAME COLUMN is_read TO unavailable_read")
            .execute(&f.pool)
            .await
            .unwrap();
        let (app, alice, _) = super::reads::router(&f.bus).await;
        for path in ["/events/mark-read", "/events/delete"] {
            unconfirmed(post(&app, path, &alice).await).await;
            assert_eq!(f.bus.unread.read().await["alice"], 4);
            assert_eq!(f.bus.history.read().await["alice"].len(), 4);
            assert!(f.bus.active_pool().is_some());
        }
        sqlx::query("ALTER TABLE event_records RENAME COLUMN unavailable_read TO is_read")
            .execute(&f.pool)
            .await
            .unwrap();
        assert_eq!(
            body(
                post(&app, "/events/mark-read", &alice).await,
                StatusCode::OK
            )
            .await,
            json!({"ok": true})
        );
        assert_eq!(
            body(
                post(&app, "/events/delete?category=kernel", &alice).await,
                StatusCode::OK
            )
            .await,
            json!({"ok":true,"deleted":2})
        );
        assert_eq!(f.count("alice").await, 2);
        assert_eq!(unread_sql(&f, "alice").await, 0);
        assert_eq!(unread_sql(&f, "bob").await, 2);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_confirmed_cold_delete_counts_sql_rows_and_preserves_read_flags() {
    with_http(|f, app, alice, _| async move {
        body(
            post(&app, "/events/mark-read", &alice).await,
            StatusCode::OK,
        )
        .await;
        publish_range(&f.bus, "alice", 4..8).await;
        // The HTTP deletion's own barrier must flush these pending publications.
        f.bus.history.write().await.clear();
        f.bus.unread.write().await.clear();
        f.bus
            .drop_new_admitted
            .write()
            .await
            .insert("alice".into(), 8);
        f.bus
            .drop_new_count_cache
            .write()
            .await
            .insert("alice".into(), (8, Instant::now()));
        assert_eq!(
            body(
                post(&app, "/events/delete?category=kernel&username=bob", &alice).await,
                StatusCode::OK
            )
            .await,
            json!({"ok": true, "deleted": 4})
        );
        assert_eq!(f.count("alice").await, 4);
        assert_eq!(unread_sql(&f, "alice").await, 2);
        assert_eq!(
            sequences(&f.bus.snapshot_for_user("alice").await),
            vec![1, 3, 5, 7]
        );
        assert!(!f.bus.drop_new_admitted.read().await.contains_key("alice"));
        assert!(!f
            .bus
            .drop_new_count_cache
            .read()
            .await
            .contains_key("alice"));
        assert_eq!(
            body(
                post(&app, "/events/delete?category=kernel", &alice).await,
                StatusCode::OK
            )
            .await,
            json!({"ok": true, "deleted": 0})
        );
        assert_eq!(f.count("bob").await, 2);
        assert_eq!(unread_sql(&f, "bob").await, 2);
    })
    .await;
}
