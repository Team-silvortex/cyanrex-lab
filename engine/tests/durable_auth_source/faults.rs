use super::*;

async fn trigger(f: &Fixture, table: &str, timing: &str, body: &str) {
    f.sql(&format!(
        "CREATE FUNCTION source_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN {body} END $$"
    ))
    .await;
    f.sql(&format!("CREATE TRIGGER source_fault {timing} ON {table} FOR EACH ROW EXECUTE FUNCTION source_fault()")).await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_source_missing_or_unknown_storage_never_uses_a_cached_identity() {
    let f = Fixture::ready().await;
    let (_, login) = f.login().await;
    f.sql("ALTER TABLE sessions RENAME TO sessions_offline")
        .await;
    assert_eq!(
        f.source.validate_session(&login.token).await,
        Err(DurableAuthError::StorageUnavailable)
    );
    assert!(f.source.install_empty_schema().await.is_err());
    f.sql("ALTER TABLE sessions_offline RENAME TO sessions")
        .await;
    assert!(f
        .source
        .validate_session(&login.token)
        .await
        .unwrap()
        .is_some());
    f.sql("UPDATE collaboration_auth_source_schema SET version = 999")
        .await;
    assert_eq!(
        f.source.validate_session(&login.token).await,
        Err(DurableAuthError::UnsupportedSchema)
    );
    assert_eq!(
        f.source.install_empty_schema().await,
        Err(DurableAuthError::UnsupportedSchema)
    );
    f.sql("UPDATE collaboration_auth_source_schema SET version = 1")
        .await;
    f.sql("ALTER TABLE users DISABLE TRIGGER collaboration_auth_account_immutable")
        .await;
    assert!(f.source.install_empty_schema().await.is_err());
    f.sql("ALTER TABLE users ENABLE TRIGGER collaboration_auth_account_immutable")
        .await;
    f.source.install_empty_schema().await.unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_source_suppressed_or_altered_writes_never_publish_success() {
    for body in [
        "RETURN NULL;",
        "NEW.account_id = gen_random_uuid(); RETURN NEW;",
    ] {
        let f = Fixture::ready().await;
        trigger(&f, "users", "BEFORE INSERT", body).await;
        assert!(f.source.register(&username(), PASSWORD).await.is_err());
        assert_eq!(f.count("users").await, 0);
        f.cleanup().await;
    }
    for body in [
        "RETURN NULL;",
        "NEW.token = repeat('a', 64); RETURN NEW;",
        "NEW.expires_at = NOW() - INTERVAL '1 hour'; RETURN NEW;",
    ] {
        let f = Fixture::ready().await;
        let created = f.source.register(&username(), PASSWORD).await.unwrap();
        trigger(&f, "sessions", "BEFORE INSERT", body).await;
        assert!(f
            .source
            .login(&username(), PASSWORD, &otp(&created.bootstrap.secret))
            .await
            .is_err());
        assert_eq!(f.count("sessions").await, 0);
        f.cleanup().await;
    }
    let f = Fixture::ready().await;
    let (_, login) = f.login().await;
    trigger(&f, "sessions", "BEFORE DELETE", "RETURN NULL;").await;
    assert_eq!(
        f.source.logout(&login.token).await,
        Err(DurableAuthError::StorageUnavailable)
    );
    assert!(f
        .source
        .validate_session(&login.token)
        .await
        .unwrap()
        .is_some());
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_source_deferred_commit_failure_publishes_neither_account_nor_token() {
    for table in ["users", "sessions"] {
        let f = Fixture::ready().await;
        let created = if table == "sessions" {
            Some(f.source.register(&username(), PASSWORD).await.unwrap())
        } else {
            None
        };
        f.sql("CREATE FUNCTION source_commit_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic commit failure'; END $$").await;
        f.sql(&format!("CREATE CONSTRAINT TRIGGER source_commit_fault AFTER INSERT ON {table} DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION source_commit_fault()")).await;
        if let Some(created) = created {
            assert!(matches!(
                f.source
                    .login(&username(), PASSWORD, &otp(&created.bootstrap.secret))
                    .await,
                Err(DurableAuthError::StorageUnavailable)
            ));
            assert_eq!(f.count("users").await, 1);
        } else {
            assert!(matches!(
                f.source.register(&username(), PASSWORD).await,
                Err(DurableAuthError::StorageUnavailable)
            ));
            assert_eq!(f.count("users").await, 0);
        }
        assert_eq!(f.count("sessions").await, 0);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_source_cancelled_login_does_not_publish_a_durable_session() {
    let f = Fixture::ready().await;
    let created = f.source.register(&username(), PASSWORD).await.unwrap();
    let mut blocker = f.pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 70451)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    trigger(
        &f,
        "sessions",
        "BEFORE INSERT",
        "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70451); RETURN NEW;",
    )
    .await;
    let source = f.source.clone();
    let code = otp(&created.bootstrap.secret);
    let pending = tokio::spawn(async move { source.login(&username(), PASSWORD, &code).await });
    f.wait_blocked().await;
    pending.abort();
    assert!(pending.await.is_err_and(|error| error.is_cancelled()));
    blocker.rollback().await.unwrap();
    f.drain().await;
    assert_eq!(f.count("sessions").await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_source_session_read_waits_for_revocation_commit() {
    let f = Fixture::ready().await;
    let (_, login) = f.login().await;
    let mut blocker = f.pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 70452)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    trigger(
        &f,
        "sessions",
        "BEFORE DELETE",
        "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70452); RETURN OLD;",
    )
    .await;
    let source = f.source.clone();
    let token = login.token.clone();
    let writer = tokio::spawn(async move { source.logout(&token).await });
    f.wait_blocked().await;
    let source = f.reopen().await;
    let token = login.token.clone();
    let mut reader = tokio::spawn(async move { source.validate_session(&token).await });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut reader)
            .await
            .is_err(),
        "read skipped the pending revocation"
    );
    blocker.rollback().await.unwrap();
    writer.await.unwrap().unwrap();
    assert_eq!(reader.await.unwrap().unwrap(), None);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_source_post_insert_rechecks_credentials_and_exact_account_generation() {
    for body in [
        "UPDATE users SET password_hash = 'synthetic-changed' WHERE username = NEW.username; RETURN NEW;",
        "CREATE TEMP TABLE copied_user ON COMMIT DROP AS SELECT * FROM users WHERE username = NEW.username; DELETE FROM users WHERE username = NEW.username; INSERT INTO users (username, account_id, password_salt, password_hash, totp_secret) SELECT username, gen_random_uuid(), password_salt, password_hash, totp_secret FROM copied_user; RETURN NEW;",
    ] {
        let f = Fixture::ready().await;
        let created = f.source.register(&username(), PASSWORD).await.unwrap();
        trigger(&f, "sessions", "BEFORE INSERT", body).await;
        assert!(f.source.login(&username(), PASSWORD, &otp(&created.bootstrap.secret)).await.is_err());
        assert_eq!(f.count("sessions").await, 0);
        let stored: Uuid = query("SELECT account_id FROM users").fetch_one(&f.pool).await.unwrap().get("account_id");
        assert_eq!(stored, created.account.account_id.as_uuid());
        f.sql("DROP TRIGGER source_fault ON sessions").await;
        f.source.login(&username(), PASSWORD, &otp(&created.bootstrap.secret)).await.unwrap();
        f.cleanup().await;
    }
}
