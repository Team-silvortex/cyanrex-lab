use super::fixture::*;
use cyanrex_engine::services::file_upload_store::FileUploadStoreError;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_duplicate_upload_and_file_version_are_conflicts_before_quota() {
    let fixture = Fixture::ready(quota_limits(11, 11, 1, 1)).await;
    let expected = manifest(11, 1);
    let first = fixture
        .store
        .reserve(upload(1), owner(1), account(1), &expected)
        .await
        .unwrap();
    let before = fixture.rows().await;
    let (reopened, pool) = fixture.reopen().await;
    for (id, candidate) in [
        (upload(1), expected.clone()),
        (upload(1), manifest(11, 2)),
        (upload(2), expected.clone()),
    ] {
        assert_eq!(
            reopened.reserve(id, owner(1), account(1), &candidate).await,
            Err(FileUploadStoreError::Conflict)
        );
        assert_eq!(fixture.rows().await, before);
    }
    // Same file with a different version is a new allocation, not the existing coordinate.
    let another_version = changed(
        &expected,
        "version_id",
        serde_json::json!(uuid::Uuid::from_u128(900).to_string()),
    );
    assert_eq!(
        reopened
            .reserve(upload(3), owner(1), account(1), &another_version)
            .await,
        Err(FileUploadStoreError::QuotaExceeded)
    );
    assert_eq!(fixture.rows().await, before);
    assert_eq!(
        reopened.get(upload(1), owner(1), account(1)).await.unwrap(),
        Some(first.clone())
    );
    drop(first);
    assert_eq!(
        fixture.rows().await,
        before,
        "dropping a receipt never releases quota"
    );
    drop(reopened);
    pool.close().await;
    fixture.cleanup().await;

    let healthy = Fixture::ready(quota_limits(22, 22, 2, 2)).await;
    let previous = healthy
        .store
        .reserve(upload(1), owner(1), account(1), &expected)
        .await
        .unwrap();
    let next = healthy
        .store
        .reserve(upload(3), owner(1), account(2), &another_version)
        .await
        .unwrap();
    assert_eq!(previous.manifest().file_id(), next.manifest().file_id());
    assert_ne!(
        previous.manifest().version_id(),
        next.manifest().version_id()
    );
    let before = healthy.rows().await;
    assert_eq!(before[1].len(), 2);
    assert_eq!(
        healthy
            .store
            .get(upload(1), owner(1), account(1))
            .await
            .unwrap(),
        Some(previous)
    );
    assert_eq!(
        healthy
            .store
            .get(upload(3), owner(1), account(2))
            .await
            .unwrap(),
        Some(next)
    );
    assert_eq!(healthy.rows().await, before);
    healthy.cleanup().await;
}
