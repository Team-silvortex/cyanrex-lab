use crate::{fixture::Fixture, support::*, *};

async fn publish_variant(f: &Fixture, seed: u128, change_file: bool) -> FileTransferManifest {
    let mut raw = serde_json::to_value(bytes::manifest(17, seed)).unwrap();
    if change_file {
        raw["version_id"] = serde_json::json!(f.sharing.base.manifest.version_id());
    } else {
        raw["file_id"] = serde_json::json!(f.sharing.base.manifest.file_id());
    }
    let manifest: FileTransferManifest = serde_json::from_value(raw).unwrap();
    let actor = f.sharing.actor();
    f.sharing
        .base
        .uploads
        .reserve(
            publication_fixture::upload(seed),
            actor.principal(),
            actor.account_id(),
            &manifest,
        )
        .await
        .unwrap();
    for index in 0..manifest.chunk_sha256().len() as u32 {
        let mut chunk = f
            .sharing
            .base
            .journaled
            .prepare(
                publication_fixture::upload(seed),
                actor.principal(),
                actor.account_id(),
                index,
            )
            .await
            .unwrap();
        drop(
            f.sharing
                .base
                .journaled
                .stage_chunk(
                    &mut chunk,
                    bytes::Pattern::new(
                        index as u64 * FILE_TRANSFER_CHUNK_BYTES,
                        manifest.chunk(index).unwrap().byte_length,
                    ),
                )
                .await
                .unwrap(),
        );
    }
    let mut attempt = f
        .sharing
        .base
        .publisher
        .prepare(
            publication_fixture::upload(seed),
            actor.principal(),
            actor.account_id(),
        )
        .await
        .unwrap();
    drop(
        auth(f)
            .source
            .publish_session_file_upload(
                &auth(f).learner_token,
                &f.sharing.base.workspace,
                grant_fixture::grant(1),
                &mut attempt,
            )
            .await
            .unwrap(),
    );
    manifest
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_download_exact_selectors_filter_same_publisher_foreign_corruption_before_decode()
{
    checked(Fixture::ready().await, |f| {
        Box::pin(async move {
            healthy(f).await;
            for (field, value) in [
                ("file_id", serde_json::json!(Uuid::new_v4())),
                ("version_id", serde_json::json!(Uuid::new_v4())),
                ("byte_length", serde_json::json!(18)),
                ("sha256", serde_json::json!("b".repeat(64))),
            ] {
                let mut r = f.request();
                let mut raw = serde_json::to_value(&r.expected_file).unwrap();
                raw[field] = value;
                r.expected_file = serde_json::from_value(raw).unwrap();
                assert_eq!(
                    read(f, &r).await,
                    Err(SessionFileDownloadError::PermissionDenied),
                    "isolated coordinate {field}"
                );
            }
            let mut missing = f.request();
            missing.upload_id = publication_fixture::upload(99);
            assert_eq!(
                read(f, &missing).await,
                Err(SessionFileDownloadError::PermissionDenied)
            );
            let actor = f.sharing.actor();
            for (seed, change_file) in [(2, true), (3, false)] {
                let other = publish_variant(f, seed, change_file).await;
                let mut foreign = f.request();
                foreign.upload_id = publication_fixture::upload(seed);
                assert_eq!(
                    read(f, &foreign).await,
                    Err(SessionFileDownloadError::PermissionDenied)
                );
                // Two REAL same-owner publications isolate EACH SQL predicate: same version /
                // different file, then same file / different version. Bytes and digest are identical.
                assert_eq!(
                    other.file_id() == f.sharing.base.manifest.file_id(),
                    !change_file
                );
                assert_eq!(
                    other.version_id() == f.sharing.base.manifest.version_id(),
                    change_file
                );
                let before = f.sharing.snapshot().await;
                query(&format!(
                    "UPDATE {}.{PUB_ROWS} SET manifest=$1 WHERE upload_id=$2",
                    f.sharing.base.publication_schema
                ))
                .bind(b"{".to_vec())
                .bind(publication_fixture::upload(seed).as_uuid())
                .execute(&auth(f).base.admin)
                .await
                .unwrap();
                let injected = f.sharing.snapshot().await;
                assert_eq!(
                    read(f, &foreign).await,
                    Err(SessionFileDownloadError::PermissionDenied),
                    "foreign same-owner body must be excluded before JSON decode"
                );
                foreign.expected_file = FileVersionRef::from_manifest(&other);
                assert_eq!(
                    read(f, &foreign).await,
                    Err(SessionFileDownloadError::PermissionDenied),
                    "client exact selector cannot rewrite the selected grant's original version"
                );
                assert_eq!(f.sharing.snapshot().await, injected);
                query(&format!(
                    "UPDATE {}.{PUB_ROWS} SET manifest=$1 WHERE upload_id=$2",
                    f.sharing.base.publication_schema
                ))
                .bind(serde_json::to_vec(&other).unwrap())
                .bind(publication_fixture::upload(seed).as_uuid())
                .execute(&auth(f).base.admin)
                .await
                .unwrap();
                assert_eq!(f.sharing.snapshot().await, before);
                healthy(f).await;
            }
            // Exact old publisher generation must remain part of selection even if the payload is bad.
            query(&format!(
                "UPDATE {}.{PUB_ROWS} SET account_id=$1,manifest=$2 WHERE upload_id=$3",
                f.sharing.base.publication_schema
            ))
            .bind(Uuid::new_v4())
            .bind(b"{".to_vec())
            .bind(publication_fixture::upload(1).as_uuid())
            .execute(&auth(f).base.admin)
            .await
            .unwrap();
            assert_eq!(
                read(f, &f.request()).await,
                Err(SessionFileDownloadError::PermissionDenied)
            );
            query(&format!(
                "UPDATE {}.{PUB_ROWS} SET account_id=$1,manifest=$2 WHERE upload_id=$3",
                f.sharing.base.publication_schema
            ))
            .bind(actor.account_id().as_uuid())
            .bind(serde_json::to_vec(&f.sharing.base.manifest).unwrap())
            .bind(publication_fixture::upload(1).as_uuid())
            .execute(&auth(f).base.admin)
            .await
            .unwrap();
            healthy(f).await;
            drain(f).await;
        })
    })
    .await;
}
