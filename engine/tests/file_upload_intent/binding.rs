use super::fixture::*;
use cyanrex_engine::services::file_upload_store::FileUploadStoreError;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_owner_generation_scope_and_complete_manifest_binding_are_exact() {
    let fixture = Fixture::ready(limits()).await;
    let expected = manifest(11, 1);
    let stored = fixture
        .store
        .reserve(upload(1), owner(1), account(1), &expected)
        .await
        .unwrap();
    assert_eq!(stored.upload_id(), upload(1));
    assert_eq!(stored.owner(), owner(1));
    assert_eq!(stored.account_id(), account(1));
    assert_eq!(stored.manifest(), &expected);
    let before = fixture.rows().await;
    for (id, actor, generation) in [
        (upload(1), owner(2), account(1)),
        (upload(1), owner(1), account(2)),
        (upload(999), owner(1), account(1)),
    ] {
        assert_eq!(
            fixture.store.get(id, actor, generation).await.unwrap(),
            None
        );
        assert_eq!(fixture.rows().await, before);
    }
    let mut foreign_owner = owner(1);
    foreign_owner.authority_id = uuid::Uuid::from_u128(901).try_into().unwrap();
    assert_eq!(
        fixture
            .store
            .get(upload(1), foreign_owner, account(1))
            .await,
        Err(FileUploadStoreError::ScopeMismatch)
    );
    assert_eq!(
        fixture
            .store
            .reserve(upload(2), foreign_owner, account(1), &manifest(11, 2))
            .await,
        Err(FileUploadStoreError::ScopeMismatch)
    );
    for authority in [true, false] {
        let mut foreign = scope();
        if authority {
            foreign.authority_id = uuid::Uuid::from_u128(902).try_into().unwrap();
        } else {
            foreign.workspace_id = uuid::Uuid::from_u128(903).try_into().unwrap();
        }
        let candidate = changed(&expected, "workspace", serde_json::json!(foreign));
        assert_eq!(
            fixture
                .store
                .reserve(upload(3), owner(1), account(1), &candidate)
                .await,
            Err(FileUploadStoreError::ScopeMismatch)
        );
        assert_eq!(fixture.rows().await, before);
    }
    assert_eq!(
        fixture
            .store
            .get(upload(1), owner(1), account(1))
            .await
            .unwrap(),
        Some(stored)
    );
    assert_eq!(fixture.rows().await, before);
    fixture.cleanup().await;
}
