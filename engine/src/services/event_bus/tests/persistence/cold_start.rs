use super::*;

impl Fixture {
    async fn cold_drop_new(&mut self, existing: usize) {
        publish_range(&self.bus, "alice", 0..existing).await;
        self.drain().await;
        sqlx::query("INSERT INTO event_user_settings VALUES ('alice', 50, 'drop_new')")
            .execute(&self.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE event_records SET is_read = true")
            .execute(&self.pool)
            .await
            .unwrap();
        self.restart().await;
    }

    async fn restart(&mut self) {
        assert!(self.receiver.is_none());
        // Fresh EventBus state against the same isolated schema models a cold cache;
        // it is not a process-crash or lost-commit simulation.
        self.bus = memory_bus();
        self.bus.db_pool = Some(self.pool.clone());
        self.bus.db_disabled.store(false, Ordering::Relaxed);
        self.bus.ensure_schema().await.unwrap();
        self.reopen();
        assert!(self.bus.history.read().await.is_empty());
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_cold_full_drop_new_rejects_before_memory_unread_and_broadcast() {
    with_fixture(|mut f| async move {
        f.cold_drop_new(50).await;
        let mut global = f.bus.subscribe();
        let mut owner = f.bus.subscribe_for_user("alice");
        publish_range(&f.bus, "alice", 100..103).await;
        let admitted = f
            .bus
            .history
            .read()
            .await
            .get("alice")
            .map_or(0, VecDeque::len);
        let unread = f.bus.unread.read().await.get("alice").copied().unwrap_or(0);
        let queued = f.receiver.as_ref().unwrap().len();
        f.drain().await;
        assert_eq!(f.count("alice").await, 50);
        assert!(f.bus.active_pool().is_some());
        assert_eq!(
            admitted, 0,
            "full durable history cannot admit phantom live events"
        );
        assert_eq!(unread, 0);
        assert_eq!(queued, 0);
        assert!(matches!(global.try_recv(), Err(TryRecvError::Empty)));
        assert!(
            tokio::time::timeout(StdDuration::from_millis(20), owner.recv())
                .await
                .is_err()
        );
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_cold_partial_drop_new_counts_pending_publications_before_flush() {
    with_fixture(|mut f| async move {
        f.cold_drop_new(48).await;
        let mut global = f.bus.subscribe();
        publish_range(&f.bus, "alice", 100..110).await;
        // Writer SQL counts expire after one second; admitted pending work must not.
        tokio::time::sleep(StdDuration::from_millis(1_050)).await;
        f.bus.publish(event("alice", 110)).await;
        let admitted = sequences(
            &f.bus
                .snapshot_from_history_filtered("alice", EventQueryFilters::default())
                .await,
        );
        let queued = f.receiver.as_ref().unwrap().len();
        f.drain().await;
        assert_eq!(f.count("alice").await, 50);
        assert_eq!(
            admitted,
            vec![100, 101],
            "pending inserts also consume capacity"
        );
        assert_eq!(queued, 2);
        assert_eq!(f.bus.unread_count_for_user("alice").await, 2);
        for seq in 100..102 {
            assert_eq!(global.try_recv().unwrap().payload["seq"], seq);
        }
        assert!(matches!(global.try_recv(), Err(TryRecvError::Empty)));
        assert!(f.bus.active_pool().is_some());
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_cold_drop_new_concurrent_clones_share_admission_and_owner_isolation() {
    with_fixture(|mut f| async move {
        f.cold_drop_new(40).await;
        let mut workers = Vec::new();
        for seq in 100..130 {
            let bus = f.bus.clone();
            workers.push(tokio::spawn(async move {
                bus.publish(event("alice", seq)).await;
            }));
        }
        for worker in workers {
            worker.await.unwrap();
        }
        publish_range(&f.bus, "bob", 0..2).await;
        let admitted = f.bus.history.read().await["alice"].len();
        f.drain().await;
        assert_eq!(admitted, 10);
        assert_eq!(f.count("alice").await, 50);
        assert_eq!(f.count("bob").await, 2);
        assert_eq!(f.bus.unread_count_for_user("alice").await, 10);
        assert_eq!(f.bus.unread_count_for_user("bob").await, 2);
        assert!(f.bus.active_pool().is_some());
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_writer_drains_multiple_batches_after_last_producer_drops() {
    with_fixture(|mut f| async move {
        publish_range(&f.bus, "alice", 0..125).await;
        publish_range(&f.bus, "bob", 0..2).await;
        let receiver = f.receiver.take().unwrap();
        let worker = tokio::spawn(event_bus_db::run_persist_loop(f.bus.clone(), f.pool.clone(), receiver));
        let state = Arc::downgrade(&f.bus.history);
        drop(f.bus);
        super::super::lifecycle::join_writer(worker).await;
        assert!(state.upgrade().is_none());
        let counts: Vec<(String, i64)> = sqlx::query("SELECT username, count(*) AS count FROM event_records GROUP BY username ORDER BY username")
            .fetch_all(&f.pool).await.unwrap().iter().map(|row| (row.get("username"), row.get("count"))).collect();
        assert_eq!(counts, vec![("alice".into(), 100), ("bob".into(), 2)]);
        let newest: i64 = sqlx::query("SELECT (payload->>'seq')::bigint AS seq FROM event_records WHERE username = 'alice' ORDER BY timestamp DESC, id DESC LIMIT 1")
            .fetch_one(&f.pool).await.unwrap().get("seq");
        assert_eq!(newest, 124, "last partial batch must also reach SQL");
    }).await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_cold_drop_new_mutations_rebase_durable_capacity() {
    with_fixture(|mut f| async move {
        f.cold_drop_new(50).await;
        f.bus.publish(event("alice", 99)).await;
        let bus = f.bus.clone();
        let task = tokio::spawn(async move {
            bus.delete_user_events_filtered("alice", Some("kernel"), None, None, None, None)
                .await
        });
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        f.drain().await;
        assert_eq!(task.await.unwrap(), 25);
        f.reopen();
        publish_range(&f.bus, "alice", 100..130).await;
        assert_eq!(f.receiver.as_ref().unwrap().len(), 25);
        f.drain().await;
        assert_eq!(f.count("alice").await, 50);

        f.reopen();
        let bus = f.bus.clone();
        let task = tokio::spawn(async move {
            bus.update_settings_for_user("alice", 75, EventOverflowPolicy::DropNew)
                .await
        });
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        f.drain().await;
        task.await.unwrap().unwrap();
        f.reopen();
        publish_range(&f.bus, "alice", 200..230).await;
        assert_eq!(f.receiver.as_ref().unwrap().len(), 25);
        f.drain().await;
        assert_eq!(f.count("alice").await, 75);

        f.reopen();
        let bus = f.bus.clone();
        let task = tokio::spawn(async move {
            bus.replace_user_events("alice", vec![event("alice", 300)])
                .await;
        });
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        f.drain().await;
        task.await.unwrap();
        f.reopen();
        publish_range(&f.bus, "alice", 301..381).await;
        assert_eq!(f.receiver.as_ref().unwrap().len(), 74);
        f.drain().await;
        assert_eq!(f.count("alice").await, 75);
        assert_eq!(f.bus.unread_count_for_user("alice").await, 74);
        assert!(f.bus.active_pool().is_some());
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_cancelled_cold_drop_new_admission_does_not_lose_capacity() {
    with_fixture(|mut f| async move {
        f.cold_drop_new(48).await;
        f.bus.settings_for_user("alice").await;
        let mut connections = Vec::new();
        for _ in 0..5 {
            connections.push(f.pool.acquire().await.unwrap());
        }
        let bus = f.bus.clone();
        let query = tokio::spawn(async move {
            bus.publish(event("alice", 98)).await;
        });
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        let waited_for_capacity = !query.is_finished();
        query.abort();
        let _ = query.await;
        drop(connections);
        assert!(
            waited_for_capacity,
            "cold admission must consult durable capacity before publishing"
        );

        let unread = f.bus.unread.read().await;
        let bus = f.bus.clone();
        let publication = tokio::spawn(async move {
            bus.publish(event("alice", 99)).await;
        });
        tokio::time::timeout(StdDuration::from_secs(2), async {
            while f.bus.history.try_write().is_ok() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("publication must reach the held unread lock");
        publication.abort();
        assert!(publication.await.unwrap_err().is_cancelled());
        drop(unread);
        publish_range(&f.bus, "alice", 100..103).await;
        assert_eq!(
            f.receiver.as_ref().unwrap().len(),
            2,
            "cancellation cannot consume a slot"
        );
        f.drain().await;
        assert_eq!(f.count("alice").await, 50);
        assert_eq!(
            sequences(
                &f.bus
                    .snapshot_from_history_filtered("alice", EventQueryFilters::default())
                    .await
            ),
            vec![100, 101]
        );
        assert!(f.bus.active_pool().is_some());
    })
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_cold_drop_new_count_failure_latches_memory_until_restart() {
    with_fixture(|mut f| async move {
        f.cold_drop_new(50).await;
        sqlx::query("ALTER TABLE event_records RENAME TO unavailable_records")
            .execute(&f.pool)
            .await
            .unwrap();
        publish_range(&f.bus, "alice", 100..102).await;
        assert!(
            f.bus.active_pool().is_none(),
            "capacity read failure must latch existing volatile fallback"
        );
        assert_eq!(
            sequences(&f.bus.snapshot_for_user("alice").await),
            vec![100, 101]
        );
        sqlx::query("ALTER TABLE unavailable_records RENAME TO event_records")
            .execute(&f.pool)
            .await
            .unwrap();
        assert!(
            f.bus.active_pool().is_none(),
            "restoring SQL is not automatic reconciliation"
        );
        f.drain().await;
        f.restart().await;
        f.bus.publish(event("alice", 102)).await;
        f.drain().await;
        assert_eq!(f.count("alice").await, 50);
        assert_eq!(
            sequences(&f.bus.snapshot_for_user("alice").await),
            (0..50).collect::<Vec<_>>()
        );
        assert!(f
            .bus
            .history
            .read()
            .await
            .get("alice")
            .is_none_or(VecDeque::is_empty));
        assert!(f.bus.active_pool().is_some());
    })
    .await;
}
