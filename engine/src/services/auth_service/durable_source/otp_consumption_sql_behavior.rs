use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_consumption_survives_reopen_logout_and_allows_only_new_steps() {
    let f = Fixture::ready().await;
    let initial = counter(&f, username().as_str()).await;
    let code = f.code();
    let login = f.source.login(&username(), PASSWORD, &code).await.unwrap();
    let committed = counter(&f, username().as_str()).await;
    let pool = f.separate_pool("otp-consumption-reopened-source").await;
    let reopened = f.independent_source(pool.clone());
    let replay = command(&reopened, None, &code).await;
    reopened.logout(&login.token).await.unwrap();
    let after_logout = command(&reopened, None, &code).await;
    reopened.install_empty_schema().await.unwrap();
    let new_code = f.next_code();
    let new_login = command(&reopened, None, &new_code).await;
    let last = counter(&f, username().as_str()).await;
    let expected = clock_counter(&f);
    let sessions = f.count_sessions().await;
    pool.close().await;
    f.cleanup().await;
    assert_eq!(initial, -1);
    assert_eq!(committed, expected - 1);
    assert_eq!(replay, Err(DurableAuthError::InvalidCredentials));
    assert_eq!(after_logout, Err(DurableAuthError::InvalidCredentials));
    assert_eq!(new_login, Ok(()));
    assert_eq!(last, expected);
    assert_eq!(sessions, 1);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_consumption_login_and_rotation_compete_for_the_same_step() {
    let f = Fixture::ready().await;
    let token = setup_rotation(&f).await;
    let code = f.code();
    let separate = f
        .separate_pool("otp-consumption-login-versus-rotation")
        .await;
    let other = f.independent_source(separate.clone());
    let (login, rotation) = tokio::join!(
        command(&other, None, &code),
        command(&f.source, Some(&token), &code),
    );
    let sessions = f.count_sessions().await;
    let last = counter(&f, username().as_str()).await;
    let expected = clock_counter(&f);
    let original = f.source.validate_session(&token).await.unwrap();
    let password = if rotation.is_ok() {
        NEW_PASSWORD
    } else {
        PASSWORD
    };
    let next = f.next_code();
    let next_login = f
        .source
        .login(&username(), password, &next)
        .await
        .map(|_| ());
    separate.close().await;
    f.cleanup().await;
    assert_eq!(
        [login, rotation]
            .iter()
            .filter(|result| result.is_ok())
            .count(),
        1
    );
    assert_eq!(
        [login, rotation]
            .iter()
            .filter(|result| **result == Err(DurableAuthError::InvalidCredentials))
            .count(),
        1
    );
    assert_eq!(last, expected);
    assert_eq!(sessions, if rotation.is_ok() { 0 } else { 2 });
    assert_eq!(original.is_none(), rotation.is_ok());
    assert_eq!(next_login, Ok(()));
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_consumption_is_scoped_to_exact_accounts_and_new_incarnations() {
    let f = Fixture::ready().await;
    let other_name = "otp-other-learner".parse().unwrap();
    f.source.register(&other_name, PASSWORD).await.unwrap();
    query("UPDATE users SET totp_secret = $1 WHERE username = $2")
        .bind(SECRET)
        .bind(other_name.as_str())
        .execute(&f.pool)
        .await
        .unwrap();
    let original = f
        .source
        .login(&username(), PASSWORD, &f.code())
        .await
        .unwrap();
    let other_before = counter(&f, other_name.as_str()).await;
    let other = f.source.login(&other_name, PASSWORD, &f.code()).await;
    let first_counter = counter(&f, username().as_str()).await;
    let second_counter = counter(&f, other_name.as_str()).await;
    let expected = clock_counter(&f);
    query("DELETE FROM users WHERE username = $1")
        .bind(username().as_str())
        .execute(&f.pool)
        .await
        .unwrap();
    let replacement = f.source.register(&username(), PASSWORD).await.unwrap();
    let replacement_before = counter(&f, username().as_str()).await;
    query("UPDATE users SET totp_secret = $1 WHERE username = $2")
        .bind(SECRET)
        .bind(username().as_str())
        .execute(&f.pool)
        .await
        .unwrap();
    let replacement_login = command(&f.source, None, &f.code()).await;
    let old_session = f.source.validate_session(&original.token).await.unwrap();
    f.cleanup().await;
    assert_eq!(
        other_before, -1,
        "another account's watermark is not consumed"
    );
    assert!(
        other.is_ok(),
        "the same code is independently scoped per account"
    );
    assert_eq!(first_counter, expected);
    assert_eq!(second_counter, expected);
    assert_ne!(
        replacement.account.account_id,
        original.session.account.account_id
    );
    assert_eq!(replacement_before, -1);
    assert_eq!(replacement_login, Ok(()));
    assert_eq!(old_session, None);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_consumption_rejects_version_one_without_adoption_or_writes() {
    for empty in [false, true] {
        let f = Fixture::ready().await;
        let token = setup_rotation(&f).await;
        if empty {
            sql(&f, "DELETE FROM users").await;
        }
        sql(&f, "ALTER TABLE users DROP COLUMN otp_last_counter").await;
        sql(
            &f,
            "UPDATE collaboration_auth_source_schema SET version = 1",
        )
        .await;
        let before = f.snapshot().await;
        let install = f.source.install_empty_schema().await;
        let login = command(&f.source, None, &f.code()).await;
        let rotation = command(&f.source, Some(&token), &f.code()).await;
        let read = f.source.validate_session(&token).await.map(|_| ());
        let logout = f.source.logout(&token).await;
        let register = f
            .source
            .register(&"otp-another-account".parse().unwrap(), PASSWORD)
            .await
            .map(|_| ());
        let after = f.snapshot().await;
        let column_exists: bool = query("SELECT EXISTS (SELECT 1 FROM pg_attribute WHERE attrelid='users'::regclass AND attname='otp_last_counter' AND NOT attisdropped) AS present")
            .fetch_one(&f.pool).await.unwrap().get("present");
        f.cleanup().await;
        for result in [install, login, rotation, read, logout, register] {
            assert_eq!(
                result,
                Err(DurableAuthError::UnsupportedSchema),
                "empty={empty}"
            );
        }
        assert_eq!(after, before);
        assert!(!column_exists);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_consumption_rejects_incompatible_counter_shape_without_repair() {
    for damage in [
        "ALTER TABLE users ALTER COLUMN otp_last_counter DROP NOT NULL",
        "ALTER TABLE users ALTER COLUMN otp_last_counter DROP DEFAULT",
        "ALTER TABLE users ALTER COLUMN otp_last_counter SET DEFAULT 0",
        "ALTER TABLE users DROP CONSTRAINT collaboration_auth_otp_last_counter",
        "ALTER TABLE users ALTER COLUMN otp_last_counter TYPE INTEGER",
    ] {
        let f = Fixture::ready().await;
        sql(&f, damage).await;
        let before = f.snapshot().await;
        let login = command(&f.source, None, &f.code()).await;
        let after = f.snapshot().await;
        f.cleanup().await;
        assert!(login.is_err(), "{damage}");
        assert_eq!(after, before, "{damage}");
    }
}
