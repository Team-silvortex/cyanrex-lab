use super::{fixture::*, *};

async fn closed() -> PgPool {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    pool
}
#[tokio::test]
async fn publisher_grant_store_valid_commands_dispatch_to_explicit_closed_pool() {
    let store = FilePublisherGrantStore::new(
        closed().await,
        scope(),
        Uuid::new_v4(),
        "private_grants",
        16,
    )
    .unwrap();
    assert_eq!(
        store.install_empty_namespace().await,
        Err(FilePublisherGrantStoreError::StorageUnavailable)
    );
    assert_eq!(
        store
            .issue(grant(1), account(1), account(2), validity())
            .await,
        Err(FilePublisherGrantStoreError::StorageUnavailable)
    );
    assert_eq!(
        store.get(grant(1), account(2)).await,
        Err(FilePublisherGrantStoreError::StorageUnavailable)
    );
    assert_eq!(
        store.revoke(grant(1), account(3), revision(1)).await,
        Err(FilePublisherGrantStoreError::StorageUnavailable)
    );
}
#[tokio::test]
async fn publisher_grant_configuration_is_bounded_and_validated_without_io() {
    assert_eq!(MAX_FILE_PUBLISHER_GRANTS, 256);
    let pool = closed().await;
    for namespace in ["private_grants", "_private_grants", &"a".repeat(63)] {
        for max in [1, 256] {
            assert!(FilePublisherGrantStore::new(
                pool.clone(),
                scope(),
                Uuid::new_v4(),
                namespace,
                max
            )
            .is_ok());
        }
    }
    for namespace in [
        "",
        "public",
        "information_schema",
        "pg_catalog",
        "pg_private",
        "Upper",
        "1prefix",
        "a.b",
        "a b",
        "quote\"",
        &"a".repeat(64),
    ] {
        assert!(matches!(
            FilePublisherGrantStore::new(pool.clone(), scope(), Uuid::new_v4(), namespace, 16),
            Err(FilePublisherGrantStoreError::InvalidInput)
        ));
    }
    for max in [0, 257, u32::MAX] {
        assert!(matches!(
            FilePublisherGrantStore::new(
                pool.clone(),
                scope(),
                Uuid::new_v4(),
                "private_grants",
                max
            ),
            Err(FilePublisherGrantStoreError::InvalidInput)
        ));
    }
    for installation in [Uuid::nil(), Uuid::from_u128(1)] {
        assert!(matches!(
            FilePublisherGrantStore::new(pool.clone(), scope(), installation, "private_grants", 16),
            Err(FilePublisherGrantStoreError::InvalidInput)
        ));
    }
}
#[tokio::test]
async fn publisher_grant_foreign_attribution_fails_before_storage() {
    let store = FilePublisherGrantStore::new(
        closed().await,
        scope(),
        Uuid::new_v4(),
        "private_grants",
        16,
    )
    .unwrap();
    let subject = FileAccountRef::new(
        PrincipalRef {
            authority_id: Uuid::from_u128(9001).try_into().unwrap(),
            principal_id: account(2).principal().principal_id,
        },
        account(2).account_id(),
    );
    assert_eq!(
        store.issue(grant(1), subject, account(2), validity()).await,
        Err(FilePublisherGrantStoreError::ScopeMismatch)
    );
    assert_eq!(
        store.issue(grant(1), account(1), subject, validity()).await,
        Err(FilePublisherGrantStoreError::ScopeMismatch)
    );
    assert_eq!(
        store.get(grant(1), subject).await,
        Err(FilePublisherGrantStoreError::ScopeMismatch)
    );
    assert_eq!(
        store.revoke(grant(1), subject, revision(1)).await,
        Err(FilePublisherGrantStoreError::ScopeMismatch)
    );
}
