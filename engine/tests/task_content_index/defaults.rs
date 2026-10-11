use super::*;

pub(super) fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: "1223c6e9-823d-4cea-998a-a92f019b35fc".parse().unwrap(),
        workspace_id: "51f0325b-ea1b-42f9-9b39-71d063b8d327".parse().unwrap(),
    }
}

pub(super) fn owner() -> PrincipalRef {
    PrincipalRef {
        authority_id: scope().authority_id,
        principal_id: "d7ed9514-330f-4cd7-9a30-32741ee8d0cf".parse().unwrap(),
    }
}

pub(super) async fn closed_pool() -> PgPool {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    pool
}

#[tokio::test]
async fn valid_index_request_reaches_closed_storage_without_fallback() {
    let store = TaskContentIndexStore::new(closed_pool().await, scope());
    assert_eq!(
        store.list(owner(), None, None).await,
        Err(TaskStoreError::StorageUnavailable)
    );
}

#[tokio::test]
async fn index_limits_are_bounded_before_any_storage_access() {
    let store = TaskContentIndexStore::new(closed_pool().await, scope());
    for limit in [0, 33, usize::MAX] {
        assert_eq!(
            store.list(owner(), None, Some(limit)).await,
            Err(TaskStoreError::InvalidInput)
        );
    }
    for limit in [1, 20, 32] {
        assert_eq!(
            store.list(owner(), None, Some(limit)).await,
            Err(TaskStoreError::StorageUnavailable)
        );
    }
}

#[tokio::test]
async fn index_owner_and_cursor_scope_are_checked_but_cursor_is_not_a_grant() {
    let store = TaskContentIndexStore::new(closed_pool().await, scope());
    let foreign = PrincipalRef {
        authority_id: task(900).task_id.as_uuid().try_into().unwrap(),
        ..owner()
    };
    assert_eq!(
        store.list(foreign, None, None).await,
        Err(TaskStoreError::ScopeMismatch)
    );
    for changed_scope in [
        WorkspaceRef {
            authority_id: foreign.authority_id,
            ..scope()
        },
        WorkspaceRef {
            workspace_id: task(901).task_id.as_uuid().try_into().unwrap(),
            ..scope()
        },
    ] {
        let after = TaskRef {
            workspace: changed_scope,
            ..task(1)
        };
        assert_eq!(
            store.list(owner(), Some(after), None).await,
            Err(TaskStoreError::ScopeMismatch)
        );
    }
    // An arbitrary same-workspace position must reach storage, not authorize or lookup that ID.
    assert_eq!(
        store.list(owner(), Some(task(999)), None).await,
        Err(TaskStoreError::StorageUnavailable)
    );
}

#[tokio::test]
async fn explicitly_frozen_index_configuration_needs_no_provider_or_artifact_service() {
    let (metadata, values) = definitions();
    let store = TaskContentIndexStore::from_definitions(closed_pool().await, scope(), &metadata);
    let store = store.clone();
    fn shareable<T: Send + Sync>(_: &T) {}
    shareable(&store);
    assert!(values[0].evidence_schema.is_some());
    assert!(values[1].evidence_schema.is_none());
    assert_ne!(values[0].reference.package, values[2].reference.package);
    drop(metadata);
    assert_eq!(
        store.list(owner(), None, None).await,
        Err(TaskStoreError::StorageUnavailable)
    );
}
