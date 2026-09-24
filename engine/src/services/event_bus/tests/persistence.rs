use super::*;
use std::future::Future;

mod followup;

struct Fixture {
    bus: EventBus,
    pool: PgPool,
    receiver: Option<mpsc::Receiver<event_bus_db::PersistMessage>>,
}

impl Fixture {
    async fn drain(&mut self) {
        let mut receiver = self.receiver.take().unwrap();
        receiver.close();
        event_bus_db::run_persist_loop(self.bus.clone(), self.pool.clone(), receiver).await;
    }

    async fn count(&self, owner: &str) -> i64 {
        sqlx::query("SELECT count(*) AS count FROM event_records WHERE username = $1")
            .bind(owner)
            .fetch_one(&self.pool)
            .await
            .unwrap()
            .get("count")
    }
}

async fn with_fixture<F, Fut>(check: F)
where
    F: FnOnce(Fixture) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    assert!(
        std::env::var_os("DATABASE_URL").is_none(),
        "never use the deployment URL"
    );
    let url = std::env::var("CYANREX_TEST_DATABASE_URL")
        .expect("explicit disposable PostgreSQL URL required");
    let admin = PgPoolOptions::new().connect(&url).await.unwrap();
    let schema = format!("event_boundary_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let scoped_schema = schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .after_connect(move |connection, _| {
            let schema = scoped_schema.clone();
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
    bus.db_pool = Some(pool.clone());
    bus.db_disabled.store(false, Ordering::Relaxed);
    bus.ensure_schema().await.unwrap();
    bus.settings.write().await.insert(
        "alice".into(),
        UserEventSettings {
            max_records: 100,
            overflow_policy: EventOverflowPolicy::DropOldest,
        },
    );
    bus.settings
        .write()
        .await
        .insert("bob".into(), UserEventSettings::default());
    let (sender, receiver) = mpsc::channel(128);
    bus.persist_sender = sender;
    let fixture = Fixture {
        bus,
        pool: pool.clone(),
        receiver: Some(receiver),
    };
    let outcome = tokio::spawn(async move {
        tokio::time::timeout(StdDuration::from_secs(15), check(fixture))
            .await
            .expect("fixture deadline");
    })
    .await;
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    outcome.expect("event boundary failed; owned schema was cleaned up");
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_mark_read_waits_for_previously_queued_events() {
    with_fixture(|mut f| async move {
        publish_range(&f.bus, "alice", 0..4).await;
        publish_range(&f.bus, "bob", 0..2).await;
        let bus = f.bus.clone();
        let task = tokio::spawn(async move {
            bus.mark_all_read_for_user("alice").await;
        });
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        let waited = !task.is_finished();
        f.drain().await;
        task.await.unwrap();
        assert_eq!(f.count("alice").await, 4);
        assert_eq!(
            f.bus.unread_count_for_user("alice").await,
            0,
            "late insertion must not revive unread state"
        );
        assert_eq!(f.bus.unread_count_for_user("bob").await, 2);
        assert!(
            waited,
            "acknowledgement cannot bypass its earlier publications"
        );
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_deletion_cannot_be_undone_by_its_pending_publications() {
    with_fixture(|mut f| async move {
        publish_range(&f.bus, "alice", 0..4).await;
        publish_range(&f.bus, "bob", 0..2).await;
        let bus = f.bus.clone();
        let task = tokio::spawn(async move {
            bus.delete_user_events_filtered("alice", Some("kernel"), None, None, None, None)
                .await
        });
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        f.drain().await;
        let deleted = task.await.unwrap();
        assert_eq!(
            f.count("alice").await,
            2,
            "deleted rows must not be inserted later"
        );
        assert_eq!(deleted, 2);
        assert_eq!(f.count("bob").await, 2);
        assert_eq!(
            sequences(&f.bus.snapshot_for_user("alice").await),
            vec![1, 3]
        );
        assert_eq!(f.bus.unread_count_for_user("alice").await, 2);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_retention_update_cannot_be_overridden_by_queued_old_settings() {
    with_fixture(|mut f| async move {
        publish_range(&f.bus, "alice", 0..80).await;
        let bus = f.bus.clone();
        let task = tokio::spawn(async move {
            bus.update_settings_for_user("alice", 50, EventOverflowPolicy::DropNew)
                .await
        });
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        f.drain().await;
        task.await.unwrap().unwrap();
        assert_eq!(
            f.count("alice").await,
            50,
            "old policy must finish before the new trim"
        );
        assert_eq!(
            sequences(&f.bus.snapshot_for_user("alice").await),
            (30..80).collect::<Vec<_>>()
        );
        assert_eq!(f.bus.unread_count_for_user("alice").await, 50);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_failed_settings_update_preserves_memory_and_previous_policy() {
    with_fixture(|mut f| async move {
        // Retained memory intentionally mirrors an earlier durable batch, without queueing new work.
        f.bus
            .history
            .write()
            .await
            .insert("alice".into(), (0..80).map(|n| event("alice", n)).collect());
        f.bus.unread.write().await.insert("alice".into(), 80);
        sqlx::query("ALTER TABLE event_user_settings ADD CHECK (max_records > 50)")
            .execute(&f.pool)
            .await
            .unwrap();
        let bus = f.bus.clone();
        let task = tokio::spawn(async move {
            bus.update_settings_for_user("alice", 50, EventOverflowPolicy::DropNew)
                .await
        });
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        f.drain().await;
        assert!(task.await.unwrap().is_err());
        assert_eq!(f.bus.settings_for_user("alice").await.max_records, 100);
        assert_eq!(f.bus.history.read().await["alice"].len(), 80);
        assert_eq!(f.bus.unread.read().await["alice"], 80);
        assert!(f.bus.active_pool().is_some());
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_deletion_invalidates_drop_new_capacity_cache() {
    with_fixture(|mut f| async move {
        f.bus.update_drop_new_count("alice", 50).await;
        let bus = f.bus.clone();
        let task = tokio::spawn(async move {
            bus.delete_user_events_filtered("alice", None, None, None, None, None)
                .await
        });
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        f.drain().await;
        task.await.unwrap();
        assert_eq!(
            f.bus.cached_drop_new_count("alice").await,
            None,
            "freed capacity must be usable immediately"
        );
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_publish_batch_really_persists_json_without_fallback() {
    with_fixture(|mut f| async move {
        let mut item = event("alice", 7);
        item.payload = json!({"text": "引号 \" and \\ and newline\n", "nested": [1, true, null]});
        f.bus.publish(item.clone()).await;
        f.drain().await;
        assert_eq!(
            f.count("alice").await,
            1,
            "memory fallback cannot stand in for persistence"
        );
        assert!(f.bus.active_pool().is_some());
        assert_eq!(
            f.bus.snapshot_for_user("alice").await[0].payload,
            item.payload
        );
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_replacement_really_persists_and_marks_only_its_owner_read() {
    with_fixture(|mut f| async move {
        let bus = f.bus.clone();
        let task = tokio::spawn(async move {
            bus.replace_user_events("alice", vec![event("alice", 7)])
                .await;
        });
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        f.drain().await;
        task.await.unwrap();
        assert_eq!(f.count("alice").await, 1);
        assert!(f.bus.active_pool().is_some());
        assert_eq!(f.bus.unread_count_for_user("alice").await, 0);
    })
    .await;
}
