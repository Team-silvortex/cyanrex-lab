use super::*;

impl Fixture {
    pub(super) fn reopen(&mut self) {
        assert!(self.receiver.is_none());
        let (sender, receiver) = mpsc::channel(128);
        self.bus.persist_sender = Some(sender);
        self.receiver = Some(receiver);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_blocked_owner_mutation_does_not_hold_other_owners_publications() {
    with_fixture(|mut f| async move {
        let mut blocker = f.pool.begin().await.unwrap();
        sqlx::query("LOCK TABLE event_records IN SHARE MODE")
            .execute(&mut *blocker)
            .await
            .unwrap();
        let bus = f.bus.clone();
        let task = tokio::spawn(async move {
            bus.update_settings_for_user("alice", 50, EventOverflowPolicy::DropNew)
                .await
        });
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        f.drain().await;
        f.reopen();
        let published = tokio::time::timeout(
            StdDuration::from_millis(100),
            f.bus.publish(event("bob", 99)),
        )
        .await;
        blocker.rollback().await.unwrap();
        task.await.unwrap().unwrap();
        f.drain().await;
        assert!(
            published.is_ok(),
            "one owner cannot hold unrelated publication admission while waiting for SQL"
        );
        assert_eq!(f.count("bob").await, 1);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_equal_timestamp_latest_page_uses_publication_order() {
    with_fixture(|mut f| async move {
        for seq in 0..4 {
            let mut item = event("alice", seq);
            item.timestamp = event("alice", 0).timestamp;
            f.bus.publish(item).await;
        }
        f.drain().await;
        let rows = f
            .bus
            .snapshot_for_user_filtered(
                "alice",
                EventQueryFilters {
                    limit: Some(1),
                    ..Default::default()
                },
            )
            .await;
        assert_eq!(sequences(&rows), vec![3]);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_equal_timestamp_retention_does_not_sort_by_event_type() {
    with_fixture(|mut f| async move {
        f.bus
            .settings
            .write()
            .await
            .get_mut("alice")
            .unwrap()
            .max_records = 50;
        for seq in 0..56 {
            let mut item = event("alice", seq);
            item.timestamp = event("alice", 0).timestamp;
            item.event_type = format!("reverse-{:03}", 100 - seq);
            f.bus.publish(item).await;
        }
        f.drain().await;
        assert_eq!(
            sequences(&f.bus.snapshot_for_user("alice").await),
            (6..56).collect::<Vec<_>>()
        );
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_trim_failure_rolls_back_settings_and_preserves_retained_rows() {
    with_fixture(|mut f| async move {
        publish_range(&f.bus, "alice", 0..80).await;
        f.drain().await;
        f.reopen();
        sqlx::query("INSERT INTO event_user_settings VALUES ('alice', 100, 'drop_oldest')").execute(&f.pool).await.unwrap();
        sqlx::query("CREATE FUNCTION reject_trim() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic trim failure'; END $$").execute(&f.pool).await.unwrap();
        sqlx::query("CREATE TRIGGER reject_trim BEFORE DELETE ON event_records FOR EACH STATEMENT EXECUTE FUNCTION reject_trim()").execute(&f.pool).await.unwrap();
        let bus = f.bus.clone();
        let task = tokio::spawn(async move { bus.update_settings_for_user("alice", 50, EventOverflowPolicy::DropNew).await });
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        f.drain().await;
        assert!(task.await.unwrap().is_err());
        let maximum: i64 = sqlx::query("SELECT max_records FROM event_user_settings WHERE username = 'alice'").fetch_one(&f.pool).await.unwrap().get("max_records");
        assert_eq!(maximum, 100);
        assert_eq!(f.bus.settings_for_user("alice").await.max_records, 100);
        assert_eq!(f.count("alice").await, 80);
        assert_eq!(f.bus.history.read().await["alice"].len(), 80);
        assert!(f.bus.active_pool().is_some());
    }).await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_failed_replacement_does_not_lose_previous_durable_rows() {
    with_fixture(|mut f| async move {
        publish_range(&f.bus, "alice", 0..4).await;
        f.drain().await;
        f.reopen();
        sqlx::query("ALTER TABLE event_records ADD CHECK (event_type <> 'reject')")
            .execute(&f.pool)
            .await
            .unwrap();
        let bus = f.bus.clone();
        let mut rejected = event("alice", 99);
        rejected.event_type = "reject".into();
        let task = tokio::spawn(async move {
            bus.replace_user_events("alice", vec![rejected]).await;
        });
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        f.drain().await;
        task.await.unwrap();
        assert_eq!(
            f.count("alice").await,
            4,
            "delete and replacement must share one transaction"
        );
        assert_eq!(
            sequences(&f.bus.snapshot_for_user("alice").await),
            vec![0, 1, 2, 3]
        );
        assert!(
            f.bus.active_pool().is_none(),
            "existing fallback policy remains explicit"
        );
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_cancelled_admitted_settings_finish_before_later_publications() {
    with_fixture(|mut f| async move {
        publish_range(&f.bus, "alice", 0..80).await;
        f.drain().await;
        f.reopen();
        let mut blocker = f.pool.begin().await.unwrap();
        sqlx::query("LOCK TABLE event_records IN SHARE MODE").execute(&mut *blocker).await.unwrap();
        let bus = f.bus.clone();
        let task = tokio::spawn(async move { bus.update_settings_for_user("alice", 50, EventOverflowPolicy::DropNew).await });
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        f.drain().await;
        tokio::time::timeout(StdDuration::from_secs(3), async {
            loop {
                let waiting: i64 = sqlx::query("SELECT count(*) AS count FROM pg_locks WHERE relation = 'event_records'::regclass AND NOT granted").fetch_one(&f.pool).await.unwrap().get("count");
                if waiting > 0 { break; }
                tokio::task::yield_now().await;
            }
        }).await.expect("settings SQL must reach the controlled table lock");
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        let publisher = f.bus.clone();
        let later = tokio::spawn(async move { publisher.publish(event("alice", 99)).await; });
        tokio::task::yield_now().await;
        assert!(!later.is_finished(), "later publication must retain mutation admission ordering");
        blocker.rollback().await.unwrap();
        later.await.unwrap();
        let completed = f.bus.mutation_admission("alice").await;
        assert_eq!(f.bus.settings_for_user("alice").await.max_records, 50);
        assert_eq!(f.bus.history.read().await["alice"].len(), 50);
        assert_eq!(f.count("alice").await, 50);
        drop(completed);
    }).await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_cold_settings_read_cannot_overwrite_a_newer_cache_value() {
    with_fixture(|mut f| async move {
        sqlx::query("INSERT INTO event_user_settings VALUES ('cold', 100, 'drop_oldest')")
            .execute(&f.pool)
            .await
            .unwrap();
        let mut connections = Vec::new();
        for _ in 0..5 {
            connections.push(f.pool.acquire().await.unwrap());
        }
        let bus = f.bus.clone();
        let task = tokio::spawn(async move { bus.settings_for_user("cold").await });
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        assert!(!task.is_finished());
        f.bus.settings.write().await.insert(
            "cold".into(),
            UserEventSettings {
                max_records: 50,
                overflow_policy: EventOverflowPolicy::DropNew,
            },
        );
        drop(connections);
        assert_eq!(task.await.unwrap().max_records, 50);
        assert_eq!(f.bus.settings_for_user("cold").await.max_records, 50);
        f.drain().await;
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_drop_new_can_use_capacity_freed_by_deletion_immediately() {
    with_fixture(|mut f| async move {
        f.bus.settings.write().await.insert(
            "alice".into(),
            UserEventSettings {
                max_records: 50,
                overflow_policy: EventOverflowPolicy::DropNew,
            },
        );
        publish_range(&f.bus, "alice", 0..50).await;
        f.drain().await;
        f.reopen();
        let bus = f.bus.clone();
        let task = tokio::spawn(async move {
            bus.delete_user_events_filtered("alice", Some("kernel"), None, None, None, None)
                .await
        });
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        f.drain().await;
        assert_eq!(task.await.unwrap(), 25);
        f.reopen();
        f.bus.publish(event("alice", 99)).await;
        f.drain().await;
        assert_eq!(f.count("alice").await, 26);
        assert_eq!(
            f.bus
                .snapshot_for_user("alice")
                .await
                .last()
                .unwrap()
                .payload["seq"],
            99
        );
        assert!(f.bus.active_pool().is_some());
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_replacement_cannot_be_undone_by_queued_old_events() {
    with_fixture(|mut f| async move {
        publish_range(&f.bus, "alice", 0..4).await;
        publish_range(&f.bus, "bob", 0..2).await;
        let bus = f.bus.clone();
        let task = tokio::spawn(async move {
            bus.replace_user_events("alice", vec![event("alice", 99)])
                .await;
        });
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        f.drain().await;
        task.await.unwrap();
        assert_eq!(sequences(&f.bus.snapshot_for_user("alice").await), vec![99]);
        assert_eq!(f.bus.unread_count_for_user("alice").await, 0);
        assert_eq!(f.bus.unread_count_for_user("bob").await, 2);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_failed_batch_releases_its_barrier_into_explicit_memory_fallback() {
    with_fixture(|mut f| async move {
        sqlx::query("ALTER TABLE event_records ADD CHECK (event_type <> 'sequence')")
            .execute(&f.pool)
            .await
            .unwrap();
        f.bus.publish(event("alice", 0)).await;
        let bus = f.bus.clone();
        let task = tokio::spawn(async move {
            bus.mark_all_read_for_user("alice").await;
        });
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        f.drain().await;
        task.await.unwrap();
        assert!(f.bus.active_pool().is_none());
        assert_eq!(f.count("alice").await, 0);
        assert_eq!(f.bus.snapshot_for_user("alice").await.len(), 1);
        assert_eq!(f.bus.unread_count_for_user("alice").await, 0);
    })
    .await;
}
