use super::{bytes::tree, fixture::*, support::*};
use cyanrex_engine::services::{
    file_upload_chunk_journal::FileUploadChunkJournalError,
    file_upload_staging::{FileUploadPublicationError, FileUploadPublisher},
};
use sqlx_core::query::query;
use std::{fs, os::unix::fs::DirBuilderExt};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_confirmed_journal_requires_exact_original_source_root_and_chunks() {
    for drift in [
        "missing",
        "unknown",
        "owner",
        "account",
        "source",
        "root",
        "digest",
        "manifest",
        "other_root",
    ] {
        let f = Fixture::ready(17).await;
        if drift != "missing" {
            stage_all(&f).await;
        }
        match drift {
            "missing" | "other_root" => {}
            "unknown" => {
                f.sql(
                    Store::Journal,
                    &format!("UPDATE {JOURNAL_ROWS} SET confirmed=false"),
                )
                .await
            }
            "owner" => {
                query(&format!("UPDATE {JOURNAL_ROWS} SET owner_id=$1"))
                    .bind(owner(2).principal_id.as_uuid())
                    .execute(&f.journal_pool)
                    .await
                    .unwrap();
            }
            "account" => {
                query(&format!("UPDATE {JOURNAL_ROWS} SET account_id=$1"))
                    .bind(account(2).as_uuid())
                    .execute(&f.journal_pool)
                    .await
                    .unwrap();
            }
            "source" => {
                f.sql(
                    Store::Journal,
                    &format!("UPDATE {JOURNAL_ROWS} SET source_key=decode(repeat('ab',32),'hex')"),
                )
                .await
            }
            "root" => {
                f.sql(
                    Store::Journal,
                    &format!("UPDATE {JOURNAL_ROWS} SET root_key=decode(repeat('cd',32),'hex')"),
                )
                .await
            }
            "digest" => {
                f.sql(
                    Store::Journal,
                    &format!("UPDATE {JOURNAL_ROWS} SET sha256=repeat('a',64)"),
                )
                .await
            }
            "manifest" => {
                let mut wire = serde_json::to_value(&f.manifest).unwrap();
                wire["display_name"] = serde_json::json!("different.bin");
                let changed =
                    cyanrex_engine::models::collaboration::FileTransferManifest::parse_json(
                        &serde_json::to_vec(&wire).unwrap(),
                    )
                    .unwrap();
                query(&format!("UPDATE {SOURCE_ROWS} SET manifest=$1"))
                    .bind(serde_json::to_vec(&changed).unwrap())
                    .execute(&f.source_pool)
                    .await
                    .unwrap();
            }
            _ => unreachable!(),
        }
        let other = f.directory.join("other-private-root");
        let publisher = if drift == "other_root" {
            fs::DirBuilder::new().mode(0o700).create(&other).unwrap();
            let staging = cyanrex_engine::services::file_transfer::staging::ChunkStaging::open(
                scope(),
                &other,
                2,
            )
            .unwrap();
            let local = cyanrex_engine::services::file_upload_staging::FileUploadStaging::new(
                f.source.clone(),
                staging,
                4,
            )
            .unwrap();
            FileUploadPublisher::new(local, f.journal.clone(), f.publication.clone()).unwrap()
        } else {
            f.publisher.clone()
        };
        let before = snapshot_all(&f).await;
        let result = publisher.prepare(upload(1), owner(1), account(1)).await;
        let error = result
            .err()
            .expect("journal drift must reject before any file observation");
        let expected = if matches!(drift, "missing" | "unknown") {
            FileUploadPublicationError::NotReady
        } else {
            FileUploadPublicationError::Journal(FileUploadChunkJournalError::InvalidRecord)
        };
        assert_eq!(error, expected, "{drift}");
        assert_eq!(snapshot_all(&f).await, before);
        if other.exists() {
            assert!(tree(&other).is_empty());
        }
        assert!(matches!(
            f.publisher.prepare(upload(1), owner(2), account(1)).await,
            Err(FileUploadPublicationError::NotVisible)
        ));
        assert!(matches!(
            f.publisher.prepare(upload(1), owner(1), account(2)).await,
            Err(FileUploadPublicationError::NotVisible)
        ));
        assert_eq!(snapshot_all(&f).await, before);
        f.cleanup().await;
    }
    for drift in [
        "nonce_after_prepare",
        "unknown_after_prepare",
        "extra",
        "geometry",
    ] {
        let f = Fixture::ready(17).await;
        stage_all(&f).await;
        let mut prepared = f
            .publisher
            .prepare(upload(1), owner(1), account(1))
            .await
            .unwrap();
        match drift {
            "nonce_after_prepare" => {
                query(&format!("UPDATE {JOURNAL_ROWS} SET nonce=$1"))
                    .bind(uuid::Uuid::new_v4())
                    .execute(&f.journal_pool)
                    .await
                    .unwrap();
            }
            "unknown_after_prepare" => {
                f.sql(
                    Store::Journal,
                    &format!("UPDATE {JOURNAL_ROWS} SET confirmed=false"),
                )
                .await
            }
            "extra" => {
                query(&format!("INSERT INTO {JOURNAL_ROWS}(upload_id,chunk_index,owner_id,account_id,source_key,root_key,total_length,chunk_length,sha256,nonce,confirmed)
                    SELECT upload_id,1,owner_id,account_id,source_key,root_key,$1,17,sha256,$2,true FROM {JOURNAL_ROWS}"))
                    .bind((cyanrex_engine::models::collaboration::FILE_TRANSFER_CHUNK_BYTES+17) as i64).bind(uuid::Uuid::new_v4())
                    .execute(&f.journal_pool).await.unwrap();
            }
            "geometry" => {
                use sqlx_core::row::Row;
                let checks=query("SELECT conname FROM pg_constraint WHERE conrelid=to_regclass($1) AND contype='c'")
                    .bind(format!("{}.{JOURNAL_ROWS}",f.journal_schema)).fetch_all(&f.journal_pool).await.unwrap();
                for row in checks {
                    let name: String = row.get("conname");
                    assert!(name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_'));
                    f.sql(
                        Store::Journal,
                        &format!("ALTER TABLE {JOURNAL_ROWS} DROP CONSTRAINT {name}"),
                    )
                    .await;
                }
                f.sql(
                    Store::Journal,
                    &format!("UPDATE {JOURNAL_ROWS} SET chunk_length=16"),
                )
                .await;
            }
            _ => unreachable!(),
        }
        let before = snapshot_all(&f).await;
        let expected =
            FileUploadPublicationError::Journal(FileUploadChunkJournalError::InvalidRecord);
        assert_eq!(
            f.publisher.publish(&mut prepared).await.unwrap_err(),
            expected,
            "{drift}"
        );
        assert_eq!(
            prepared.state(),
            cyanrex_engine::services::file_upload_staging::FileUploadPublicationState::Unknown
        );
        if matches!(drift, "extra" | "geometry") {
            assert!(matches!(
                f.publisher.prepare(upload(1), owner(1), account(1)).await,
                Err(FileUploadPublicationError::Journal(
                    FileUploadChunkJournalError::InvalidRecord
                ))
            ));
        }
        assert_eq!(
            snapshot_all(&f).await,
            before,
            "original prepared fingerprint mismatch does not touch files"
        );
        f.cleanup().await;
    }
    let f = Fixture::ready(17).await;
    stage_all(&f).await;
    publish(&f, &f.publisher, 1, &f.manifest).await;
    f.cleanup().await;
}
