use super::*;

#[tokio::test]
async fn deletion_preserves_read_state_and_other_users() {
    let bus = memory_bus();
    publish_range(&bus, "alice", 0..4).await;
    bus.mark_all_read_for_user("alice").await;
    publish_range(&bus, "alice", 4..8).await;
    publish_range(&bus, "bob", 0..3).await;
    assert_eq!(
        bus.delete_user_events_filtered("alice", Some("kernel"), None, None, None, None)
            .await,
        4
    );
    assert_eq!(
        sequences(&bus.snapshot_for_user("alice").await),
        vec![1, 3, 5, 7]
    );
    assert_eq!(bus.unread_count_for_user("alice").await, 2);
    assert_eq!(
        sequences(&bus.snapshot_for_user("bob").await),
        vec![0, 1, 2]
    );
    assert_eq!(bus.unread_count_for_user("bob").await, 3);
}

#[tokio::test]
async fn invalid_service_filters_never_widen_deletion() {
    let bus = memory_bus();
    publish_range(&bus, "alice", 0..8).await;
    for (category, severity, minutes, start, end) in [
        (Some("typo"), None, None, None, None),
        (None, Some(""), None, None, None),
        (None, None, Some(-1), None, None),
        (None, None, Some(i64::MAX), None, None),
        (
            None,
            None,
            None,
            Some(event("alice", 7).timestamp),
            Some(event("alice", 0).timestamp),
        ),
    ] {
        assert_eq!(
            bus.delete_user_events_filtered("alice", category, severity, minutes, start, end)
                .await,
            0
        );
        assert_eq!(bus.snapshot_for_user("alice").await.len(), 8);
        assert_eq!(bus.unread_count_for_user("alice").await, 8);
    }
}

#[tokio::test]
async fn cancelled_deletion_never_splits_history_and_unread() {
    let bus = memory_bus();
    publish_range(&bus, "alice", 0..8).await;
    for category in [Some("kernel"), None] {
        let unread = bus.unread.read().await;
        let writer = bus.clone();
        let task = tokio::spawn(async move {
            writer
                .delete_user_events_filtered("alice", category, None, None, None, None)
                .await
        });
        tokio::time::timeout(StdDuration::from_secs(5), async {
            while bus.history.try_read().is_ok() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("deletion should wait for the unread lock");
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        drop(unread);
        assert_eq!(
            sequences(&bus.snapshot_for_user("alice").await),
            (0..8).collect::<Vec<_>>()
        );
        assert_eq!(bus.unread_count_for_user("alice").await, 8);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn deletion_cannot_erase_concurrent_nonmatching_publications() {
    let bus = memory_bus();
    publish_range(&bus, "alice", 0..100).await;
    let writer = bus.clone();
    let publishing = tokio::spawn(async move {
        for seq in (101..501).step_by(2) {
            writer.publish(event("alice", seq)).await;
            tokio::task::yield_now().await;
        }
    });
    assert_eq!(
        bus.delete_user_events_filtered("alice", Some("kernel"), None, None, None, None)
            .await,
        50
    );
    publishing.await.unwrap();
    let expected = (1..100)
        .step_by(2)
        .chain((101..501).step_by(2))
        .collect::<Vec<_>>();
    assert_eq!(sequences(&bus.snapshot_for_user("alice").await), expected);
    assert_eq!(bus.unread_count_for_user("alice").await, 250);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_filtered_delete_preserves_uncached_rows_and_read_flags() {
    let url = std::env::var("CYANREX_TEST_DATABASE_URL").expect("disposable database URL required");
    let admin = PgPoolOptions::new().connect(&url).await.unwrap();
    let schema = format!("event_deletion_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let connection_schema = schema.clone();
    let pool = PgPoolOptions::new()
        .after_connect(move |connection, _| {
            let schema = connection_schema.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path', $1, false)")
                    .bind(schema)
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    let mut bus = memory_bus();
    // Deliberately partial cache; SQL owns ten Alice rows and one unrelated Bob row.
    publish_range(&bus, "alice", 8..10).await;
    bus.db_pool = Some(pool.clone());
    bus.db_disabled.store(false, Ordering::Relaxed);
    bus.ensure_schema().await.unwrap();
    // A durable EventBus has its ordered persistence worker, including mutation barriers.
    let (sender, receiver) = mpsc::channel(DB_PERSIST_QUEUE_CAPACITY);
    bus.persist_sender = Some(sender);
    let worker = tokio::spawn(event_bus_db::run_persist_loop(
        bus.clone(),
        pool.clone(),
        receiver,
    ));
    sqlx::query("INSERT INTO event_records (username, timestamp, source, event_type, category, severity, color, payload, is_read)
        SELECT 'alice', now(), 'test', 'test', CASE WHEN n % 2 = 0 THEN 'kernel' ELSE 'platform' END,
        'warning', 'yellow', jsonb_build_object('seq', n), n < 5 FROM generate_series(0, 9) n")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO event_records (username, timestamp, source, event_type, category, severity, color, payload, is_read)
        VALUES ('bob', now(), 'test', 'test', 'kernel', 'warning', 'yellow', '{}', false)")
        .execute(&pool).await.unwrap();
    let deleted = bus
        .delete_user_events_filtered("alice", Some("kernel"), None, None, None, None)
        .await;
    let rows = sqlx::query("SELECT id, username, payload, is_read FROM event_records ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    let summary = rows
        .iter()
        .map(|row| {
            (
                row.get::<i64, _>("id"),
                row.get::<String, _>("username"),
                row.get::<bool, _>("is_read"),
            )
        })
        .collect::<Vec<_>>();
    let disabled = bus.db_disabled.load(Ordering::Relaxed);
    let cached = sequences(
        &bus.history.read().await["alice"]
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
    );
    let unread = bus.unread.read().await["alice"];
    worker.abort();
    let _ = worker.await;
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    assert_eq!(deleted, 5);
    assert!(!disabled);
    assert_eq!(
        summary,
        vec![
            (2, "alice".into(), true),
            (4, "alice".into(), true),
            (6, "alice".into(), false),
            (8, "alice".into(), false),
            (10, "alice".into(), false),
            (11, "bob".into(), false)
        ]
    );
    assert_eq!(cached, vec![9]);
    assert_eq!(unread, 1);
}
