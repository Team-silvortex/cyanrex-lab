use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_access_failed_schema_install_rolls_back_without_repairing_legacy_data() {
    let f = Fixture::ready().await;
    f.sql("CREATE TABLE collaboration_deployment_grants (sentinel TEXT)")
        .await;
    f.sql("INSERT INTO collaboration_deployment_grants VALUES ('synthetic sentinel')")
        .await;
    assert!(f.store.install_access_schema().await.is_err());
    let row = query("SELECT to_regclass('collaboration_access_schema')::TEXT AS metadata, to_regclass('collaboration_memberships')::TEXT AS members")
        .fetch_one(&f.pool).await.unwrap();
    assert_eq!(row.get::<Option<String>, _>("metadata"), None);
    assert_eq!(row.get::<Option<String>, _>("members"), None);
    assert_eq!(f.count("collaboration_deployment_grants").await, 1);
    assert_eq!(f.count("collaboration_workspaces").await, 1);
    f.sql("DROP TABLE collaboration_deployment_grants").await;
    f.store.install_access_schema().await.unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_access_revision_exhaustion_and_ambiguous_persisted_roles_fail_closed() {
    let f = ready().await;
    let original = member(&f, "teacher-one", "cyanrex.teaching.teacher", true).await;
    let principal = original.policy.membership.principal_id;
    f.sql("UPDATE collaboration_memberships SET revision = 9007199254740991")
        .await;
    f.sql("UPDATE collaboration_deployment_grants SET revision = 9007199254740991")
        .await;
    let last = f
        .store
        .legacy_access(scope(), principal)
        .await
        .unwrap()
        .unwrap();
    let mut revoked = original.policy.clone();
    revoked.deployment_granted = false;
    assert_eq!(
        f.store
            .replace_legacy_access(Some(last.revision), &revoked)
            .await,
        Err(IdentityStoreError::InvalidRecord)
    );
    assert_eq!(
        f.store.legacy_access(scope(), principal).await.unwrap(),
        Some(last)
    );
    f.sql("UPDATE collaboration_memberships SET role_refs = ARRAY['cyanrex.teaching.teacher', 'cyanrex.teaching.learner']").await;
    assert_eq!(
        f.store.legacy_access(scope(), principal).await,
        Err(IdentityStoreError::InvalidRecord)
    );
    assert!(f
        .store
        .preview_legacy_access(
            scope(),
            actor(principal),
            LegacyAction::ManageDeployment,
            &deployment()
        )
        .await
        .is_err());
    f.sql("UPDATE collaboration_memberships SET role_refs = ARRAY['cyanrex.teaching.teacher', 'cyanrex.teaching.teacher']").await;
    assert_eq!(
        f.store.legacy_access(scope(), principal).await,
        Err(IdentityStoreError::InvalidRecord)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_access_altered_write_postcondition_is_not_acknowledged() {
    let f = ready().await;
    let original = member(&f, "teacher-one", "cyanrex.teaching.teacher", true).await;
    let principal = original.policy.membership.principal_id;
    f.sql("CREATE FUNCTION alter_grant() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN NEW.status := 'active'; RETURN NEW; END $$").await;
    f.sql("CREATE TRIGGER alter_grant BEFORE UPDATE ON collaboration_deployment_grants FOR EACH ROW EXECUTE FUNCTION alter_grant()").await;
    let mut revoked = original.policy.clone();
    revoked.deployment_granted = false;
    assert_eq!(
        f.store
            .replace_legacy_access(Some(original.revision), &revoked)
            .await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    assert_eq!(
        f.store.legacy_access(scope(), principal).await.unwrap(),
        Some(original.clone())
    );
    f.sql("DROP TRIGGER alter_grant ON collaboration_deployment_grants")
        .await;
    f.store
        .replace_legacy_access(Some(original.revision), &revoked)
        .await
        .unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_access_partial_or_suppressed_writes_never_commit() {
    let f = ready().await;
    let principal = f.bind().await.binding.principal_id;
    let proposed = policy(principal, "cyanrex.teaching.teacher", true);
    f.sql("CREATE FUNCTION suppress_grant() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NULL; END $$").await;
    f.sql("CREATE TRIGGER suppress_grant BEFORE INSERT ON collaboration_deployment_grants FOR EACH ROW EXECUTE FUNCTION suppress_grant()").await;
    assert!(f
        .store
        .replace_legacy_access(None, &proposed)
        .await
        .is_err());
    assert_eq!(f.count("collaboration_memberships").await, 0);
    f.sql("DROP TRIGGER suppress_grant ON collaboration_deployment_grants")
        .await;
    let original = f
        .store
        .replace_legacy_access(None, &proposed)
        .await
        .unwrap();
    let mut revoked = proposed.clone();
    revoked.deployment_granted = false;
    f.sql("CREATE TRIGGER suppress_grant BEFORE UPDATE ON collaboration_deployment_grants FOR EACH ROW EXECUTE FUNCTION suppress_grant()").await;
    assert!(f
        .store
        .replace_legacy_access(Some(original.revision), &revoked)
        .await
        .is_err());
    assert_eq!(
        f.store.legacy_access(scope(), principal).await.unwrap(),
        Some(original.clone())
    );
    f.sql("DROP TRIGGER suppress_grant ON collaboration_deployment_grants")
        .await;
    f.sql("CREATE TRIGGER suppress_member BEFORE UPDATE ON collaboration_memberships FOR EACH ROW EXECUTE FUNCTION suppress_grant()").await;
    assert!(f
        .store
        .replace_legacy_access(Some(original.revision), &revoked)
        .await
        .is_err());
    assert_eq!(
        f.store.legacy_access(scope(), principal).await.unwrap(),
        Some(original.clone())
    );
    f.sql("DROP TRIGGER suppress_member ON collaboration_memberships")
        .await;
    f.store
        .replace_legacy_access(Some(original.revision), &revoked)
        .await
        .unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_access_deferred_commit_failure_preserves_prior_policy() {
    let f = ready().await;
    let original = member(&f, "teacher-one", "cyanrex.teaching.teacher", true).await;
    let principal = original.policy.membership.principal_id;
    f.sql("CREATE FUNCTION fail_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic commit fault'; END $$").await;
    f.sql("CREATE CONSTRAINT TRIGGER fail_commit AFTER UPDATE ON collaboration_deployment_grants DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_commit()").await;
    let mut revoked = original.policy.clone();
    revoked.deployment_granted = false;
    assert!(f
        .store
        .replace_legacy_access(Some(original.revision), &revoked)
        .await
        .is_err());
    assert_eq!(
        f.store.legacy_access(scope(), principal).await.unwrap(),
        Some(original.clone())
    );
    f.sql("DROP TRIGGER fail_commit ON collaboration_deployment_grants")
        .await;
    f.store
        .replace_legacy_access(Some(original.revision), &revoked)
        .await
        .unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_access_corrupt_or_missing_state_never_uses_cached_allow() {
    let f = ready().await;
    let original = member(&f, "teacher-one", "cyanrex.teaching.teacher", true).await;
    let principal = original.policy.membership.principal_id;
    assert!(f
        .store
        .preview_legacy_access(
            scope(),
            actor(principal),
            LegacyAction::ManageDeployment,
            &deployment()
        )
        .await
        .unwrap());
    f.sql("UPDATE collaboration_deployment_grants SET revision = 2")
        .await;
    assert_eq!(
        f.store.legacy_access(scope(), principal).await,
        Err(IdentityStoreError::InvalidRecord)
    );
    assert!(f
        .store
        .preview_legacy_access(
            scope(),
            actor(principal),
            LegacyAction::ManageDeployment,
            &deployment()
        )
        .await
        .is_err());
    f.sql("UPDATE collaboration_deployment_grants SET revision = 1")
        .await;
    f.sql("UPDATE collaboration_principals SET status = 'disabled'")
        .await;
    assert!(!f
        .store
        .preview_legacy_access(
            scope(),
            actor(principal),
            LegacyAction::ManageDeployment,
            &deployment()
        )
        .await
        .unwrap());
    assert_eq!(
        f.store
            .replace_legacy_access(Some(original.revision), &original.policy)
            .await,
        Err(IdentityStoreError::IdentityInactive)
    );
    f.sql("UPDATE collaboration_principals SET status = 'active'")
        .await;
    f.sql("ALTER TABLE collaboration_deployment_grants RENAME TO missing_grants")
        .await;
    assert!(f
        .store
        .preview_legacy_access(
            scope(),
            actor(principal),
            LegacyAction::ManageDeployment,
            &deployment()
        )
        .await
        .is_err());
    f.sql("ALTER TABLE missing_grants RENAME TO collaboration_deployment_grants")
        .await;
    assert!(f
        .store
        .preview_legacy_access(
            scope(),
            actor(principal),
            LegacyAction::ManageDeployment,
            &deployment()
        )
        .await
        .unwrap());
    f.sql("DELETE FROM collaboration_deployment_grants").await;
    assert_eq!(
        f.store.legacy_access(scope(), principal).await,
        Err(IdentityStoreError::InvalidRecord)
    );
    assert!(f
        .store
        .replace_legacy_access(Some(original.revision), &original.policy)
        .await
        .is_err());
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_access_cancelled_change_and_waiting_preview_do_not_publish_stale_allow() {
    let f = ready().await;
    let original = member(&f, "teacher-one", "cyanrex.teaching.teacher", true).await;
    let principal = original.policy.membership.principal_id;
    let mut blocker = f.pool.begin().await.unwrap();
    query("SELECT authority_id FROM collaboration_authorities FOR UPDATE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let store = f.store.clone();
    let mut revoked = original.policy.clone();
    revoked.deployment_granted = false;
    let cancelled = tokio::spawn(async move {
        store
            .replace_legacy_access(Some(original.revision), &revoked)
            .await
    });
    f.wait_blocked().await;
    cancelled.abort();
    assert!(cancelled.await.unwrap_err().is_cancelled());
    blocker.rollback().await.unwrap();
    f.drain().await;
    assert_eq!(
        f.store.legacy_access(scope(), principal).await.unwrap(),
        Some(original.clone())
    );
    let mut blocker = f.pool.begin().await.unwrap();
    query("SELECT authority_id FROM collaboration_authorities FOR UPDATE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let store = f.store.clone();
    let preview = tokio::spawn(async move {
        store
            .preview_legacy_access(
                scope(),
                actor(principal),
                LegacyAction::ManageDeployment,
                &deployment(),
            )
            .await
    });
    f.wait_blocked().await;
    query("UPDATE collaboration_deployment_grants SET status = 'revoked', revision = 2")
        .execute(&mut *blocker)
        .await
        .unwrap();
    query("UPDATE collaboration_memberships SET revision = 2")
        .execute(&mut *blocker)
        .await
        .unwrap();
    blocker.commit().await.unwrap();
    assert!(!preview.await.unwrap().unwrap());
    f.cleanup().await;
}
