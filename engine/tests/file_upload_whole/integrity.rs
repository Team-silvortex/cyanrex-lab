use super::{bytes::*, fixture::*, support::*};
use cyanrex_engine::{
    models::collaboration::*, services::file_upload_staging::UploadFileVerificationError,
};
use std::fs;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_complete_chunks_still_require_whole_digest_and_order() {
    let mut fixture = Fixture::new(4, 2).await;
    let correct = manifest(FILE_TRANSFER_CHUNK_BYTES + 17, 1);
    fixture.manifest = FileTransferManifest::new(
        scope(),
        correct.file_id(),
        correct.version_id(),
        correct.display_name(),
        correct.byte_length(),
        correct.chunk_sha256().to_vec(),
        "a".repeat(64).parse().unwrap(),
    )
    .unwrap();
    assert_ne!(fixture.manifest.sha256(), correct.sha256());
    fixture
        .source
        .reserve(upload(1), owner(1), account(1), &fixture.manifest)
        .await
        .unwrap();
    stage_all(&fixture).await;
    assert_eq!(tree(&fixture.root).len(), 2);
    // Both chunk stage operations succeeded against their correct digests; only the whole is wrong.
    bad(&fixture, UploadFileVerificationError::Unavailable).await;
    fixture.pool_restored().await;
    fixture.cleanup().await;

    let fixture = Fixture::ready(FILE_TRANSFER_CHUNK_BYTES * 2).await;
    stage_all(&fixture).await;
    good(&fixture, &fixture.adapter).await;
    let entries = tree(&fixture.root);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].length, entries[1].length);
    assert_ne!(entries[0].digest, entries[1].digest);
    let first = fixture.root.join(&entries[0].name);
    let second = fixture.root.join(&entries[1].name);
    let saved = fixture.directory.join("swapped-first");
    fs::rename(&first, &saved).unwrap();
    fs::rename(&second, &first).unwrap();
    fs::rename(&saved, &second).unwrap();
    bad(&fixture, UploadFileVerificationError::Unavailable).await;
    fs::rename(&first, &saved).unwrap();
    fs::rename(&second, &first).unwrap();
    fs::rename(&saved, &second).unwrap();
    good(&fixture, &fixture.adapter).await;
    fixture.pool_restored().await;
    fixture.cleanup().await;
}
