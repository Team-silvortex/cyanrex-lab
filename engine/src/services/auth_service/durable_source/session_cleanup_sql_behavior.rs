use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_cleanup_is_source_wide_but_preserves_other_sources_and_live_rows() {
    let f = Fixture::ready().await;
    let other = Fixture::ready().await;
    insert_sessions(&f, 1, 2, true).await;
    insert_sessions(&f, 101, 1, false).await;
    insert_sessions(&other, 1, 2, true).await;
    query("INSERT INTO users(username, account_id, password_salt, password_hash, totp_secret)
        SELECT 'another-cleanup-account', $1, password_salt, password_hash, totp_secret FROM users WHERE username = $2")
        .bind(uuid::Uuid::new_v4()).bind(username().as_str()).execute(&f.pool).await.unwrap();
    query("INSERT INTO sessions(token, username, account_id, expires_at)
        SELECT lpad(to_hex(n), 64, '0'), username, account_id,
            clock_timestamp() + CASE WHEN n = 201 THEN INTERVAL '-1 microsecond' ELSE INTERVAL '1 day' END
        FROM users CROSS JOIN generate_series(201, 202) n WHERE username = 'another-cleanup-account'")
        .execute(&f.pool).await.unwrap();
    let before = f.snapshot().await;
    let other_before = other.snapshot().await;
    let active_before = active_sessions(&f).await;
    let result = f.source.prune_expired_sessions().await;
    let after = f.snapshot().await;
    let other_after = other.snapshot().await;
    let active_after = active_sessions(&f).await;
    let remaining = f.count_sessions().await;
    other.cleanup().await;
    f.cleanup().await;
    assert_eq!(result, Ok(3));
    assert_eq!(remaining, 2);
    assert_eq!(active_after, active_before);
    assert_eq!(after[0], before[0]);
    assert_eq!(after[2], before[2]);
    assert_eq!(other_after, other_before);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_cleanup_independent_pools_confirm_disjoint_bounded_batches() {
    let f = Fixture::ready().await;
    insert_sessions(&f, 1, 260, true).await;
    insert_sessions(&f, 501, 2, false).await;
    let before = f.snapshot().await;
    let pool = f.separate_pool("independent-session-cleanup").await;
    let other = f.independent_source(pool.clone());
    let (first, second) = tokio::join!(
        f.source.prune_expired_sessions(),
        other.prune_expired_sessions()
    );
    let remaining = f.count_sessions().await;
    let last = other.prune_expired_sessions().await;
    let empty = f.source.prune_expired_sessions().await;
    let after = f.snapshot().await;
    pool.close().await;
    f.cleanup().await;
    assert_eq!((first, second), (Ok(128), Ok(128)));
    assert_eq!(
        remaining, 6,
        "separate confirmations must represent disjoint deletions"
    );
    assert_eq!(last, Ok(4));
    assert_eq!(empty, Ok(0));
    assert_eq!(after[0], before[0]);
    assert_eq!(after[2], before[2]);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_cleanup_uses_fresh_database_cutoff_after_its_source_writer_wait() {
    let f = Fixture::ready().await;
    insert_sessions(&f, 1, 1, false).await;
    let mut holder = f.pool.begin().await.unwrap();
    query("SELECT singleton FROM collaboration_auth_source_schema FOR UPDATE")
        .fetch_one(&mut *holder)
        .await
        .unwrap();
    let blocker: i32 = query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut *holder)
        .await
        .unwrap()
        .get("pid");
    let source = f.source.clone();
    let pending = tokio::spawn(async move { source.prune_expired_sessions().await });
    f.wait_blocked(blocker).await;
    let waiting = !pending.is_finished();
    // This expiry is later than the reader's transaction/admission start, but earlier than the
    // eventual source-lock acquisition. No application clock or arbitrary sleep establishes it.
    query("UPDATE sessions SET expires_at = clock_timestamp()")
        .execute(&mut *holder)
        .await
        .unwrap();
    holder.commit().await.unwrap();
    let result = pending.await.unwrap();
    let remaining = f.count_sessions().await;
    f.cleanup().await;
    assert!(waiting);
    assert_eq!(result, Ok(1));
    assert_eq!(remaining, 0);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_cleanup_rejects_corrupt_selected_records_and_source_errors_without_repair(
) {
    for malformed in [
        "username",
        "oversized_username",
        "expires_at",
        "created_at",
        "positive_infinite_created_at",
        "finite_out_of_range_created_at",
        "oversized_token",
        "nonhex_token",
        "orphan_account",
    ] {
        let f = Fixture::ready().await;
        insert_sessions(&f, 99, 1, true).await;
        sql(
            &f,
            "UPDATE sessions SET expires_at = clock_timestamp() - INTERVAL '10 days'",
        )
        .await;
        if matches!(malformed, "username" | "oversized_username") {
            let name = if malformed == "username" {
                "bad/name".to_owned()
            } else {
                "x".repeat(65)
            };
            query("INSERT INTO users(username, account_id, password_salt, password_hash, totp_secret)
                SELECT $1, $2, password_salt, password_hash, totp_secret FROM users WHERE username = $3")
                .bind(&name).bind(uuid::Uuid::new_v4()).bind(username().as_str()).execute(&f.pool).await.unwrap();
            query("INSERT INTO sessions(token,username,account_id,expires_at)
                SELECT repeat('a',64),username,account_id,clock_timestamp()-INTERVAL '1 day' FROM users WHERE username=$1")
                .bind(name).execute(&f.pool).await.unwrap();
        } else if matches!(malformed, "oversized_token" | "nonhex_token") {
            // The existing schema guard checks this constraint's name/type, not its expression.
            // Keep the shape accepted to prove the selected-row decoder rejects corruption itself.
            sql(
                &f,
                "ALTER TABLE sessions DROP CONSTRAINT collaboration_auth_session_digest",
            )
            .await;
            sql(&f, "ALTER TABLE sessions ADD CONSTRAINT collaboration_auth_session_digest CHECK (TRUE)").await;
            let token = if malformed == "oversized_token" {
                "a".repeat(4097)
            } else {
                "g".repeat(64)
            };
            query("INSERT INTO sessions(token,username,account_id,expires_at)
                SELECT $1,username,account_id,clock_timestamp()-INTERVAL '1 day' FROM users WHERE username=$2")
                .bind(token).bind(username().as_str()).execute(&f.pool).await.unwrap();
        } else if malformed == "orphan_account" {
            // Synthetic database-owner corruption only; restore all ordinary/FK triggers before
            // invoking maintenance, so rejection proves exact account presence, not disabled guards.
            sql(&f, "ALTER TABLE sessions DISABLE TRIGGER ALL").await;
            query(
                "INSERT INTO sessions(token,username,account_id,expires_at)
                VALUES(repeat('a',64),$1,$2,clock_timestamp()-INTERVAL '1 day')",
            )
            .bind(username().as_str())
            .bind(uuid::Uuid::new_v4())
            .execute(&f.pool)
            .await
            .unwrap();
            sql(&f, "ALTER TABLE sessions ENABLE TRIGGER ALL").await;
        } else {
            insert_sessions(&f, 1, 1, true).await;
            let (column, timestamp) = match malformed {
                "expires_at" => ("expires_at", "-infinity"),
                "created_at" => ("created_at", "-infinity"),
                "positive_infinite_created_at" => ("created_at", "infinity"),
                "finite_out_of_range_created_at" => ("created_at", "262143-01-01 00:00:00+00"),
                _ => unreachable!(),
            };
            sql(
                &f,
                &format!(
                    "UPDATE sessions SET {column} = '{timestamp}' WHERE token = lpad('1', 64, '0')"
                ),
            )
            .await;
        }
        let before = f.snapshot().await;
        let source = f.source.clone();
        let result = tokio::spawn(async move { source.prune_expired_sessions().await }).await;
        drain(&f).await;
        let after = f.snapshot().await;
        f.cleanup().await;
        assert!(
            result.is_ok(),
            "{malformed}: invalid storage must not panic its caller"
        );
        assert_eq!(
            result.unwrap(),
            Err(DurableAuthError::InvalidRecord),
            "{malformed}"
        );
        assert_eq!(after, before, "{malformed}");
    }
    for populated in [false, true] {
        let f = Fixture::ready().await;
        if populated {
            insert_sessions(&f, 1, 1, true).await;
        }
        let before = f.snapshot().await;
        let mismatch = DurableAuthSource::new(
            f.pool.clone(),
            uuid::Uuid::new_v4().to_string().parse().unwrap(),
        )
        .prune_expired_sessions()
        .await;
        let closed_pool = f.separate_pool("closed-session-cleanup").await;
        closed_pool.close().await;
        let unavailable = f
            .independent_source(closed_pool)
            .prune_expired_sessions()
            .await;
        let unchanged = f.snapshot().await;
        sql(
            &f,
            "UPDATE collaboration_auth_source_schema SET version = 1",
        )
        .await;
        let old_schema = f.snapshot().await;
        let unsupported = f.source.prune_expired_sessions().await;
        let after = f.snapshot().await;
        f.cleanup().await;
        assert_eq!(
            mismatch,
            Err(DurableAuthError::SourceMismatch),
            "populated={populated}"
        );
        assert_eq!(
            unavailable,
            Err(DurableAuthError::StorageUnavailable),
            "populated={populated}"
        );
        assert_eq!(
            unsupported,
            Err(DurableAuthError::UnsupportedSchema),
            "populated={populated}"
        );
        assert_eq!(unchanged, before);
        assert_eq!(after, old_schema);
    }
}
