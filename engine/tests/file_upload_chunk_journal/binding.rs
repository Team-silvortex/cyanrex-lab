use super::{bytes::*, fixture::*, support::*};
use cyanrex_engine::{
    models::collaboration::FILE_TRANSFER_CHUNK_BYTES,
    services::{
        file_transfer::staging::ChunkStaging,
        file_upload_chunk_journal::{FileUploadChunkJournalError, PersistentChunkState},
        file_upload_staging::{
            FileUploadStaging, JournaledFileUploadStaging, JournaledUploadChunkError,
            UploadChunkError, UploadChunkState,
        },
        file_upload_store::FileUploadStore,
    },
};
use sqlx_core::query::query;
use std::{fs, os::unix::fs::DirBuilderExt, sync::atomic::Ordering};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_existing_unknown_coordinate_conflicts_across_source_and_root_drift() {
    for drift in ["manifest", "owner", "account", "installation", "root"] {
        let fixture = Fixture::ready(FILE_TRANSFER_CHUNK_BYTES + 17).await;
        let source = fixture.rows(false).await;
        let journal = fixture.rows(true).await;
        let oids = fixture.oids(true).await;
        for (id, actor, generation, index) in [
            (upload(999), owner(1), account(1), 0),
            (upload(1), owner(2), account(1), 0),
            (upload(1), owner(1), account(2), 0),
        ] {
            assert!(matches!(
                fixture
                    .journaled
                    .prepare(id, actor, generation, index)
                    .await,
                Err(JournaledUploadChunkError::Local(
                    UploadChunkError::NotVisible
                ))
            ));
            assert_eq!(fixture.rows(false).await, source);
            assert_eq!(fixture.rows(true).await, journal);
            assert!(tree(&fixture.root).is_empty());
        }
        assert!(matches!(
            fixture
                .journaled
                .prepare(upload(1), owner(1), account(1), 2)
                .await,
            Err(JournaledUploadChunkError::Local(
                UploadChunkError::InvalidInput
            ))
        ));
        let mut attempt = fixture
            .journaled
            .prepare(upload(1), owner(1), account(1), 0)
            .await
            .unwrap();
        assert!(matches!(
            fixture
                .journaled
                .stage_chunk(&mut attempt, FailedReader)
                .await,
            Err(JournaledUploadChunkError::Unconfirmed)
        ));
        assert_eq!(attempt.state(), UploadChunkState::Unknown);
        let retained = tree(&fixture.root);
        let persisted = fixture.rows(true).await;
        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].length, 0);
        let mut current_owner = owner(1);
        let mut current_account = account(1);
        let mut installation = fixture.source_installation_id;
        let mut changed_manifest = fixture.manifest.clone();
        match drift {
            "manifest" => {
                let mut wire = serde_json::to_value(&changed_manifest).unwrap();
                wire["display_name"] = serde_json::json!("changed-label.bin");
                changed_manifest =
                    cyanrex_engine::models::collaboration::FileTransferManifest::parse_json(
                        &serde_json::to_vec(&wire).unwrap(),
                    )
                    .unwrap();
                query(&format!("UPDATE {SOURCE_ROWS} SET manifest=$1"))
                    .bind(serde_json::to_vec(&changed_manifest).unwrap())
                    .execute(&fixture.source_pool)
                    .await
                    .unwrap();
            }
            "owner" => {
                current_owner = owner(2);
                query(&format!("UPDATE {SOURCE_ROWS} SET owner_id=$1"))
                    .bind(current_owner.principal_id.as_uuid())
                    .execute(&fixture.source_pool)
                    .await
                    .unwrap();
            }
            "account" => {
                current_account = account(2);
                query(&format!("UPDATE {SOURCE_ROWS} SET account_id=$1"))
                    .bind(current_account.as_uuid())
                    .execute(&fixture.source_pool)
                    .await
                    .unwrap();
            }
            "installation" => {
                installation = Uuid::new_v4();
                query(&format!("UPDATE {SOURCE_MARKER} SET installation_id=$1"))
                    .bind(installation)
                    .execute(&fixture.source_pool)
                    .await
                    .unwrap();
            }
            "root" => {}
            _ => unreachable!(),
        }
        let other_root = fixture.directory.join("other-root");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&other_root)
            .unwrap();
        let root = if drift == "root" {
            &other_root
        } else {
            &fixture.root
        };
        let current = FileUploadStore::new(
            fixture.source_pool.clone(),
            scope(),
            installation,
            &fixture.source_schema,
            limits(),
        )
        .unwrap();
        let fresh = JournaledFileUploadStaging::new(
            FileUploadStaging::new(current, ChunkStaging::open(scope(), root, 2).unwrap(), 4)
                .unwrap(),
            fixture.journal.clone(),
        )
        .unwrap();
        let after_source = fixture.rows(false).await;
        for index in [0, 1] {
            let mut fresh_attempt = fresh
                .prepare(upload(1), current_owner, current_account, index)
                .await
                .unwrap();
            assert_eq!(fresh_attempt.manifest(), &changed_manifest);
            let reader = Pattern::new(
                u64::from(index) * FILE_TRANSFER_CHUNK_BYTES,
                changed_manifest.chunk(index).unwrap().byte_length,
            );
            let calls = reader.calls.clone();
            assert!(matches!(
                fresh.stage_chunk(&mut fresh_attempt, reader).await,
                Err(JournaledUploadChunkError::Journal(
                    FileUploadChunkJournalError::Conflict
                ))
            ));
            assert_eq!(fresh_attempt.state(), UploadChunkState::Unknown);
            assert_eq!(calls.load(Ordering::SeqCst), 0);
            assert_eq!(fixture.rows(false).await, after_source);
            assert_eq!(fixture.rows(true).await, persisted);
            assert_eq!(tree(&fixture.root), retained);
            assert!(tree(&other_root).is_empty());
            assert_eq!(fixture.oids(true).await, oids);
        }
        let before = fixture.rows(true).await;
        metadata(
            &fixture
                .journal
                .get(upload(1), owner(1), account(1), 0)
                .await
                .unwrap()
                .unwrap(),
            0,
            &fixture.manifest,
            PersistentChunkState::Unknown,
        );
        assert_eq!(fixture.rows(true).await, before);
        assert_eq!(fixture.rows(false).await, after_source);
        assert_eq!(tree(&fixture.root), retained);
        // Restore only deliberate fixture mutations, then prove an unrelated healthy intent still works.
        query(&format!(
            "UPDATE {SOURCE_ROWS} SET owner_id=$1,account_id=$2,manifest=$3"
        ))
        .bind(owner(1).principal_id.as_uuid())
        .bind(account(1).as_uuid())
        .bind(serde_json::to_vec(&fixture.manifest).unwrap())
        .execute(&fixture.source_pool)
        .await
        .unwrap();
        query(&format!("UPDATE {SOURCE_MARKER} SET installation_id=$1"))
            .bind(fixture.source_installation_id)
            .execute(&fixture.source_pool)
            .await
            .unwrap();
        assert_eq!(fixture.rows(false).await, source);
        healthy(&fixture, 3).await;
        fixture.pool_restored(false).await;
        fixture.pool_restored(true).await;
        fixture.cleanup().await;
    }
}
