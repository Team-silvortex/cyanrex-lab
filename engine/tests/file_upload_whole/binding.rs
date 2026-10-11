use super::{bytes::*, fixture::*, support::*};
use cyanrex_engine::services::{
    file_transfer::staging::ChunkStaging,
    file_upload_staging::{FileUploadStaging, UploadFileVerificationError},
    file_upload_store::{FileUploadStore, FileUploadStoreError},
};
use sqlx_core::query::query;
use std::{fs, os::unix::fs::DirBuilderExt};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_visibility_scope_source_full_manifest_and_legacy_namespace_are_separate() {
    let fixture = Fixture::ready(17).await;
    fixture
        .staging
        .stage_chunk(&fixture.manifest, 0, Pattern::new(0, 17))
        .await
        .unwrap();
    let legacy = tree(&fixture.root);
    assert_eq!(legacy.len(), 1);
    assert!(legacy[0].name.to_string_lossy().starts_with("v1-"));
    bad(&fixture, UploadFileVerificationError::Unavailable).await;
    stage_all(&fixture).await;
    good(&fixture, &fixture.adapter).await;
    let rows = fixture.rows().await;
    let oids = fixture.oids().await;
    let files = tree(&fixture.root);
    for (id, actor, generation) in [
        (upload(99), owner(1), account(1)),
        (upload(1), owner(2), account(1)),
        (upload(1), owner(1), account(2)),
    ] {
        assert_eq!(
            fixture
                .adapter
                .verify_file(id, actor, generation)
                .await
                .unwrap_err(),
            UploadFileVerificationError::NotVisible
        );
        assert_eq!(fixture.rows().await, rows);
        assert_eq!(fixture.oids().await, oids);
        assert_eq!(tree(&fixture.root), files);
    }
    let mut foreign = owner(1);
    foreign.authority_id = uuid::Uuid::from_u128(900).try_into().unwrap();
    assert_eq!(
        fixture
            .adapter
            .verify_file(upload(1), foreign, account(1))
            .await
            .unwrap_err(),
        UploadFileVerificationError::Store(FileUploadStoreError::ScopeMismatch)
    );
    let wrong = FileUploadStore::new(
        fixture.pool.clone(),
        scope(),
        uuid::Uuid::new_v4(),
        &fixture.schema,
        limits(),
    )
    .unwrap();
    let wrong = FileUploadStaging::new(wrong, fixture.staging.clone(), 4).unwrap();
    assert_eq!(
        wrong
            .verify_file(upload(1), owner(1), account(1))
            .await
            .unwrap_err(),
        UploadFileVerificationError::Store(FileUploadStoreError::IncarnationMismatch)
    );
    assert_eq!(fixture.rows().await, rows);
    assert_eq!(fixture.oids().await, oids);
    assert_eq!(tree(&fixture.root), files);
    let other_root = fixture.directory.join("other-root");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&other_root)
        .unwrap();
    for entry in &files {
        fs::copy(fixture.root.join(&entry.name), other_root.join(&entry.name)).unwrap();
    }
    let other_files = tree(&other_root);
    let other = FileUploadStaging::new(
        fixture.source.clone(),
        ChunkStaging::open(scope(), &other_root, 2).unwrap(),
        4,
    )
    .unwrap();
    assert_eq!(
        other
            .verify_file(upload(1), owner(1), account(1))
            .await
            .unwrap_err(),
        UploadFileVerificationError::Unavailable
    );
    assert_eq!(tree(&other_root), other_files);
    assert_eq!(fixture.rows().await, rows);
    assert_eq!(tree(&fixture.root), files);

    for drift in ["owner", "account", "label"] {
        let mut actor = owner(1);
        let mut generation = account(1);
        match drift {
            "owner" => {
                actor = owner(2);
                query(&format!("UPDATE {LEDGER} SET owner_id=$1"))
                    .bind(actor.principal_id.as_uuid())
                    .execute(&fixture.pool)
                    .await
                    .unwrap();
            }
            "account" => {
                generation = account(2);
                query(&format!("UPDATE {LEDGER} SET account_id=$1"))
                    .bind(generation.as_uuid())
                    .execute(&fixture.pool)
                    .await
                    .unwrap();
            }
            "label" => {
                let mut wire = serde_json::to_value(&fixture.manifest).unwrap();
                wire["display_name"] = serde_json::json!("different inert label.bin");
                let changed =
                    cyanrex_engine::models::collaboration::FileTransferManifest::parse_json(
                        &serde_json::to_vec(&wire).unwrap(),
                    )
                    .unwrap();
                query(&format!("UPDATE {LEDGER} SET manifest=$1"))
                    .bind(serde_json::to_vec(&changed).unwrap())
                    .execute(&fixture.pool)
                    .await
                    .unwrap();
            }
            _ => unreachable!(),
        }
        let injected = fixture.rows().await;
        assert_eq!(
            fixture
                .adapter
                .verify_file(upload(1), actor, generation)
                .await
                .unwrap_err(),
            UploadFileVerificationError::Unavailable
        );
        assert_eq!(fixture.rows().await, injected);
        assert_eq!(fixture.oids().await, oids);
        assert_eq!(tree(&fixture.root), files);
        query(&format!(
            "UPDATE {LEDGER} SET owner_id=$1,account_id=$2,manifest=$3"
        ))
        .bind(owner(1).principal_id.as_uuid())
        .bind(account(1).as_uuid())
        .bind(serde_json::to_vec(&fixture.manifest).unwrap())
        .execute(&fixture.pool)
        .await
        .unwrap();
        assert_eq!(fixture.rows().await, rows);
        good(&fixture, &fixture.adapter).await;
    }
    fixture.pool_restored().await;
    fixture.cleanup().await;
}
