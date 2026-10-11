use super::{fixture::*, support::*};
use cyanrex_engine::{
    models::collaboration::FileTransferManifest,
    services::{
        file_upload_publication::FileUploadPublicationStoreError,
        file_upload_staging::{
            FileUploadPublicationError, FileUploadPublicationState, FileUploadPublisher,
        },
    },
};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_upload_version_uniqueness_and_global_capacity_include_empty_files() {
    let f = Fixture::with_limit(0, 1).await;
    publish(&f, &f.publisher, 1, &f.manifest).await;
    let m = manifest(0, 2);
    f.source
        .reserve(upload(2), owner(2), account(2), &m)
        .await
        .unwrap();
    let before = snapshot_all(&f).await;
    for (seed, error) in [
        (1, FileUploadPublicationStoreError::Conflict),
        (2, FileUploadPublicationStoreError::CapacityExhausted),
    ] {
        let mut attempt = f
            .publisher
            .prepare(upload(seed), owner(seed), account(seed))
            .await
            .unwrap();
        assert_eq!(
            f.publisher.publish(&mut attempt).await.unwrap_err(),
            FileUploadPublicationError::Publication(error)
        );
        assert_eq!(attempt.state(), FileUploadPublicationState::Unknown);
        assert_eq!(snapshot_all(&f).await, before);
    }
    f.cleanup().await;

    let f = Fixture::ready(17).await;
    stage_all(&f).await;
    publish(&f, &f.publisher, 1, &f.manifest).await;
    // A different legitimate source incarnation supplies the exact same file/version under a new
    // upload. This reaches publication UNIQUE, rather than failing old source intent uniqueness.
    let mut other = Fixture::new(4, 2, 16, 8).await;
    other.journal.install_empty_namespace().await.unwrap();
    other.publication.install_empty_namespace().await.unwrap();
    other.manifest = f.manifest.clone();
    other
        .source
        .reserve(upload(2), owner(2), account(2), &other.manifest)
        .await
        .unwrap();
    stage(&other, 2, &other.manifest).await;
    let publisher = FileUploadPublisher::new(
        other.local.clone(),
        other.journal.clone(),
        f.publication.clone(),
    )
    .unwrap();
    let before = snapshot_all(&f).await;
    let other_before = snapshot_all(&other).await;
    let mut attempt = publisher
        .prepare(upload(2), owner(2), account(2))
        .await
        .unwrap();
    assert_eq!(
        publisher.publish(&mut attempt).await.unwrap_err(),
        FileUploadPublicationError::Publication(FileUploadPublicationStoreError::Conflict)
    );
    assert_eq!(snapshot_all(&f).await, before);
    assert_eq!(snapshot_all(&other).await, other_before);
    publish(&other, &other.publisher, 2, &other.manifest).await;
    let variant = manifest(17, 3);
    let newer = FileTransferManifest::new(
        scope(),
        f.manifest.file_id(),
        variant.version_id(),
        "new-version.bin",
        17,
        variant.chunk_sha256().to_vec(),
        variant.sha256().clone(),
    )
    .unwrap();
    f.source
        .reserve(upload(3), owner(3), account(3), &newer)
        .await
        .unwrap();
    stage(&f, 3, &newer).await;
    publish(&f, &f.publisher, 3, &newer).await;
    other.cleanup().await;
    f.cleanup().await;
}
