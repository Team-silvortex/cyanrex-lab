use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_boundary_login_rechecks_source_metadata_after_insert() {
    for statement in [
        "UPDATE collaboration_auth_source_schema SET version = 999".to_owned(),
        format!(
            "UPDATE collaboration_auth_source_schema SET authority_id = '{}'",
            Uuid::new_v4()
        ),
    ] {
        let f = Fixture::ready().await;
        let created = f.source.register(&username(), PASSWORD).await.unwrap();
        fault(&f, "AFTER INSERT", &format!("{statement}; RETURN NEW;")).await;
        let result = f
            .source
            .login(&username(), PASSWORD, &otp(&created.bootstrap.secret))
            .await;
        assert_eq!(calls(&f).await, 1);
        assert!(
            result.is_err(),
            "login published a token after mutating source authority/version"
        );
        assert_eq!(f.count("sessions").await, 0);
        assert_eq!(f.count("users").await, 1);
        source_unchanged(&f).await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_boundary_logout_rechecks_source_metadata_after_delete() {
    for statement in [
        "UPDATE collaboration_auth_source_schema SET version = 999".to_owned(),
        format!(
            "UPDATE collaboration_auth_source_schema SET authority_id = '{}'",
            Uuid::new_v4()
        ),
    ] {
        let f = Fixture::ready().await;
        let (_, login) = f.login().await;
        fault(&f, "AFTER DELETE", &format!("{statement}; RETURN OLD;")).await;
        let result = f.source.logout(&login.token).await;
        assert_eq!(calls(&f).await, 1);
        assert!(
            result.is_err(),
            "logout committed a change to the source authority/version"
        );
        assert_eq!(f.count("sessions").await, 1);
        source_unchanged(&f).await;
        assert_eq!(
            f.source.validate_session(&login.token).await.unwrap(),
            Some(login.session)
        );
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_boundary_login_rejects_identical_replacement_account_relation() {
    let f = Fixture::ready().await;
    let created = f.source.register(&username(), PASSWORD).await.unwrap();
    let original: i64 = query("SELECT 'users'::regclass::oid::bigint AS oid")
        .fetch_one(&f.pool)
        .await
        .unwrap()
        .get("oid");
    fault(&f, "AFTER INSERT", "ALTER TABLE users RENAME TO prior_users;
        CREATE TABLE users (LIKE prior_users INCLUDING ALL); INSERT INTO users SELECT * FROM prior_users;
        RETURN NEW;").await;
    let result = f
        .source
        .login(&username(), PASSWORD, &otp(&created.bootstrap.secret))
        .await;
    assert_eq!(calls(&f).await, 1);
    assert!(
        result.is_err(),
        "identical account fields do not authorize replacing their relation"
    );
    let row = query("SELECT 'users'::regclass::oid::bigint AS oid, to_regclass('prior_users') IS NULL AS restored").fetch_one(&f.pool).await.unwrap();
    assert_eq!(row.get::<i64, _>("oid"), original);
    assert!(row.get::<bool, _>("restored"));
    assert_eq!(f.count("sessions").await, 0);
    assert_eq!(f.count("users").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_boundary_login_rejects_a_fully_valid_redirected_namespace() {
    let f = Fixture::ready().await;
    let shadow = Fixture::ready().await;
    let created = f.source.register(&username(), PASSWORD).await.unwrap();
    query(&format!(
        "INSERT INTO {}.users SELECT * FROM {}.users",
        shadow.schema, f.schema
    ))
    .execute(&f.admin)
    .await
    .unwrap();
    fault(
        &f,
        "AFTER INSERT",
        &format!(
            "INSERT INTO {}.sessions SELECT NEW.*;
        PERFORM set_config('search_path', '{}', true); RETURN NEW;",
            shadow.schema, shadow.schema
        ),
    )
    .await;
    let result = f
        .source
        .login(&username(), PASSWORD, &otp(&created.bootstrap.secret))
        .await;
    assert_eq!(calls(&f).await, 1);
    assert!(
        result.is_err(),
        "matching users/source metadata do not authorize switching namespaces"
    );
    assert_eq!(f.count("sessions").await, 0);
    assert_eq!(shadow.count("sessions").await, 0);
    source_unchanged(&f).await;
    source_unchanged(&shadow).await;
    shadow.cleanup().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_boundary_logout_rejects_replaced_source_relation_and_restores_session() {
    let f = Fixture::ready().await;
    let (_, login) = f.login().await;
    let original: i64 =
        query("SELECT 'collaboration_auth_source_schema'::regclass::oid::bigint AS oid")
            .fetch_one(&f.pool)
            .await
            .unwrap()
            .get("oid");
    fault(
        &f,
        "AFTER DELETE",
        "ALTER TABLE collaboration_auth_source_schema RENAME TO prior_source;
        CREATE TABLE collaboration_auth_source_schema (LIKE prior_source INCLUDING ALL);
        INSERT INTO collaboration_auth_source_schema SELECT * FROM prior_source; RETURN OLD;",
    )
    .await;
    let result = f.source.logout(&login.token).await;
    assert_eq!(calls(&f).await, 1);
    assert!(
        result.is_err(),
        "logout adopted a replacement source relation inside its own transaction"
    );
    let oid: i64 = query("SELECT 'collaboration_auth_source_schema'::regclass::oid::bigint AS oid")
        .fetch_one(&f.pool)
        .await
        .unwrap()
        .get("oid");
    assert_eq!(oid, original);
    assert_eq!(
        f.source.validate_session(&login.token).await.unwrap(),
        Some(login.session)
    );
    assert_eq!(f.count("sessions").await, 1);
    f.cleanup().await;
}
