use super::fixture::*;
use cyanrex_engine::services::file_upload_store::FileUploadStoreError;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_byte_limits_apply_globally_and_per_principal_across_generations() {
    let fixture = Fixture::ready(quota_limits(12, 8, 16, 8)).await;
    fixture
        .store
        .reserve(upload(1), owner(1), account(1), &manifest(5, 1))
        .await
        .unwrap();
    fixture
        .store
        .reserve(upload(2), owner(1), account(2), &manifest(3, 2))
        .await
        .unwrap();
    let before = fixture.rows().await;
    assert_eq!(
        fixture
            .store
            .reserve(upload(3), owner(1), account(3), &manifest(1, 3))
            .await,
        Err(FileUploadStoreError::QuotaExceeded),
        "new account generation must not reset owner bytes"
    );
    assert_eq!(fixture.rows().await, before);
    fixture
        .store
        .reserve(upload(4), owner(2), account(4), &manifest(4, 4))
        .await
        .unwrap();
    let before = fixture.rows().await;
    assert_eq!(
        fixture
            .store
            .reserve(upload(5), owner(3), account(5), &manifest(1, 5))
            .await,
        Err(FileUploadStoreError::QuotaExceeded),
        "other owner remains subject to installation-wide bytes"
    );
    assert_eq!(fixture.rows().await, before);
    let (reopened, pool) = fixture.reopen().await;
    assert_eq!(
        reopened
            .reserve(upload(6), owner(4), account(6), &manifest(1, 6))
            .await,
        Err(FileUploadStoreError::QuotaExceeded)
    );
    assert_eq!(fixture.rows().await, before);
    assert!(reopened
        .get(upload(2), owner(1), account(2))
        .await
        .unwrap()
        .is_some());
    assert_eq!(fixture.rows().await, before);
    drop(reopened);
    pool.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_empty_files_consume_upload_counts_without_reserving_bytes() {
    let fixture = Fixture::ready(quota_limits(0, 0, 3, 2)).await;
    for (id, generation) in [(1, 1), (2, 2)] {
        let reserved = fixture
            .store
            .reserve(upload(id), owner(1), account(generation), &manifest(0, id))
            .await
            .unwrap();
        assert_eq!(reserved.manifest().byte_length(), 0);
        assert!(reserved.manifest().chunk_sha256().is_empty());
    }
    let before = fixture.rows().await;
    assert_eq!(
        fixture
            .store
            .reserve(upload(3), owner(1), account(3), &manifest(0, 3))
            .await,
        Err(FileUploadStoreError::QuotaExceeded),
        "empty files consume per-principal count across generations"
    );
    assert_eq!(fixture.rows().await, before);
    fixture
        .store
        .reserve(upload(4), owner(2), account(4), &manifest(0, 4))
        .await
        .unwrap();
    let before = fixture.rows().await;
    assert_eq!(
        fixture
            .store
            .reserve(upload(5), owner(3), account(5), &manifest(0, 5))
            .await,
        Err(FileUploadStoreError::QuotaExceeded),
        "empty files consume global count"
    );
    assert_eq!(fixture.rows().await, before);
    assert_eq!(
        fixture
            .store
            .reserve(upload(6), owner(3), account(6), &manifest(1, 6))
            .await,
        Err(FileUploadStoreError::QuotaExceeded)
    );
    assert_eq!(fixture.rows().await, before);
    for (id, actor, generation) in [(1, 1, 1), (2, 1, 2), (4, 2, 4)] {
        assert!(fixture
            .store
            .get(upload(id), owner(actor), account(generation))
            .await
            .unwrap()
            .is_some());
        assert_eq!(fixture.rows().await, before);
    }
    fixture.cleanup().await;
}
