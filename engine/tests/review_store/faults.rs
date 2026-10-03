use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_reviews_install_only_empty_namespaces_and_pin_scope() {
    let f = Fixture::new().await;
    let reference = ReviewRef {
        workspace: scope(),
        review_id: id(),
        revision: 1.try_into().unwrap(),
    };
    assert!(f.store.read(reference, reviewer()).await.is_err());
    f.sql("CREATE TABLE unrelated (value TEXT)").await;
    assert_eq!(
        f.store.install_empty_namespace().await,
        Err(ReviewStoreError::NamespaceNotEmpty)
    );
    f.sql("DROP TABLE unrelated").await;
    f.store.install_empty_namespace().await.unwrap();
    assert_eq!(
        f.store.install_empty_namespace().await,
        Err(ReviewStoreError::NamespaceNotEmpty)
    );
    let foreign = WorkspaceRef {
        workspace_id: id(),
        ..scope()
    };
    let wrong = ReviewStore::new(f.pool.clone(), foreign);
    assert_eq!(
        wrong
            .read(
                ReviewRef {
                    workspace: foreign,
                    ..reference
                },
                reviewer()
            )
            .await,
        Err(ReviewStoreError::ScopeMismatch)
    );
    assert_eq!(
        ReviewStore::new(f.admin.clone(), scope())
            .install_empty_namespace()
            .await,
        Err(ReviewStoreError::UnsupportedSchema)
    );
    f.sql("UPDATE collaboration_review_schema SET version = 99")
        .await;
    assert_eq!(
        f.store.read(reference, reviewer()).await,
        Err(ReviewStoreError::UnsupportedSchema)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_reviews_suppressed_events_roll_back_creation_and_amendment() {
    let f = Fixture::ready().await;
    f.sql("CREATE FUNCTION suppress_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NULL; END $$").await;
    f.sql("CREATE TRIGGER suppress_event BEFORE INSERT ON collaboration_review_outbox FOR EACH ROW EXECUTE FUNCTION suppress_event()").await;
    assert!(f
        .store
        .create(id(), reviewer(), draft(artifact()))
        .await
        .is_err());
    assert_eq!(f.count("collaboration_reviews").await, 0);
    assert_eq!(f.count("collaboration_review_revisions").await, 0);
    f.sql("DROP TRIGGER suppress_event ON collaboration_review_outbox")
        .await;
    let first = f
        .store
        .create(id(), reviewer(), draft(artifact()))
        .await
        .unwrap();
    f.sql("CREATE TRIGGER suppress_event BEFORE INSERT ON collaboration_review_outbox FOR EACH ROW EXECUTE FUNCTION suppress_event()").await;
    assert!(f
        .store
        .revise(
            first.reference,
            reviewer(),
            edit(HumanReviewVerdict::Commented, "Updated")
        )
        .await
        .is_err());
    assert_eq!(
        f.store.read(first.reference, reviewer()).await.unwrap(),
        Some(first)
    );
    assert_eq!(f.count("collaboration_review_revisions").await, 1);
    assert_eq!(f.count("collaboration_review_outbox").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_reviews_deferred_commit_failure_returns_no_success() {
    let f = Fixture::ready().await;
    f.sql("CREATE FUNCTION fail_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic commit failure'; END $$").await;
    f.sql("CREATE CONSTRAINT TRIGGER fail_commit AFTER INSERT ON collaboration_review_outbox DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_commit()").await;
    assert_eq!(
        f.store.create(id(), reviewer(), draft(artifact())).await,
        Err(ReviewStoreError::StorageUnavailable)
    );
    assert_eq!(f.count("collaboration_reviews").await, 0);
    assert_eq!(f.count("collaboration_review_revisions").await, 0);
    assert_eq!(f.count("collaboration_review_outbox").await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_reviews_cancelled_write_before_commit_has_no_partial_judgment() {
    let f = Fixture::ready().await;
    let first = f
        .store
        .create(id(), reviewer(), draft(artifact()))
        .await
        .unwrap();
    let blocker = f.pause_events().await;
    let store = f.store.clone();
    let reference = first.reference;
    let pending = tokio::spawn(async move {
        store
            .revise(
                reference,
                reviewer(),
                edit(HumanReviewVerdict::Commented, "Cancelled edit"),
            )
            .await
    });
    f.wait_blocked().await;
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    blocker.rollback().await.unwrap();
    // A row lock is a barrier for the aborted transaction's cleanup, not a production retry recipe.
    let mut barrier = f.pool.begin().await.unwrap();
    query("SELECT review_id FROM collaboration_reviews FOR UPDATE")
        .fetch_all(&mut *barrier)
        .await
        .unwrap();
    barrier.commit().await.unwrap();
    assert_eq!(
        f.store.read(reference, reviewer()).await.unwrap(),
        Some(first)
    );
    assert_eq!(f.count("collaboration_review_revisions").await, 1);
    assert_eq!(f.count("collaboration_review_outbox").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_reviews_readers_see_committed_history_during_amendment() {
    let f = Fixture::ready().await;
    let first = f
        .store
        .create(id(), reviewer(), draft(artifact()))
        .await
        .unwrap();
    let blocker = f.pause_events().await;
    let store = f.store.clone();
    let reference = first.reference;
    let pending = tokio::spawn(async move {
        store
            .revise(
                reference,
                reviewer(),
                edit(HumanReviewVerdict::Commented, "Next"),
            )
            .await
    });
    f.wait_blocked().await;
    let next = ReviewRef {
        revision: 2.try_into().unwrap(),
        ..reference
    };
    assert_eq!(
        f.store.read(reference, reviewer()).await.unwrap(),
        Some(first)
    );
    assert_eq!(f.store.read(next, reviewer()).await.unwrap(), None);
    blocker.rollback().await.unwrap();
    let second = pending.await.unwrap().unwrap();
    assert_eq!(f.store.read(next, reviewer()).await.unwrap(), Some(second));
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_reviews_post_write_scope_or_event_tampering_rolls_back() {
    let f = Fixture::ready().await;
    f.sql("CREATE FUNCTION change_scope() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        UPDATE collaboration_review_schema SET workspace_id = '267c6b0f-182c-4980-a925-cdc967327c57'; RETURN NEW; END $$").await;
    f.sql("CREATE TRIGGER change_scope BEFORE INSERT ON collaboration_review_outbox FOR EACH ROW EXECUTE FUNCTION change_scope()").await;
    assert_eq!(
        f.store.create(id(), reviewer(), draft(artifact())).await,
        Err(ReviewStoreError::ScopeMismatch)
    );
    assert_eq!(f.count("collaboration_reviews").await, 0);
    f.sql("DROP TRIGGER change_scope ON collaboration_review_outbox")
        .await;
    f.sql("CREATE FUNCTION corrupt_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN NEW.snapshot = '{}'; RETURN NEW; END $$").await;
    f.sql("CREATE TRIGGER corrupt_event BEFORE INSERT ON collaboration_review_outbox FOR EACH ROW EXECUTE FUNCTION corrupt_event()").await;
    assert_eq!(
        f.store.create(id(), reviewer(), draft(artifact())).await,
        Err(ReviewStoreError::InvalidRecord)
    );
    assert_eq!(f.count("collaboration_review_revisions").await, 0);
    assert_eq!(f.count("collaboration_review_outbox").await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_reviews_missing_events_rls_and_temporary_shadows_fail_closed() {
    let f = Fixture::ready().await;
    let first = f
        .store
        .create(id(), reviewer(), draft(artifact()))
        .await
        .unwrap();
    f.sql("ALTER TABLE collaboration_reviews ENABLE ROW LEVEL SECURITY")
        .await;
    assert_eq!(
        f.store.read(first.reference, reviewer()).await,
        Err(ReviewStoreError::UnsupportedSchema)
    );
    f.sql("ALTER TABLE collaboration_reviews DISABLE ROW LEVEL SECURITY")
        .await;
    let schema = f.schema.clone();
    let pool = PgPoolOptions::new().max_connections(1).after_connect(move |connection, _| {
        let schema = schema.clone();
        Box::pin(async move {
            query("SELECT set_config('search_path', $1, false)").bind(schema).execute(&mut *connection).await?;
            query("CREATE TEMP TABLE collaboration_reviews (LIKE collaboration_reviews INCLUDING ALL)").execute(connection).await?;
            Ok(())
        })
    }).connect(&std::env::var("CYANREX_TEST_DATABASE_URL").unwrap()).await.unwrap();
    assert_eq!(
        ReviewStore::new(pool.clone(), scope())
            .read(first.reference, reviewer())
            .await,
        Err(ReviewStoreError::UnsupportedSchema)
    );
    pool.close().await;
    f.sql("DELETE FROM collaboration_review_outbox").await;
    assert_eq!(
        f.store.read(first.reference, reviewer()).await,
        Err(ReviewStoreError::InvalidRecord)
    );
    assert_eq!(
        f.store
            .revise(
                first.reference,
                reviewer(),
                edit(HumanReviewVerdict::Approved, "")
            )
            .await,
        Err(ReviewStoreError::InvalidRecord)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_reviews_rewound_heads_cannot_hide_later_judgments() {
    let f = Fixture::ready().await;
    let first = f
        .store
        .create(id(), reviewer(), draft(artifact()))
        .await
        .unwrap();
    f.store
        .revise(
            first.reference,
            reviewer(),
            edit(HumanReviewVerdict::ChangesRequested, "Not approved now"),
        )
        .await
        .unwrap();
    f.sql("UPDATE collaboration_reviews SET revision = 1").await;
    assert_eq!(
        f.store.read(first.reference, reviewer()).await,
        Err(ReviewStoreError::InvalidRecord)
    );
    assert_eq!(
        f.store
            .revise(
                first.reference,
                reviewer(),
                edit(HumanReviewVerdict::Approved, "")
            )
            .await,
        Err(ReviewStoreError::InvalidRecord)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_reviews_payload_bounds_and_changed_contracts_fail_closed() {
    let f = Fixture::ready().await;
    let first = f
        .store
        .create(id(), reviewer(), draft(artifact()))
        .await
        .unwrap();
    f.sql("ALTER TABLE collaboration_review_revisions DROP CONSTRAINT collaboration_review_revisions_snapshot_check").await;
    f.sql("UPDATE collaboration_review_revisions SET snapshot = repeat('x', 65537)")
        .await;
    assert_eq!(
        f.store.read(first.reference, reviewer()).await,
        Err(ReviewStoreError::InvalidRecord)
    );
    query("UPDATE collaboration_review_revisions SET snapshot = $1")
        .bind(serde_json::to_string(&first).unwrap())
        .execute(&f.pool)
        .await
        .unwrap();
    f.sql("UPDATE collaboration_review_revisions SET snapshot = jsonb_set(snapshot::jsonb, '{schema_version}', '99')::text").await;
    assert_eq!(
        f.store.read(first.reference, reviewer()).await,
        Err(ReviewStoreError::InvalidRecord)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_reviews_metadata_lock_timeout_cannot_publish() {
    let f = Fixture::ready().await;
    let mut blocker = f.pool.begin().await.unwrap();
    query("SELECT version FROM collaboration_review_schema FOR UPDATE")
        .fetch_all(&mut *blocker)
        .await
        .unwrap();
    assert_eq!(
        f.store.create(id(), reviewer(), draft(artifact())).await,
        Err(ReviewStoreError::StorageUnavailable)
    );
    blocker.rollback().await.unwrap();
    assert_eq!(f.count("collaboration_reviews").await, 0);
    assert_eq!(f.count("collaboration_review_outbox").await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_reviews_revision_overflow_does_not_publish_an_amendment() {
    let f = Fixture::ready().await;
    let mut record = f
        .store
        .create(id(), reviewer(), draft(artifact()))
        .await
        .unwrap();
    let max = 9_007_199_254_740_991_u64;
    record.reference.revision = max.try_into().unwrap();
    record.supersedes = Some(ReviewRef {
        revision: (max - 1).try_into().unwrap(),
        ..record.reference
    });
    let raw = serde_json::to_string(&record).unwrap();
    f.sql("DELETE FROM collaboration_review_outbox").await;
    query("UPDATE collaboration_review_revisions SET revision = $1, snapshot = $2")
        .bind(max as i64)
        .bind(&raw)
        .execute(&f.pool)
        .await
        .unwrap();
    query("UPDATE collaboration_reviews SET revision = $1")
        .bind(max as i64)
        .execute(&f.pool)
        .await
        .unwrap();
    query("INSERT INTO collaboration_review_outbox (event_id, review_id, revision, actor_id, event_type, snapshot) VALUES ($1, $2, $3, $4, 'cyanrex.review.amended', $5)")
        .bind(uuid::Uuid::new_v4()).bind(record.reference.review_id.as_uuid()).bind(max as i64)
        .bind(reviewer().principal_id.as_uuid()).bind(raw).execute(&f.pool).await.unwrap();
    // Head validation is not full-history reconciliation; this synthetic head reaches overflow.
    assert_eq!(
        f.store.read(record.reference, reviewer()).await.unwrap(),
        Some(record.clone())
    );
    assert_eq!(
        f.store
            .revise(
                record.reference,
                reviewer(),
                edit(HumanReviewVerdict::Approved, "")
            )
            .await,
        Err(ReviewStoreError::InvalidRecord)
    );
    assert_eq!(f.count("collaboration_review_revisions").await, 1);
    assert_eq!(f.count("collaboration_review_outbox").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_reviews_historical_reads_validate_events_and_unchanged_subject() {
    let f = Fixture::ready().await;
    let first = f
        .store
        .create(id(), reviewer(), draft(artifact()))
        .await
        .unwrap();
    let second = f
        .store
        .revise(
            first.reference,
            reviewer(),
            edit(HumanReviewVerdict::Commented, "New comment"),
        )
        .await
        .unwrap();
    let mut altered = first.clone();
    altered.targets = vec![artifact()];
    let raw = serde_json::to_string(&altered).unwrap();
    for table in [
        "collaboration_review_revisions",
        "collaboration_review_outbox",
    ] {
        query(&format!(
            "UPDATE {table} SET snapshot = $1 WHERE revision = 1"
        ))
        .bind(&raw)
        .execute(&f.pool)
        .await
        .unwrap();
    }
    assert_eq!(
        f.store.read(first.reference, reviewer()).await,
        Err(ReviewStoreError::InvalidRecord)
    );
    assert_eq!(
        f.store.read(second.reference, reviewer()).await.unwrap(),
        Some(second)
    );
    query("UPDATE collaboration_review_revisions SET snapshot = $1 WHERE revision = 1")
        .bind(serde_json::to_string(&first).unwrap())
        .execute(&f.pool)
        .await
        .unwrap();
    f.sql("DELETE FROM collaboration_review_outbox WHERE revision = 1")
        .await;
    assert_eq!(
        f.store.read(first.reference, reviewer()).await,
        Err(ReviewStoreError::InvalidRecord)
    );
    f.cleanup().await;
}
