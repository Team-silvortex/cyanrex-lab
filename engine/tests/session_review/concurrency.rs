use super::*;
use std::time::Duration;

async fn backend(tx: &mut sqlx_core::transaction::Transaction<'_, sqlx_postgres::Postgres>) -> i32 {
    query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
        .get("pid")
}

async fn blocked_by(f: &Fixture, blocker: i32) -> i32 {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let row = query(
                "SELECT pid FROM pg_stat_activity WHERE datname = current_database()
                AND $1 = ANY(pg_blocking_pids(pid)) ORDER BY pid LIMIT 1",
            )
            .bind(blocker)
            .fetch_optional(&f.auth.base.admin)
            .await
            .unwrap();
            if let Some(row) = row {
                return row.get("pid");
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("expected backend did not reach its exact synthetic blocker")
}

async fn wait_expired(f: &Fixture) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let expired: bool = query(&format!("SELECT clock_timestamp() >= expires_at AS expired FROM {}.sessions WHERE token = $1", f.auth.base.schema))
                .bind(source_fixture::token_hash(&f.auth.learner_token)).fetch_one(&f.auth.base.admin).await.unwrap().get("expired");
            if expired { return; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("synthetic Session did not expire before the database lock deadline");
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_reviews_reader_holds_review_lock_while_waiting_for_artifacts() {
    let f = Fixture::ready().await;
    let artifact = f
        .create_artifact(&f.auth.learner_token, b"Locked review subject")
        .await;
    let first = f
        .create(
            &f.auth.learner_token,
            vec![artifact.reference.clone()],
            vec![],
        )
        .await;
    let mut holder = f.artifact_pool.begin().await.unwrap();
    query("SELECT artifact_id FROM collaboration_artifacts WHERE artifact_id = $1 FOR UPDATE")
        .bind(artifact.reference.artifact_id.as_uuid())
        .fetch_one(&mut *holder)
        .await
        .unwrap();
    let holder_pid = backend(&mut holder).await;
    let source = f.auth.source.clone();
    let token = f.auth.learner_token.clone();
    let workspace = f.workspace.clone();
    let reference = first.reference;
    let reader = tokio::spawn(async move {
        source
            .read_session_review(&token, &workspace, reference)
            .await
    });
    let reader_pid = blocked_by(&f, holder_pid).await;
    let reviews = f.reviews.clone();
    let reviewer = first.reviewer;
    let writer = tokio::spawn(async move {
        reviews
            .revise(
                reference,
                reviewer,
                edit(HumanReviewVerdict::Commented, "Trusted later opinion"),
            )
            .await
    });
    // The raw writer must wait on the Session reader, not just on the Artifact holder.
    blocked_by(&f, reader_pid).await;
    assert!(!reader.is_finished());
    assert!(!writer.is_finished());
    holder.rollback().await.unwrap();
    assert_eq!(reader.await.unwrap().unwrap(), Some(first.clone()));
    let next = writer.await.unwrap().unwrap();
    assert_eq!(u64::from(next.reference.revision), 2);
    assert_eq!(next.supersedes, Some(first.reference));
    assert_eq!(next.targets, first.targets);
    assert_eq!(
        f.reviews.read(next.reference, next.reviewer).await.unwrap(),
        Some(next)
    );
    assert_eq!(f.count_review("collaboration_review_revisions").await, 2);
    assert_eq!(f.count_review("collaboration_review_outbox").await, 2);
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 1);
    assert_eq!(f.files(), 1);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_reviews_historical_read_waits_for_trusted_amendment() {
    let f = Fixture::ready().await;
    let artifact = f
        .create_artifact(&f.auth.learner_token, b"Exact old opinion subject")
        .await;
    let first = f
        .create(&f.auth.learner_token, vec![artifact.reference], vec![])
        .await;
    let mut holder = f.pause_review_event().await;
    let holder_pid = backend(&mut holder).await;
    let reviews = f.reviews.clone();
    let reference = first.reference;
    let reviewer = first.reviewer;
    let writer = tokio::spawn(async move {
        reviews
            .revise(
                reference,
                reviewer,
                edit(
                    HumanReviewVerdict::ChangesRequested,
                    "A newer committed judgment",
                ),
            )
            .await
    });
    let writer_pid = blocked_by(&f, holder_pid).await;
    let source = f.auth.source.clone();
    let token = f.auth.learner_token.clone();
    let workspace = f.workspace.clone();
    let reader = tokio::spawn(async move {
        source
            .read_session_review(&token, &workspace, reference)
            .await
    });
    blocked_by(&f, writer_pid).await;
    assert!(!writer.is_finished());
    assert!(!reader.is_finished());
    holder.rollback().await.unwrap();
    let next = writer.await.unwrap().unwrap();
    assert_eq!(reader.await.unwrap().unwrap(), Some(first.clone()));
    assert_eq!(next.supersedes, Some(first.reference));
    assert_eq!(
        f.read(&f.auth.learner_token, next.reference).await.unwrap(),
        Some(next)
    );
    assert_eq!(f.count_review("collaboration_reviews").await, 1);
    assert_eq!(f.count_review("collaboration_review_revisions").await, 2);
    assert_eq!(f.count_review("collaboration_review_outbox").await, 2);
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 1);
    assert_eq!(f.files(), 1);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_reviews_expiry_after_resource_and_event_waits_returns_nothing() {
    for operation in ["create", "read", "revise"] {
        let f = Fixture::ready().await;
        let artifact = f
            .create_artifact(&f.auth.learner_token, b"Do not publish expired authority")
            .await;
        let original = if operation == "create" {
            None
        } else {
            Some(
                f.create(
                    &f.auth.learner_token,
                    vec![artifact.reference.clone()],
                    vec![],
                )
                .await,
            )
        };
        let mut holder = if operation == "create" {
            f.pause_review_event().await
        } else {
            let mut holder = f.artifact_pool.begin().await.unwrap();
            query(
                "SELECT artifact_id FROM collaboration_artifacts WHERE artifact_id = $1 FOR UPDATE",
            )
            .bind(artifact.reference.artifact_id.as_uuid())
            .fetch_one(&mut *holder)
            .await
            .unwrap();
            holder
        };
        let holder_pid = backend(&mut holder).await;
        query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1")
            .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
        let source = f.auth.source.clone();
        let token = f.auth.learner_token.clone();
        let workspace = f.workspace.clone();
        let target = artifact.reference.clone();
        let reference = original.as_ref().map(|record| record.reference);
        let pending = tokio::spawn(async move {
            match operation {
                "create" => source
                    .create_session_review(
                        &token,
                        &workspace,
                        id(),
                        vec![target],
                        vec![],
                        edit(HumanReviewVerdict::Approved, ""),
                    )
                    .await
                    .map(|_| ()),
                "read" => source
                    .read_session_review(&token, &workspace, reference.unwrap())
                    .await
                    .map(|_| ()),
                _ => source
                    .revise_session_review(
                        &token,
                        &workspace,
                        reference.unwrap(),
                        edit(HumanReviewVerdict::Commented, "Expired amendment"),
                    )
                    .await
                    .map(|_| ()),
            }
        });
        blocked_by(&f, holder_pid).await;
        wait_expired(&f).await;
        assert!(!pending.is_finished());
        holder.rollback().await.unwrap();
        assert_eq!(
            pending.await.unwrap(),
            Err(SessionReviewError::Session(
                SessionCommandError::InvalidSession
            ))
        );
        f.auth.base.drain().await;
        for table in [
            "collaboration_reviews",
            "collaboration_review_revisions",
            "collaboration_review_outbox",
        ] {
            assert_eq!(f.count_review(table).await, i64::from(original.is_some()));
        }
        if let Some(record) = original {
            assert_eq!(
                f.reviews
                    .read(record.reference, record.reviewer)
                    .await
                    .unwrap(),
                Some(record)
            );
        }
        assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 1);
        assert_eq!(f.files(), 1);
        assert_eq!(
            f.artifacts
                .read(&artifact.reference, artifact.owner)
                .await
                .unwrap()
                .unwrap()
                .bytes,
            b"Do not publish expired authority"
        );
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_reviews_logout_order_preserves_single_transaction_authority() {
    for logout_first in [false, true] {
        let f = Fixture::ready().await;
        let artifact = f
            .create_artifact(&f.auth.learner_token, b"Subject persists across logout")
            .await;
        let source = f.auth.source.clone();
        let token = f.auth.learner_token.clone();
        let workspace = f.workspace.clone();
        let target = artifact.reference.clone();
        if logout_first {
            f.auth.sql("CREATE FUNCTION pause_review_logout() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
                PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 772); RETURN OLD; END $$").await;
            f.auth.sql("CREATE TRIGGER pause_review_logout BEFORE DELETE ON sessions FOR EACH ROW EXECUTE FUNCTION pause_review_logout()").await;
            let mut holder = f.auth.base.pool.begin().await.unwrap();
            query("SELECT pg_advisory_xact_lock(hashtext($1), 772)")
                .bind(&f.auth.base.schema)
                .execute(&mut *holder)
                .await
                .unwrap();
            let holder_pid = backend(&mut holder).await;
            let other = f.auth.base.reopen().await;
            let logout_token = token.clone();
            let logout = tokio::spawn(async move { other.logout(&logout_token).await });
            let logout_pid = blocked_by(&f, holder_pid).await;
            let create = tokio::spawn(async move {
                source
                    .create_session_review(
                        &token,
                        &workspace,
                        id(),
                        vec![target],
                        vec![],
                        edit(HumanReviewVerdict::Approved, ""),
                    )
                    .await
            });
            blocked_by(&f, logout_pid).await;
            assert!(!logout.is_finished());
            assert!(!create.is_finished());
            holder.rollback().await.unwrap();
            logout.await.unwrap().unwrap();
            assert_eq!(
                create.await.unwrap(),
                Err(SessionReviewError::Session(
                    SessionCommandError::InvalidSession
                ))
            );
        } else {
            let mut holder = f.pause_review_event().await;
            let holder_pid = backend(&mut holder).await;
            let create = tokio::spawn(async move {
                source
                    .create_session_review(
                        &token,
                        &workspace,
                        id(),
                        vec![target],
                        vec![],
                        edit(HumanReviewVerdict::Approved, ""),
                    )
                    .await
            });
            let create_pid = blocked_by(&f, holder_pid).await;
            let other = f.auth.base.reopen().await;
            let logout_token = f.auth.learner_token.clone();
            let logout = tokio::spawn(async move { other.logout(&logout_token).await });
            blocked_by(&f, create_pid).await;
            assert!(!create.is_finished());
            assert!(!logout.is_finished());
            holder.rollback().await.unwrap();
            let record = create.await.unwrap().unwrap();
            logout.await.unwrap().unwrap();
            assert_eq!(
                f.reviews
                    .read(record.reference, record.reviewer)
                    .await
                    .unwrap(),
                Some(record)
            );
        }
        for table in [
            "collaboration_reviews",
            "collaboration_review_revisions",
            "collaboration_review_outbox",
        ] {
            assert_eq!(f.count_review(table).await, i64::from(!logout_first));
        }
        assert_eq!(f.files(), 1);
        assert_eq!(
            f.artifacts
                .read(&artifact.reference, artifact.owner)
                .await
                .unwrap()
                .unwrap()
                .bytes,
            b"Subject persists across logout"
        );
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_reviews_cancelled_write_rolls_back_without_touching_inputs() {
    let f = Fixture::ready().await;
    let artifact = f
        .create_artifact(&f.auth.learner_token, b"Existing immutable input")
        .await;
    let mut holder = f.pause_review_event().await;
    let holder_pid = backend(&mut holder).await;
    let source = f.auth.source.clone();
    let token = f.auth.learner_token.clone();
    let workspace = f.workspace.clone();
    let target = artifact.reference.clone();
    let pending = tokio::spawn(async move {
        source
            .create_session_review(
                &token,
                &workspace,
                id(),
                vec![target],
                vec![],
                edit(HumanReviewVerdict::Approved, ""),
            )
            .await
    });
    blocked_by(&f, holder_pid).await;
    pending.abort();
    assert!(pending.await.is_err_and(|error| error.is_cancelled()));
    holder.rollback().await.unwrap();
    f.auth.base.drain().await;
    f.assert_pool_restored().await;
    for table in [
        "collaboration_reviews",
        "collaboration_review_revisions",
        "collaboration_review_outbox",
    ] {
        assert_eq!(f.count_review(table).await, 0);
    }
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 1);
    assert_eq!(f.files(), 1);
    assert_eq!(
        f.artifacts
            .read(&artifact.reference, artifact.owner)
            .await
            .unwrap()
            .unwrap()
            .bytes,
        b"Existing immutable input"
    );
    let record = f
        .create(&f.auth.learner_token, vec![artifact.reference], vec![])
        .await;
    assert_eq!(u64::from(record.reference.revision), 1);
    assert_eq!(f.count_review("collaboration_review_outbox").await, 1);
    f.cleanup().await;
}
