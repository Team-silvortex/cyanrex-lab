use super::*;

async fn insert_trigger(f: &Fixture, body: &str) {
    f.sql(&format!("CREATE FUNCTION identity_audit_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN {body} END $$")).await;
    f.sql("CREATE TRIGGER identity_audit_fault BEFORE INSERT ON collaboration_identity_audit FOR EACH ROW EXECUTE FUNCTION identity_audit_fault()").await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_lifecycle_lookup_waits_for_binding_and_its_audit_before_reporting_absence() {
    let (f, manager, _, _) = ready().await;
    insert_trigger(
        &f,
        "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 34568); RETURN NEW;",
    )
    .await;
    let mut blocker = f.pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 34568)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let cmd = bind(manager.binding.principal_id);
    let LegacyIdentityAction::Bind {
        username,
        account_id,
    } = cmd.action.clone()
    else {
        unreachable!()
    };
    let store = f.store.clone();
    let writer = tokio::spawn(async move { store.apply_legacy_identity_command(&cmd).await });
    f.wait_blocked().await;
    let store = f.reopen().await;
    let mut reader = tokio::spawn(async move {
        store
            .lookup_legacy_account(scope(), &username, account_id)
            .await
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut reader)
            .await
            .is_err(),
        "identity lookup skipped the authority lock"
    );
    blocker.rollback().await.unwrap();
    let bound = writer.await.unwrap().unwrap();
    assert_eq!(reader.await.unwrap().unwrap(), Some(bound.entry.after));
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_lifecycle_suppressed_or_altered_audit_rolls_back_binding_and_retirement() {
    for body in ["RETURN NULL;", "NEW.after_state = NEW.after_state || '{\"unexpected\": true}'::jsonb; RETURN NEW;",
        "UPDATE collaboration_principals SET display_name = 'tampered' WHERE authority_id = NEW.authority_id AND principal_id = NEW.principal_id; RETURN NEW;"] {
        let (f, manager, _, target) = ready().await;
        insert_trigger(&f, body).await;
        let cmd = bind(manager.binding.principal_id);
        assert!(f.store.apply_legacy_identity_command(&cmd).await.is_err());
        let retirement = retire(manager.binding.principal_id, &target);
        assert!(f.store.apply_legacy_identity_command(&retirement).await.is_err());
        assert_eq!(current(&f, &target).await, target);
        assert_eq!(f.count("collaboration_principals").await, 3);
        assert_eq!(f.count("collaboration_legacy_identities").await, 3);
        assert_eq!(f.count("collaboration_identity_audit").await, 3);
        f.sql("DROP TRIGGER identity_audit_fault ON collaboration_identity_audit").await;
        assert!(!f.store.apply_legacy_identity_command(&cmd).await.unwrap().replayed);
        assert!(!f.store.apply_legacy_identity_command(&retirement).await.unwrap().replayed);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_lifecycle_deferred_commit_failure_keeps_identity_and_audit_together() {
    let (f, manager, _, target) = ready().await;
    f.sql("CREATE FUNCTION identity_commit_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic commit failure'; END $$").await;
    f.sql("CREATE CONSTRAINT TRIGGER identity_commit_fault AFTER INSERT ON collaboration_identity_audit DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION identity_commit_fault()").await;
    let cmd = retire(manager.binding.principal_id, &target);
    assert_eq!(
        f.store.apply_legacy_identity_command(&cmd).await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    assert_eq!(current(&f, &target).await, target);
    assert_eq!(f.count("collaboration_identity_audit").await, 3);
    f.sql("DROP TRIGGER identity_commit_fault ON collaboration_identity_audit")
        .await;
    f.store.apply_legacy_identity_command(&cmd).await.unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_lifecycle_cancellation_before_audit_commit_restores_active_identity() {
    let (f, manager, _, target) = ready().await;
    insert_trigger(
        &f,
        "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 34567); RETURN NEW;",
    )
    .await;
    let mut blocker = f.pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 34567)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let cmd = retire(manager.binding.principal_id, &target);
    let store = f.store.clone();
    let pending = cmd.clone();
    let task = tokio::spawn(async move { store.apply_legacy_identity_command(&pending).await });
    f.wait_blocked().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    blocker.rollback().await.unwrap();
    f.drain().await;
    assert_eq!(current(&f, &target).await, target);
    assert_eq!(f.count("collaboration_identity_audit").await, 3);
    f.store.apply_legacy_identity_command(&cmd).await.unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_lifecycle_missing_audit_or_unlogged_state_changes_fail_closed() {
    let (f, manager, _, target) = ready().await;
    f.sql("ALTER TABLE collaboration_identity_audit RENAME TO identity_audit_offline")
        .await;
    assert!(f
        .store
        .lookup_legacy_account(scope(), &username(), account())
        .await
        .is_err());
    assert!(f
        .store
        .apply_legacy_identity_command(&bind(manager.binding.principal_id))
        .await
        .is_err());
    assert!(f.store.install_schema().await.is_err());
    assert!(f.store.upgrade_identity_audit_schema().await.is_err());
    f.sql("ALTER TABLE identity_audit_offline RENAME TO collaboration_identity_audit")
        .await;
    assert_eq!(current(&f, &target).await, target);
    f.sql(&format!(
        "UPDATE collaboration_principals SET status = 'disabled' WHERE principal_id = '{}'",
        target.binding.principal_id
    ))
    .await;
    assert_eq!(
        f.store
            .lookup_legacy_account(scope(), &username(), account())
            .await,
        Err(IdentityStoreError::InvalidRecord)
    );
    assert_eq!(
        f.store
            .preview_legacy_access(
                scope(),
                actor(target.binding.principal_id),
                LegacyAction::ManageDeployment,
                &LegacyPolicyResource::Deployment {
                    authority_id: scope().authority_id
                }
            )
            .await,
        Err(IdentityStoreError::InvalidRecord)
    );
    assert!(f
        .store
        .apply_legacy_identity_command(&retire(manager.binding.principal_id, &target))
        .await
        .is_err());
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_lifecycle_bad_baseline_or_conflicting_table_rolls_back_upgrade() {
    let (f, _, _, target) = seeded().await;
    f.sql(&format!(
        "UPDATE collaboration_legacy_identities SET retired_at = NOW() WHERE principal_id = '{}'",
        target.binding.principal_id
    ))
    .await;
    assert_eq!(
        f.store.upgrade_identity_audit_schema().await,
        Err(IdentityStoreError::InvalidRecord)
    );
    assert_eq!(
        query("SELECT version FROM collaboration_identity_schema")
            .fetch_one(&f.pool)
            .await
            .unwrap()
            .get::<i32, _>("version"),
        1
    );
    assert!(query("SELECT * FROM collaboration_identity_audit")
        .execute(&f.pool)
        .await
        .is_err());
    f.sql("UPDATE collaboration_legacy_identities SET retired_at = NULL")
        .await;
    f.sql("CREATE TABLE collaboration_identity_audit (sentinel TEXT)")
        .await;
    f.sql("INSERT INTO collaboration_identity_audit VALUES ('keep')")
        .await;
    assert!(f.store.upgrade_identity_audit_schema().await.is_err());
    assert_eq!(f.count("collaboration_identity_audit").await, 1);
    assert_eq!(current(&f, &target).await, target);
    f.cleanup().await;
}
