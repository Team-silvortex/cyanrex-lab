use super::*;
use faults::trigger;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_change_concurrent_reauthentication_commits_only_once() {
    let f = Fixture::ready().await;
    let (account, login) = f.login().await;
    let other = f.reopen().await;
    let code = otp(&account.bootstrap.secret);
    let (first, second) = tokio::join!(
        f.source
            .change_session_password(&login.token, PASSWORD, NEW_PASSWORD, &code),
        other.change_session_password(&login.token, PASSWORD, "another-synthetic-password", &code),
    );
    assert_ne!(first.is_ok(), second.is_ok());
    assert_eq!(
        first.err().or(second.err()),
        Some(DurableAuthError::InvalidCredentials)
    );
    assert_eq!(f.count("sessions").await, 0);
    let password = if first.is_ok() {
        NEW_PASSWORD
    } else {
        "another-synthetic-password"
    };
    assert_eq!(
        other
            .login(&username(), password, &otp(&account.bootstrap.secret))
            .await
            .unwrap()
            .session
            .account,
        account.account
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_change_serializes_login_without_surviving_old_tokens() {
    for change_first in [false, true] {
        let f = Fixture::ready().await;
        let (account, login) = f.login().await;
        let mut blocker = f.pool.begin().await.unwrap();
        query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 70903)")
            .execute(&mut *blocker)
            .await
            .unwrap();
        if change_first {
            trigger(
                &f,
                "users",
                "AFTER UPDATE",
                "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70903); RETURN NEW;",
            )
            .await;
        } else {
            trigger(
                &f,
                "sessions",
                "BEFORE INSERT",
                "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70903); RETURN NEW;",
            )
            .await;
        }
        let source = f.source.clone();
        let token = login.token.clone();
        let code = otp(&account.bootstrap.secret);
        let mut first = tokio::spawn(async move {
            if change_first {
                source
                    .change_session_password(&token, PASSWORD, NEW_PASSWORD, &code)
                    .await
                    .map(|_| None)
            } else {
                source
                    .login(&username(), PASSWORD, &code)
                    .await
                    .map(|login| Some(login.token))
            }
        });
        f.wait_blocked().await;
        let other = f.reopen().await;
        let token = login.token;
        let code = otp(&account.bootstrap.secret);
        let mut second = tokio::spawn(async move {
            if change_first {
                other
                    .login(&username(), PASSWORD, &code)
                    .await
                    .map(|login| Some(login.token))
            } else {
                other
                    .change_session_password(&token, PASSWORD, NEW_PASSWORD, &code)
                    .await
                    .map(|_| None)
            }
        });
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), &mut second)
                .await
                .is_err()
        );
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), &mut first)
                .await
                .is_err()
        );
        blocker.rollback().await.unwrap();
        let issued = first.await.unwrap().unwrap();
        if change_first {
            assert_eq!(
                second.await.unwrap(),
                Err(DurableAuthError::InvalidCredentials)
            );
        } else {
            second.await.unwrap().unwrap();
            assert_eq!(
                f.source.validate_session(&issued.unwrap()).await.unwrap(),
                None
            );
        }
        assert_eq!(f.count("sessions").await, 0);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_change_rechecks_snapshot_after_writer_wait() {
    for change in [
        "password",
        "totp",
        "session",
        "incarnation",
        "pending-login",
    ] {
        let f = Fixture::ready().await;
        let (account, login) = f.login().await;
        let replacement = f
            .source
            .register(&"replacement-source".parse().unwrap(), NEW_PASSWORD)
            .await
            .unwrap();
        let mut blocker = f.pool.begin().await.unwrap();
        // Let snapshot/Argon2 finish, but block the writer phase. Synthetic privileged changes
        // below deliberately bypass the adapter to test its stale-snapshot rejection.
        query("SELECT version FROM collaboration_auth_source_schema FOR SHARE")
            .fetch_one(&mut *blocker)
            .await
            .unwrap();
        let source = f.source.clone();
        let code = otp(&account.bootstrap.secret);
        let pending = tokio::spawn(async move {
            if change == "pending-login" {
                source.login(&username(), PASSWORD, &code).await.map(|_| ())
            } else {
                source
                    .change_session_password(&login.token, PASSWORD, NEW_PASSWORD, &code)
                    .await
            }
        });
        f.wait_blocked().await;
        match change {
            "password" | "pending-login" => {
                query("UPDATE users SET password_hash = (SELECT password_hash FROM users WHERE username = 'replacement-source'), password_salt = (SELECT password_salt FROM users WHERE username = 'replacement-source') WHERE username = 'source-learner'")
                    .execute(&mut *blocker).await.unwrap();
            }
            "totp" => {
                query("UPDATE users SET totp_secret = $1 WHERE username = 'source-learner'")
                    .bind(&replacement.bootstrap.secret)
                    .execute(&mut *blocker)
                    .await
                    .unwrap();
            }
            "session" => {
                query("DELETE FROM sessions")
                    .execute(&mut *blocker)
                    .await
                    .unwrap();
            }
            "incarnation" => {
                query("DELETE FROM users WHERE username = 'source-learner'")
                    .execute(&mut *blocker)
                    .await
                    .unwrap();
                query("INSERT INTO users (username, account_id, password_hash, password_salt, totp_secret) SELECT 'source-learner', gen_random_uuid(), password_hash, password_salt, totp_secret FROM users WHERE username = 'replacement-source'")
                    .execute(&mut *blocker).await.unwrap();
            }
            _ => unreachable!(),
        }
        blocker.commit().await.unwrap();
        assert_eq!(
            pending.await.unwrap(),
            Err(DurableAuthError::InvalidCredentials),
            "{change}"
        );
        assert_eq!(
            f.count("sessions").await,
            if matches!(change, "session" | "incarnation") {
                0
            } else {
                1
            }
        );
        let current = query("SELECT password_hash, totp_secret, account_id FROM users WHERE username = 'source-learner'").fetch_one(&f.pool).await.unwrap();
        if change == "incarnation" {
            assert_ne!(
                current.get::<Uuid, _>("account_id"),
                account.account.account_id.as_uuid()
            );
        } else if change == "totp" {
            assert_eq!(
                current.get::<String, _>("totp_secret"),
                replacement.bootstrap.secret
            );
        }
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_change_serializes_logout_and_blocks_session_validation() {
    for change_first in [false, true] {
        let f = Fixture::ready().await;
        let (account, login) = f.login().await;
        let before = state(&f).await;
        let mut blocker = f.pool.begin().await.unwrap();
        query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 70904)")
            .execute(&mut *blocker)
            .await
            .unwrap();
        trigger(
            &f,
            "sessions",
            "AFTER DELETE",
            "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70904); RETURN OLD;",
        )
        .await;
        let source = f.source.clone();
        let token = login.token.clone();
        let code = otp(&account.bootstrap.secret);
        let first = tokio::spawn(async move {
            if change_first {
                source
                    .change_session_password(&token, PASSWORD, NEW_PASSWORD, &code)
                    .await
            } else {
                source.logout(&token).await
            }
        });
        f.wait_blocked().await;
        let other = f.reopen().await;
        let token = login.token.clone();
        let code = otp(&account.bootstrap.secret);
        let mut second = tokio::spawn(async move {
            if change_first {
                other.logout(&token).await
            } else {
                other
                    .change_session_password(&token, PASSWORD, NEW_PASSWORD, &code)
                    .await
            }
        });
        let reader = f.reopen().await;
        let mut validation =
            tokio::spawn(async move { reader.validate_session(&login.token).await });
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), &mut second)
                .await
                .is_err()
        );
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), &mut validation)
                .await
                .is_err()
        );
        blocker.rollback().await.unwrap();
        first.await.unwrap().unwrap();
        if change_first {
            second.await.unwrap().unwrap();
        } else {
            assert_eq!(
                second.await.unwrap(),
                Err(DurableAuthError::InvalidCredentials)
            );
            assert_eq!(state(&f).await.0, before.0);
        }
        assert_eq!(validation.await.unwrap().unwrap(), None);
        assert_eq!(f.count("sessions").await, 0);
        f.cleanup().await;
    }
}
