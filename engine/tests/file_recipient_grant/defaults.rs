use super::{fixture::*, *};
async fn closed() -> PgPool {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    pool
}
#[tokio::test]
async fn recipient_grant_valid_operations_dispatch_to_explicit_closed_pool() {
    let store = FileRecipientGrantStore::new(
        closed().await,
        scope(),
        Uuid::new_v4(),
        "recipient_grants",
        16,
    )
    .unwrap();
    assert_eq!(
        store.install_empty_namespace().await,
        Err(FileRecipientGrantStoreError::StorageUnavailable)
    );
    assert_eq!(
        store
            .issue(
                grant(1),
                account(1),
                file(1),
                account(2),
                account(3),
                validity()
            )
            .await,
        Err(FileRecipientGrantStoreError::StorageUnavailable)
    );
    assert_eq!(
        store.get(grant(1), account(3)).await,
        Err(FileRecipientGrantStoreError::StorageUnavailable)
    );
    assert_eq!(
        store.revoke(grant(1), account(4), revision(1)).await,
        Err(FileRecipientGrantStoreError::StorageUnavailable)
    );
}
#[tokio::test]
async fn recipient_grant_configuration_admission_is_bounded_and_inert() {
    assert_eq!(MAX_FILE_RECIPIENT_GRANTS, 256);
    let pool = closed().await;
    for name in ["recipient_grants", "_recipient_grants", &"a".repeat(63)] {
        for max in [1, 256] {
            assert!(
                FileRecipientGrantStore::new(pool.clone(), scope(), Uuid::new_v4(), name, max)
                    .is_ok()
            );
        }
    }
    for name in [
        "",
        "public",
        "information_schema",
        "pg_catalog",
        "pg_private",
        "Upper",
        "1bad",
        "a.b",
        "a b",
        "quote\"",
        &"a".repeat(64),
    ] {
        assert!(matches!(
            FileRecipientGrantStore::new(pool.clone(), scope(), Uuid::new_v4(), name, 16),
            Err(FileRecipientGrantStoreError::InvalidInput)
        ));
    }
    for max in [0, 257, u32::MAX] {
        assert!(matches!(
            FileRecipientGrantStore::new(
                pool.clone(),
                scope(),
                Uuid::new_v4(),
                "recipient_grants",
                max
            ),
            Err(FileRecipientGrantStoreError::InvalidInput)
        ));
    }
    for id in [Uuid::nil(), Uuid::from_u128(1)] {
        assert!(matches!(
            FileRecipientGrantStore::new(pool.clone(), scope(), id, "recipient_grants", 16),
            Err(FileRecipientGrantStoreError::InvalidInput)
        ));
    }
}
#[tokio::test]
async fn recipient_grant_foreign_file_or_any_attribution_fails_before_io() {
    let store = FileRecipientGrantStore::new(
        closed().await,
        scope(),
        Uuid::new_v4(),
        "recipient_grants",
        16,
    )
    .unwrap();
    let foreign_scope = WorkspaceRef {
        authority_id: Uuid::from_u128(9001).try_into().unwrap(),
        ..scope()
    };
    let foreign = FileAccountRef::new(
        PrincipalRef {
            authority_id: foreign_scope.authority_id,
            ..account(1).principal()
        },
        account(1).account_id(),
    );
    for (issuer, publisher, recipient) in [
        (foreign, account(2), account(3)),
        (account(1), foreign, account(3)),
        (account(1), account(2), foreign),
    ] {
        assert_eq!(
            store
                .issue(grant(1), issuer, file(1), publisher, recipient, validity())
                .await,
            Err(FileRecipientGrantStoreError::ScopeMismatch)
        );
    }
    for s in [
        foreign_scope,
        WorkspaceRef {
            workspace_id: Uuid::from_u128(9002).try_into().unwrap(),
            ..scope()
        },
    ] {
        assert_eq!(
            store
                .issue(
                    grant(1),
                    account(1),
                    file_for(s, 1),
                    account(2),
                    account(3),
                    validity()
                )
                .await,
            Err(FileRecipientGrantStoreError::ScopeMismatch)
        );
    }
    assert_eq!(
        store.get(grant(1), foreign).await,
        Err(FileRecipientGrantStoreError::ScopeMismatch)
    );
    assert_eq!(
        store.revoke(grant(1), foreign, revision(1)).await,
        Err(FileRecipientGrantStoreError::ScopeMismatch)
    );
}
