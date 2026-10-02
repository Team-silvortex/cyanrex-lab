use super::*;
use faults::trigger;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_deletion_serializes_with_login_without_surviving_sessions() {
    for delete_first in [false, true] {
        let f = Fixture::ready().await;
        let created = f
            .source
            .register(&"racing-login".parse().unwrap(), PASSWORD)
            .await
            .unwrap();
        let target = f
            .source
            .bind_session_account(
                &f.manager_token,
                &SessionBindCommand {
                    command_id: id(),
                    workspace: scope(),
                    target: created.account.clone(),
                },
            )
            .await
            .unwrap()
            .entry
            .after;
        let mut blocker = f.base.pool.begin().await.unwrap();
        query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 70805)")
            .execute(&mut *blocker)
            .await
            .unwrap();
        if delete_first {
            trigger(
                &f,
                "users",
                "AFTER DELETE",
                "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70805); RETURN OLD;",
            )
            .await;
        } else {
            trigger(
                &f,
                "sessions",
                "BEFORE INSERT",
                "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70805); RETURN NEW;",
            )
            .await;
        }
        let source = f.source.clone();
        let token = f.manager_token.clone();
        let cmd = deletion(&target);
        let name = created.account.username.clone();
        let code = otp(&created.bootstrap.secret);
        let mut first = tokio::spawn(async move {
            if delete_first {
                source
                    .delete_session_account(&token, &cmd)
                    .await
                    .map(|_| None)
            } else {
                source
                    .login(&name, PASSWORD, &code)
                    .await
                    .map(|login| Some(login.token))
                    .map_err(SessionCommandError::from)
            }
        });
        f.base.wait_blocked().await;
        let other = f.base.reopen().await;
        let token = f.manager_token.clone();
        let cmd = deletion(&target);
        let name = created.account.username;
        let code = otp(&created.bootstrap.secret);
        let mut second = tokio::spawn(async move {
            if delete_first {
                other
                    .login(&name, PASSWORD, &code)
                    .await
                    .map(|login| Some(login.token))
                    .map_err(SessionCommandError::from)
            } else {
                other
                    .delete_session_account(&token, &cmd)
                    .await
                    .map(|_| None)
            }
        });
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), &mut second)
                .await
                .is_err()
        );
        // A paused INSERT/DELETE must never publish a token or receipt.
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), &mut first)
                .await
                .is_err()
        );
        blocker.rollback().await.unwrap();
        let result = first.await.unwrap().unwrap();
        if delete_first {
            assert_eq!(
                second.await.unwrap(),
                Err(SessionCommandError::Authentication(
                    DurableAuthError::InvalidCredentials
                ))
            );
        } else {
            second.await.unwrap().unwrap();
            assert!(f
                .source
                .validate_session(&result.unwrap())
                .await
                .unwrap()
                .is_none());
        }
        assert_eq!(f.base.count("users").await, 3);
        assert_eq!(f.base.count("sessions").await, 3);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_deletion_concurrent_duplicate_commits_only_once() {
    let f = Fixture::ready().await;
    let other = f.base.reopen().await;
    let cmd = deletion(&f.learner);
    let (a, b) = tokio::join!(
        f.source.delete_session_account(&f.manager_token, &cmd),
        other.delete_session_account(&f.manager_token, &cmd)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    assert_eq!(
        a.err().or(b.err()),
        Some(SessionCommandError::AccountMismatch)
    );
    assert_eq!(f.base.count("users").await, 2);
    assert_eq!(f.base.count("sessions").await, 2);
    assert_eq!(f.count("collaboration_identity_audit").await, 3);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_deletion_racing_policy_revoke_preserves_a_source_manager() {
    let f = Fixture::ready().await;
    grant_learner(&f).await;
    let other = f.base.reopen().await;
    let delete = deletion(&f.learner);
    let mut revoke = policy_command(f.manager.binding.principal_id, false);
    revoke.expected_revision = Some(RevisionNumber::try_from(1).unwrap());
    let (a, b) = tokio::join!(
        f.source.delete_session_account(&f.manager_token, &delete),
        other.apply_session_policy_command(&f.learner_token, &revoke)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    if a.is_ok() {
        assert_eq!(b, Err(SessionCommandError::InvalidSession));
        assert!(
            f.current(f.manager.binding.principal_id)
                .await
                .policy
                .deployment_granted
        );
        assert_eq!(f.base.count("users").await, 2);
    } else {
        assert_eq!(
            a,
            Err(SessionCommandError::Identity(
                IdentityStoreError::AccessDenied
            ))
        );
        assert!(f
            .source
            .validate_session(&f.learner_token)
            .await
            .unwrap()
            .is_some());
        assert!(
            f.current(f.learner.binding.principal_id)
                .await
                .policy
                .deployment_granted
        );
        assert_unchanged(&f, 2).await;
    }
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_deletion_serializes_with_logout_in_both_orders() {
    for delete_first in [true, false] {
        let f = Fixture::ready().await;
        let mut blocker = f.base.pool.begin().await.unwrap();
        query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 70803)")
            .execute(&mut *blocker)
            .await
            .unwrap();
        if delete_first {
            trigger(
                &f,
                "users",
                "AFTER DELETE",
                "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70803); RETURN OLD;",
            )
            .await;
        } else {
            trigger(
                &f,
                "sessions",
                "BEFORE DELETE",
                "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70803); RETURN OLD;",
            )
            .await;
        }
        let source = f.source.clone();
        let token = f.manager_token.clone();
        let cmd = deletion(&f.learner);
        let first = tokio::spawn(async move {
            if delete_first {
                source
                    .delete_session_account(&token, &cmd)
                    .await
                    .map(|_| ())
            } else {
                source
                    .logout(&token)
                    .await
                    .map_err(SessionCommandError::from)
            }
        });
        f.base.wait_blocked().await;
        let other = f.base.reopen().await;
        let token = f.manager_token.clone();
        let cmd = deletion(&f.learner);
        let mut second = tokio::spawn(async move {
            if delete_first {
                other
                    .logout(&token)
                    .await
                    .map_err(SessionCommandError::from)
            } else {
                other.delete_session_account(&token, &cmd).await.map(|_| ())
            }
        });
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), &mut second)
                .await
                .is_err()
        );
        blocker.rollback().await.unwrap();
        first.await.unwrap().unwrap();
        if delete_first {
            second.await.unwrap().unwrap();
            assert_eq!(f.base.count("users").await, 2);
            assert_eq!(f.base.count("sessions").await, 1);
            assert_eq!(f.count("collaboration_identity_audit").await, 3);
        } else {
            assert_eq!(
                second.await.unwrap(),
                Err(SessionCommandError::InvalidSession)
            );
            assert_eq!(f.base.count("users").await, 3);
            assert_eq!(f.base.count("sessions").await, 2);
            assert_eq!(f.count("collaboration_identity_audit").await, 2);
        }
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_deletion_blocks_validation_and_recreation_until_commit() {
    let f = Fixture::ready().await;
    let mut blocker = f.base.pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 70804)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    trigger(
        &f,
        "users",
        "AFTER DELETE",
        "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70804); RETURN OLD;",
    )
    .await;
    let source = f.source.clone();
    let token = f.manager_token.clone();
    let cmd = deletion(&f.learner);
    let pending = tokio::spawn(async move { source.delete_session_account(&token, &cmd).await });
    f.base.wait_blocked().await;
    let other = f.base.reopen().await;
    let token = f.learner_token.clone();
    let mut reader = tokio::spawn(async move { other.validate_session(&token).await });
    let other = f.base.reopen().await;
    let name = f.learner.binding.username.clone();
    let mut register = tokio::spawn(async move { other.register(&name, PASSWORD).await });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut reader)
            .await
            .is_err()
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut register)
            .await
            .is_err()
    );
    blocker.rollback().await.unwrap();
    pending.await.unwrap().unwrap();
    assert!(reader.await.unwrap().unwrap().is_none());
    assert_ne!(
        register.await.unwrap().unwrap().account.account_id,
        f.learner.account_id
    );
    assert_eq!(f.base.count("users").await, 3);
    assert_eq!(f.base.count("sessions").await, 2);
    f.cleanup().await;
}
