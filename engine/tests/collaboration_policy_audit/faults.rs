use super::*;

async fn insert_trigger(f: &Fixture, body: &str) {
    f.sql(&format!(
        "CREATE FUNCTION audit_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN {body} END $$"
    ))
    .await;
    f.sql("CREATE TRIGGER audit_fault BEFORE INSERT ON collaboration_policy_audit FOR EACH ROW EXECUTE FUNCTION audit_fault()").await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_audit_suppressed_or_tampered_receipt_rolls_back_policy_and_allows_retry() {
    for body in [
        "RETURN NULL;",
        "NEW.actor_principal_id = NEW.principal_id; RETURN NEW;",
        "NEW.after_state = NEW.after_state || '{\"unexpected\": true}'::jsonb; RETURN NEW;",
        "UPDATE collaboration_deployment_grants SET status = 'revoked' WHERE authority_id = NEW.authority_id AND principal_id = NEW.principal_id; RETURN NEW;",
    ] {
        let (f, manager, _, target) = ready().await;
        let before = current(&f, target).await;
        let cmd = command(manager, target);
        insert_trigger(&f, body).await;
        assert!(f.store.apply_legacy_policy_command(&cmd).await.is_err());
        assert_eq!(current(&f, target).await, before);
        assert_eq!(f.count("collaboration_policy_audit").await, 3);
        f.sql("DROP TRIGGER audit_fault ON collaboration_policy_audit")
            .await;
        assert!(
            !f.store
                .apply_legacy_policy_command(&cmd)
                .await
                .unwrap()
                .replayed
        );
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_audit_deferred_commit_failure_never_commits_policy_alone() {
    let (f, manager, _, target) = ready().await;
    let before = current(&f, target).await;
    f.sql("CREATE FUNCTION audit_commit_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic commit failure'; END $$").await;
    f.sql("CREATE CONSTRAINT TRIGGER audit_commit_fault AFTER INSERT ON collaboration_policy_audit DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION audit_commit_fault()").await;
    let cmd = command(manager, target);
    assert_eq!(
        f.store.apply_legacy_policy_command(&cmd).await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    assert_eq!(current(&f, target).await, before);
    assert_eq!(f.count("collaboration_policy_audit").await, 3);
    f.sql("DROP TRIGGER audit_commit_fault ON collaboration_policy_audit")
        .await;
    f.store.apply_legacy_policy_command(&cmd).await.unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_audit_cancellation_during_receipt_insert_rolls_back_prior_policy_writes() {
    let (f, manager, _, target) = ready().await;
    let before = current(&f, target).await;
    insert_trigger(
        &f,
        "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 86421); RETURN NEW;",
    )
    .await;
    let mut blocker = f.pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 86421)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let store = f.store.clone();
    let cmd = command(manager, target);
    let pending = cmd.clone();
    let task = tokio::spawn(async move { store.apply_legacy_policy_command(&pending).await });
    f.wait_blocked().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    blocker.rollback().await.unwrap();
    f.drain().await;
    assert_eq!(current(&f, target).await, before);
    assert_eq!(f.count("collaboration_policy_audit").await, 3);
    f.store.apply_legacy_policy_command(&cmd).await.unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_audit_missing_history_or_unlogged_policy_edits_fail_closed_without_cache() {
    let (f, manager, _, target) = ready().await;
    let resource = LegacyPolicyResource::Deployment {
        authority_id: scope().authority_id,
    };
    assert!(f
        .store
        .preview_legacy_access(
            scope(),
            actor(manager),
            LegacyAction::ManageDeployment,
            &resource
        )
        .await
        .unwrap());
    f.sql("ALTER TABLE collaboration_policy_audit RENAME TO audit_offline")
        .await;
    assert!(f.store.legacy_access(scope(), manager).await.is_err());
    assert!(f
        .store
        .preview_legacy_access(
            scope(),
            actor(manager),
            LegacyAction::ManageDeployment,
            &resource
        )
        .await
        .is_err());
    assert!(f
        .store
        .apply_legacy_policy_command(&command(manager, target))
        .await
        .is_err());
    assert!(f.store.install_access_schema().await.is_err());
    assert!(f.store.upgrade_policy_audit_schema().await.is_err());
    f.sql("ALTER TABLE audit_offline RENAME TO collaboration_policy_audit")
        .await;
    f.sql("ALTER TABLE collaboration_policy_audit DISABLE TRIGGER collaboration_policy_audit_immutable").await;
    assert_eq!(
        f.store.install_access_schema().await,
        Err(IdentityStoreError::InvalidRecord)
    );
    assert_eq!(
        f.store.upgrade_policy_audit_schema().await,
        Err(IdentityStoreError::InvalidRecord)
    );
    f.sql("ALTER TABLE collaboration_policy_audit ENABLE TRIGGER collaboration_policy_audit_immutable").await;
    f.store.upgrade_policy_audit_schema().await.unwrap();
    assert!(f
        .store
        .preview_legacy_access(
            scope(),
            actor(manager),
            LegacyAction::ManageDeployment,
            &resource
        )
        .await
        .unwrap());
    f.sql(&format!("UPDATE collaboration_deployment_grants SET status = 'revoked' WHERE principal_id = '{manager}'")).await;
    assert_eq!(
        f.store.legacy_access(scope(), manager).await,
        Err(IdentityStoreError::InvalidRecord)
    );
    assert!(f
        .store
        .preview_legacy_access(
            scope(),
            actor(manager),
            LegacyAction::ManageDeployment,
            &resource
        )
        .await
        .is_err());
    assert!(f
        .store
        .apply_legacy_policy_command(&command(manager, target))
        .await
        .is_err());
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_audit_invalid_baseline_rolls_back_schema_upgrade() {
    let (f, _, _, target) = seeded().await;
    f.sql(&format!("UPDATE collaboration_memberships SET role_refs = ARRAY['cyanrex.teaching.teacher', 'cyanrex.teaching.learner'] WHERE principal_id = '{target}'")).await;
    assert_eq!(
        f.store.upgrade_policy_audit_schema().await,
        Err(IdentityStoreError::InvalidRecord)
    );
    assert_eq!(
        query("SELECT version FROM collaboration_access_schema")
            .fetch_one(&f.pool)
            .await
            .unwrap()
            .get::<i32, _>("version"),
        1
    );
    assert!(query("SELECT * FROM collaboration_policy_audit")
        .execute(&f.pool)
        .await
        .is_err());
    f.sql(&format!("UPDATE collaboration_memberships SET role_refs = ARRAY['cyanrex.teaching.learner'] WHERE principal_id = '{target}'")).await;
    f.store.upgrade_policy_audit_schema().await.unwrap();
    f.cleanup().await;
}
