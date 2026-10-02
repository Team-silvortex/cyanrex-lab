use super::*;
use faults::Fault;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_bootstrap_concurrent_requests_choose_only_one_complete_authority() {
    let _guard = fixture_guard().await;
    for different in [false, true] {
        let f = Fixture::new().await;
        let other_scope = if different {
            WorkspaceRef {
                authority_id: id(),
                workspace_id: id(),
            }
        } else {
            scope()
        };
        let other = DurableAuthSource::new(f.pool.clone(), other_scope.authority_id);
        let other_name = if different {
            "other-owner".parse().unwrap()
        } else {
            username()
        };
        let name = username();
        let (one, two) = tokio::join!(
            f.source.bootstrap_empty_authority(scope(), &name, PASSWORD),
            other.bootstrap_empty_authority(other_scope, &other_name, PASSWORD)
        );
        let winner = match (one, two) {
            (Ok(winner), Err(BootstrapError::NotEmpty))
            | (Err(BootstrapError::NotEmpty), Ok(winner)) => winner,
            _ => panic!("bootstrap must commit one winner and reject the other"),
        };
        assert_eq!(f.count("users").await, 1);
        assert_eq!(f.count("sessions").await, 0);
        let source =
            DurableAuthSource::new(f.pool.clone(), winner.workspace.reference.authority_id);
        source
            .login(
                &winner.registration.account.username,
                PASSWORD,
                &otp(&winner.registration.bootstrap.secret),
            )
            .await
            .unwrap();
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL with event-trigger privileges"]
async fn postgres_bootstrap_installers_wait_for_commit_without_partial_activation() {
    let _guard = fixture_guard().await;
    for installer in ["source", "identity", "access"] {
        let f = Fixture::new().await;
        let mut blocker = f.pool.begin().await.unwrap();
        query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 71002)")
            .execute(&mut *blocker)
            .await
            .unwrap();
        let fault = Fault::install(
            &f,
            "collaboration_identity_audit",
            "AFTER INSERT",
            "PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 71002); RETURN NEW;",
            false,
        )
        .await;
        let source = f.source.clone();
        let mut bootstrap = tokio::spawn(async move {
            source
                .bootstrap_empty_authority(scope(), &username(), PASSWORD)
                .await
        });
        f.wait_blocked().await;
        assert_eq!(objects(&f).await, 0);
        let other = f.reopen().await;
        let store = CollaborationIdentityStore::new(f.pool.clone());
        let mut pending = tokio::spawn(async move {
            match installer {
                "source" => other
                    .install_empty_schema()
                    .await
                    .map_err(BootstrapError::from),
                "identity" => store.install_schema().await.map_err(BootstrapError::from),
                "access" => store
                    .install_access_schema()
                    .await
                    .map_err(BootstrapError::from),
                _ => unreachable!(),
            }
        });
        if installer == "access" {
            // This legacy installer reads identity metadata before its own advisory fence.
            // It sees no uncommitted table, so it may reject instead of waiting; neither is success.
            if let Ok(result) =
                tokio::time::timeout(std::time::Duration::from_millis(50), &mut pending).await
            {
                assert!(result.unwrap().is_err());
                blocker.rollback().await.unwrap();
                bootstrap.await.unwrap().unwrap();
                fault.remove(&f).await;
                f.cleanup().await;
                continue;
            }
        } else {
            assert!(
                tokio::time::timeout(std::time::Duration::from_millis(50), &mut pending)
                    .await
                    .is_err()
            );
        }
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), &mut bootstrap)
                .await
                .is_err()
        );
        blocker.rollback().await.unwrap();
        bootstrap.await.unwrap().unwrap();
        pending.await.unwrap().unwrap();
        assert_eq!(f.count("users").await, 1);
        fault.remove(&f).await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL with event-trigger privileges"]
async fn postgres_bootstrap_existing_installer_wins_without_implicit_manager_adoption() {
    let _guard = fixture_guard().await;
    for installer in ["source", "identity"] {
        let f = Fixture::new().await;
        let table = if installer == "source" {
            "collaboration_auth_source_schema"
        } else {
            "collaboration_identity_schema"
        };
        let mut blocker = f.pool.begin().await.unwrap();
        query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 71003)")
            .execute(&mut *blocker)
            .await
            .unwrap();
        let fault = Fault::install(
            &f,
            table,
            "AFTER INSERT",
            "PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 71003); RETURN NEW;",
            false,
        )
        .await;
        let source = f.source.clone();
        let store = CollaborationIdentityStore::new(f.pool.clone());
        let first = tokio::spawn(async move {
            if installer == "source" {
                source
                    .install_empty_schema()
                    .await
                    .map_err(BootstrapError::from)
            } else {
                store.install_schema().await.map_err(BootstrapError::from)
            }
        });
        f.wait_blocked().await;
        let other = f.reopen().await;
        let mut bootstrap = tokio::spawn(async move {
            other
                .bootstrap_empty_authority(scope(), &username(), PASSWORD)
                .await
        });
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), &mut bootstrap)
                .await
                .is_err()
        );
        blocker.rollback().await.unwrap();
        first.await.unwrap().unwrap();
        let before = objects(&f).await;
        assert!(matches!(
            bootstrap.await.unwrap(),
            Err(BootstrapError::NotEmpty)
        ));
        assert_eq!(objects(&f).await, before);
        fault.remove(&f).await;
        f.cleanup().await;
    }
}
