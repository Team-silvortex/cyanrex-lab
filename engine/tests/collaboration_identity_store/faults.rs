use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_identity_failed_bootstrap_leaves_no_partial_authority_or_workspace() {
    let f = Fixture::new().await;
    f.store.install_schema().await.unwrap();
    f.sql("CREATE FUNCTION mapping_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NULL; END; $$").await;
    f.sql("CREATE TRIGGER mapping_fault BEFORE INSERT ON collaboration_legacy_workspaces FOR EACH ROW EXECUTE FUNCTION mapping_fault()").await;
    assert_eq!(
        f.store.provision_legacy_workspace(scope()).await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    f.drain().await;
    for table in [
        "collaboration_authorities",
        "collaboration_workspaces",
        "collaboration_legacy_workspaces",
    ] {
        assert_eq!(f.count(table).await, 0);
    }
    f.sql("DROP TRIGGER mapping_fault ON collaboration_legacy_workspaces")
        .await;
    f.store.provision_legacy_workspace(scope()).await.unwrap();
    f.cleanup().await;
}

async fn binding_fault(action: &str, deferred: bool) {
    let f = Fixture::ready().await;
    f.sql(&format!("CREATE FUNCTION binding_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN {action} END; $$")).await;
    let trigger = if deferred {
        "CREATE CONSTRAINT TRIGGER binding_fault AFTER INSERT ON collaboration_legacy_identities DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION binding_fault()"
    } else {
        "CREATE TRIGGER binding_fault BEFORE INSERT ON collaboration_legacy_identities FOR EACH ROW EXECUTE FUNCTION binding_fault()"
    };
    f.sql(trigger).await;
    assert_eq!(
        f.store
            .bind_legacy_account(scope(), &username(), account())
            .await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    f.drain().await;
    assert_eq!(f.count("collaboration_principals").await, 0);
    assert_eq!(f.count("collaboration_legacy_identities").await, 0);
    f.sql("DROP TRIGGER binding_fault ON collaboration_legacy_identities")
        .await;
    f.bind().await;
    assert_eq!(f.count("collaboration_principals").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_identity_binding_failure_rolls_back_the_principal() {
    binding_fault("RAISE EXCEPTION 'synthetic binding failure';", false).await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_identity_suppressed_binding_is_not_success() {
    binding_fault("RETURN NULL;", false).await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_identity_commit_failure_is_not_success() {
    binding_fault("RAISE EXCEPTION 'synthetic commit failure';", true).await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_identity_retirement_failure_keeps_both_principal_and_binding_active() {
    let f = Fixture::ready().await;
    let original = f.bind().await;
    f.sql("CREATE FUNCTION retire_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NULL; END; $$").await;
    f.sql("CREATE TRIGGER retire_fault BEFORE UPDATE ON collaboration_principals FOR EACH ROW EXECUTE FUNCTION retire_fault()").await;
    assert_eq!(
        f.store
            .retire_legacy_account(
                scope(),
                &username(),
                account(),
                original.binding.principal_id
            )
            .await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    f.drain().await;
    assert_eq!(
        f.store
            .lookup_legacy_account(scope(), &username(), account())
            .await
            .unwrap(),
        Some(original.clone())
    );
    f.sql("DROP TRIGGER retire_fault ON collaboration_principals")
        .await;
    f.store
        .retire_legacy_account(
            scope(),
            &username(),
            account(),
            original.binding.principal_id,
        )
        .await
        .unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_identity_retirement_commit_failure_preserves_the_active_mapping() {
    let f = Fixture::ready().await;
    let original = f.bind().await;
    f.sql("CREATE FUNCTION retire_commit_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic retire commit failure'; END; $$").await;
    f.sql("CREATE CONSTRAINT TRIGGER retire_commit_fault AFTER UPDATE ON collaboration_principals DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION retire_commit_fault()").await;
    assert_eq!(
        f.store
            .retire_legacy_account(
                scope(),
                &username(),
                account(),
                original.binding.principal_id
            )
            .await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    assert_eq!(
        f.store
            .lookup_legacy_account(scope(), &username(), account())
            .await
            .unwrap(),
        Some(original.clone())
    );
    f.sql("DROP TRIGGER retire_commit_fault ON collaboration_principals")
        .await;
    f.store
        .retire_legacy_account(
            scope(),
            &username(),
            account(),
            original.binding.principal_id,
        )
        .await
        .unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_identity_cancelled_binding_rolls_back_without_publishing_an_id() {
    let f = Fixture::ready().await;
    let key = 765435;
    let mut blocker = f.admin.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock($1)")
        .bind(key as i64)
        .execute(&mut *blocker)
        .await
        .unwrap();
    f.sql(&format!("CREATE FUNCTION pause_binding() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock({key}); RETURN NEW; END; $$")).await;
    f.sql("CREATE TRIGGER pause_binding BEFORE INSERT ON collaboration_legacy_identities FOR EACH ROW EXECUTE FUNCTION pause_binding()").await;
    let store = f.store.clone();
    let pending = tokio::spawn(async move {
        store
            .bind_legacy_account(scope(), &username(), account())
            .await
    });
    f.wait_blocked().await;
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    blocker.commit().await.unwrap();
    f.drain().await;
    assert_eq!(f.count("collaboration_principals").await, 0);
    assert_eq!(f.count("collaboration_legacy_identities").await, 0);
    f.sql("DROP TRIGGER pause_binding ON collaboration_legacy_identities")
        .await;
    f.bind().await;
    f.cleanup().await;
}
