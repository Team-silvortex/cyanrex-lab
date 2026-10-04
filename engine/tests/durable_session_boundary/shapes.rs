use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_boundary_normal_source_needs_no_collaboration_registry() {
    let f = Fixture::ready().await;
    let (_, login) = f.login().await;
    assert_eq!(f.table_count().await, 3);
    assert_eq!(
        f.source.validate_session(&login.token).await.unwrap(),
        Some(login.session)
    );
    let independent = f.reopen().await;
    independent.logout(&login.token).await.unwrap();
    independent.logout(&login.token).await.unwrap();
    assert_eq!(f.source.validate_session(&login.token).await.unwrap(), None);
    assert_eq!(f.table_count().await, 3);
    assert_eq!(f.count("sessions").await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_boundary_all_entry_points_reject_ambiguous_search_path() {
    let f = Fixture::ready().await;
    let (created, login) = f.login().await;
    for path in [
        format!("{}, public", f.schema),
        format!("{}, pg_catalog", f.schema),
    ] {
        let pool = scoped_pool(&path, "boundary_ambiguous_path").await;
        let source = DurableAuthSource::new(pool.clone(), authority());
        assert!(source
            .login(&username(), PASSWORD, &next_otp(&created.bootstrap.secret))
            .await
            .is_err());
        assert!(source.validate_session(&login.token).await.is_err());
        assert!(source.logout(&login.token).await.is_err());
        pool.close().await;
        assert_eq!(f.count("sessions").await, 1);
    }
    assert_eq!(
        f.source.validate_session(&login.token).await.unwrap(),
        Some(login.session)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_boundary_all_entry_points_reject_temporary_relation_shadowing() {
    let f = Fixture::ready().await;
    let (created, login) = f.login().await;
    for table in ["users", "sessions", "collaboration_auth_source_schema"] {
        let pool = scoped_pool(&f.schema, "boundary_temporary_shadow").await;
        query(&format!(
            "CREATE TEMP VIEW {table} AS SELECT * FROM {}.{table}",
            f.schema
        ))
        .execute(&pool)
        .await
        .unwrap();
        let source = DurableAuthSource::new(pool.clone(), authority());
        assert!(
            source
                .login(&username(), PASSWORD, &next_otp(&created.bootstrap.secret))
                .await
                .is_err(),
            "{table}"
        );
        assert!(
            source.validate_session(&login.token).await.is_err(),
            "{table}"
        );
        assert!(source.logout(&login.token).await.is_err(), "{table}");
        pool.close().await;
        assert_eq!(f.count("sessions").await, 1);
    }
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_boundary_reads_reject_views_row_security_and_unknown_storage() {
    for alteration in [
        "ALTER TABLE sessions ENABLE ROW LEVEL SECURITY",
        "ALTER TABLE users ENABLE ROW LEVEL SECURITY",
        "ALTER TABLE collaboration_auth_source_schema ENABLE ROW LEVEL SECURITY",
        "ALTER TABLE sessions RENAME TO real_sessions; CREATE VIEW sessions AS SELECT * FROM real_sessions",
        "ALTER TABLE collaboration_auth_source_schema SET UNLOGGED",
        "ALTER TABLE sessions DROP CONSTRAINT sessions_pkey",
        "UPDATE collaboration_auth_source_schema SET version = 999",
    ] {
        let f = Fixture::ready().await;
        let (_, login) = f.login().await;
        for statement in alteration.split(';') { f.sql(statement).await; }
        assert!(f.source.validate_session(&login.token).await.is_err(), "accepted invalid source shape: {alteration}");
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_boundary_unprepared_or_wrong_authority_never_installs_or_adopts() {
    let f = Fixture::new().await;
    let token = Uuid::new_v4().to_string();
    assert!(f
        .source
        .login(&username(), PASSWORD, "000000")
        .await
        .is_err());
    assert!(f.source.validate_session(&token).await.is_err());
    assert!(f.source.logout(&token).await.is_err());
    assert_eq!(f.table_count().await, 0);
    f.source.install_empty_schema().await.unwrap();
    let (_, login) = f.login().await;
    let wrong = DurableAuthSource::new(f.pool.clone(), id());
    assert_eq!(
        wrong.validate_session(&login.token).await,
        Err(DurableAuthError::SourceMismatch)
    );
    assert_eq!(
        wrong.logout(&login.token).await,
        Err(DurableAuthError::SourceMismatch)
    );
    assert_eq!(f.count("sessions").await, 1);
    source_unchanged(&f).await;
    f.cleanup().await;
}
