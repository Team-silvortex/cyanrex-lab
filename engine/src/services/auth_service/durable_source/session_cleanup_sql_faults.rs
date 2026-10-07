use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_cleanup_rejects_suppressed_deletion_and_future_reinsertion() {
    for (timing, body) in [
        ("BEFORE DELETE", "RETURN NULL;"),
        ("AFTER DELETE", "INSERT INTO sessions(token,username,expires_at,created_at,account_id) SELECT OLD.token, OLD.username,
            clock_timestamp() + INTERVAL '1 day', OLD.created_at, OLD.account_id; RETURN OLD;"),
    ] {
        let f = Fixture::ready().await;
        insert_sessions(&f, 1, 1, true).await;
        let before = f.snapshot().await;
        fault(&f, timing, body).await;
        let result = f.source.prune_expired_sessions().await;
        let reached = calls(&f).await;
        let after = f.snapshot().await;
        sql(&f, "DROP TRIGGER cleanup_fault ON sessions").await;
        let retry = f.source.prune_expired_sessions().await;
        f.cleanup().await;
        assert_eq!(reached, 1, "{timing}");
        assert_eq!(result, Err(DurableAuthError::StorageUnavailable), "{timing}");
        assert_eq!(after, before, "{timing}");
        assert_eq!(retry, Ok(1), "retry only after observing rollback and removing the synthetic fault");
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_cleanup_rolls_back_source_drift_and_selected_row_trigger_changes() {
    for which in ["path", "relation", "expiry", "created_at", "authority"] {
        let f = Fixture::ready().await;
        let shadow = Fixture::ready().await;
        insert_sessions(
            &f,
            1,
            if matches!(which, "expiry" | "created_at") {
                2
            } else {
                1
            },
            true,
        )
        .await;
        let before = f.snapshot().await;
        let shadow_before = shadow.snapshot().await;
        let oid: i64 =
            query("SELECT 'collaboration_auth_source_schema'::regclass::oid::bigint AS oid")
                .fetch_one(&f.pool)
                .await
                .unwrap()
                .get("oid");
        let body = match which {
            "path" => format!("PERFORM set_config('search_path', '{}', true); RETURN OLD;", shadow.schema),
            "relation" => "ALTER TABLE collaboration_auth_source_schema RENAME TO cleanup_prior_source;
                CREATE TABLE collaboration_auth_source_schema (LIKE cleanup_prior_source INCLUDING ALL);
                INSERT INTO collaboration_auth_source_schema SELECT * FROM cleanup_prior_source; RETURN OLD;".to_owned(),
            "expiry" => "UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '1 day'
                WHERE token <> OLD.token; RETURN OLD;".to_owned(),
            "created_at" => "UPDATE sessions SET created_at = created_at - INTERVAL '1 second'
                WHERE token <> OLD.token; RETURN OLD;".to_owned(),
            "authority" => format!("UPDATE collaboration_auth_source_schema SET authority_id = '{}'; RETURN OLD;", uuid::Uuid::new_v4()),
            _ => unreachable!(),
        };
        fault(&f, "BEFORE DELETE", &body).await;
        let result = f.source.prune_expired_sessions().await;
        let reached = calls(&f).await;
        let after = f.snapshot().await;
        let shadow_after = shadow.snapshot().await;
        let restored = query(
            "SELECT 'collaboration_auth_source_schema'::regclass::oid::bigint AS oid,
            to_regclass('cleanup_prior_source') IS NULL AS original_name",
        )
        .fetch_one(&f.pool)
        .await
        .unwrap();
        shadow.cleanup().await;
        f.cleanup().await;
        assert!(reached >= 1, "{which}: must reach the write trigger");
        assert!(result.is_err(), "{which}");
        assert_eq!(after, before, "{which}");
        assert_eq!(shadow_after, shadow_before, "{which}");
        assert_eq!(restored.get::<i64, _>("oid"), oid, "{which}");
        assert!(restored.get::<bool, _>("original_name"), "{which}");
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_cleanup_deferred_commit_failure_returns_no_confirmed_count() {
    let f = Fixture::ready().await;
    insert_sessions(&f, 1, 2, true).await;
    let before = f.snapshot().await;
    sql(&f, "CREATE SEQUENCE cleanup_fault_hits").await;
    sql(&f, &format!("CREATE FUNCTION cleanup_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.cleanup_fault_hits'); RAISE EXCEPTION 'synthetic cleanup commit failure'; END $$", f.schema)).await;
    sql(
        &f,
        "CREATE CONSTRAINT TRIGGER cleanup_fault AFTER DELETE ON sessions
        DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION cleanup_fault()",
    )
    .await;
    let result = f.source.prune_expired_sessions().await;
    let reached = calls(&f).await;
    let after = f.snapshot().await;
    f.cleanup().await;
    assert_eq!(
        reached, 1,
        "the fault runs at COMMIT, after delete/readback"
    );
    assert_eq!(result, Err(DurableAuthError::StorageUnavailable));
    assert_eq!(after, before);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_cleanup_cancellation_during_post_delete_wait_is_observed_rolled_back() {
    let f = Fixture::ready().await;
    insert_sessions(&f, 1, 2, true).await;
    let before = f.snapshot().await;
    f.trigger("AFTER DELETE").await;
    let mut holder = f.pool.begin().await.unwrap();
    let blocker: i32 = query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut *holder)
        .await
        .unwrap()
        .get("pid");
    query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 76531)")
        .execute(&mut *holder)
        .await
        .unwrap();
    let source = f.source.clone();
    let pending = tokio::spawn(async move { source.prune_expired_sessions().await });
    f.wait_blocked(blocker).await;
    let reached = f.trigger_calls().await;
    pending.abort();
    let cancelled = pending.await.is_err_and(|error| error.is_cancelled());
    holder.rollback().await.unwrap();
    drain(&f).await;
    let after = f.snapshot().await;
    sql(&f, "DROP TRIGGER otp_fault ON sessions").await;
    let retry = f.source.prune_expired_sessions().await;
    f.cleanup().await;
    assert_eq!(reached, 1);
    assert!(cancelled);
    assert_eq!(
        after, before,
        "this rollback is observed, not inferred from cancellation alone"
    );
    assert_eq!(retry, Ok(2));
}
