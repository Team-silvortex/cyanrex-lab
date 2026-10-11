use super::{
    bytes::{tree, Pattern},
    fixture::*,
};
use cyanrex_engine::{
    models::collaboration::FileTransferManifest,
    services::{
        file_upload_staging::{FileUploadStaging, UploadChunkError, UploadChunkState},
        file_upload_store::FileUploadStoreError,
    },
};
use std::sync::atomic::Ordering;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_prepare_visibility_generation_scope_and_index_are_read_only() {
    let fixture = Fixture::ready(17).await;
    let before = fixture.rows().await;
    let oids = fixture.oids().await;
    for (id, actor, generation) in [
        (upload(999), owner(1), account(1)),
        (upload(1), owner(2), account(1)),
        (upload(1), owner(1), account(2)),
    ] {
        assert!(matches!(
            fixture.adapter.prepare(id, actor, generation, 0).await,
            Err(UploadChunkError::NotVisible)
        ));
        assert_eq!(fixture.rows().await, before);
        assert!(tree(&fixture.root).is_empty());
    }
    for index in [1, u32::MAX] {
        assert!(matches!(
            fixture
                .adapter
                .prepare(upload(1), owner(1), account(1), index)
                .await,
            Err(UploadChunkError::InvalidInput)
        ));
        assert_eq!(fixture.rows().await, before);
        assert!(tree(&fixture.root).is_empty());
    }
    let mut foreign = owner(1);
    foreign.authority_id = uuid::Uuid::from_u128(900).try_into().unwrap();
    assert!(matches!(
        fixture
            .adapter
            .prepare(upload(1), foreign, account(1), 0)
            .await,
        Err(UploadChunkError::Store(FileUploadStoreError::ScopeMismatch))
    ));
    let empty = manifest(0, 2);
    fixture
        .source
        .reserve(upload(2), owner(1), account(2), &empty)
        .await
        .unwrap();
    let before = fixture.rows().await;
    assert!(matches!(
        fixture
            .adapter
            .prepare(upload(2), owner(1), account(2), 0)
            .await,
        Err(UploadChunkError::InvalidInput)
    ));
    let ready = fixture
        .adapter
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    assert_eq!(ready.state(), UploadChunkState::Ready);
    assert_eq!(ready.manifest(), &fixture.manifest);
    assert_eq!(fixture.rows().await, before);
    assert_eq!(fixture.oids().await, oids);
    assert!(tree(&fixture.root).is_empty());
    std::fs::set_permissions(
        &fixture.root,
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();
    let changed_root = super::bytes::snapshot(&fixture.root);
    let mut prepared_without_fs = fixture
        .adapter
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    assert_eq!(prepared_without_fs.state(), UploadChunkState::Ready);
    assert_eq!(
        super::bytes::snapshot(&fixture.root),
        changed_root,
        "prepare must not repair or read current root/chunk state"
    );
    let reader = Pattern::new(0, 17);
    let calls = reader.calls.clone();
    assert!(matches!(
        fixture
            .adapter
            .stage_chunk(&mut prepared_without_fs, reader)
            .await,
        Err(UploadChunkError::Unavailable)
    ));
    assert_eq!(prepared_without_fs.state(), UploadChunkState::Unknown);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.rows().await, before);
    assert!(tree(&fixture.root).is_empty());
    std::fs::set_permissions(
        &fixture.root,
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .unwrap();
    let mut fresh = fixture
        .adapter
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    fixture
        .adapter
        .stage_chunk(&mut fresh, Pattern::new(0, 17))
        .await
        .unwrap();
    assert_eq!(fixture.rows().await, before);
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_issuer_clone_fixed_destination_and_full_manifest_binding_are_exact() {
    let fixture = Fixture::ready(17).await;
    let independent =
        FileUploadStaging::new(fixture.source.clone(), fixture.staging.clone(), 4).unwrap();
    let mut original = fixture
        .adapter
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    let reader = Pattern::new(0, 17);
    let calls = reader.calls.clone();
    assert!(matches!(
        independent.stage_chunk(&mut original, reader).await,
        Err(UploadChunkError::InvalidInput)
    ));
    assert_eq!(original.state(), UploadChunkState::Ready);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let before = fixture.rows().await;
    fixture
        .adapter
        .clone()
        .stage_chunk(&mut original, Pattern::new(0, 17))
        .await
        .unwrap();
    let saved = tree(&fixture.root);
    assert_eq!(saved.len(), 1);
    let mut fresh = independent
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    let reader = Pattern::new(0, 17);
    let calls = reader.calls.clone();
    assert!(matches!(
        independent.stage_chunk(&mut fresh, reader).await,
        Err(UploadChunkError::Conflict)
    ));
    assert_eq!(fresh.state(), UploadChunkState::Unknown);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(tree(&fixture.root), saved);
    assert_eq!(fixture.rows().await, before);
    // A newly prepared trusted call with different canonical metadata gets another fixed key.
    let mut wire = serde_json::to_value(&fixture.manifest).unwrap();
    wire["display_name"] = serde_json::json!("changed.bin");
    let changed = FileTransferManifest::parse_json(&serde_json::to_vec(&wire).unwrap()).unwrap();
    sqlx_core::query::query(&format!(
        "UPDATE {LEDGER} SET manifest=$1 WHERE upload_id=$2"
    ))
    .bind(serde_json::to_vec(&changed).unwrap())
    .bind(upload(1).as_uuid())
    .execute(&fixture.pool)
    .await
    .unwrap();
    let before = fixture.rows().await;
    let mut distinct = fixture
        .adapter
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    fixture
        .adapter
        .stage_chunk(&mut distinct, Pattern::new(0, 17))
        .await
        .unwrap();
    let now = tree(&fixture.root);
    assert_eq!(now.len(), 2);
    assert!(now.iter().any(|entry| entry == &saved[0]));
    assert_eq!(fixture.rows().await, before);
    for (column, value, id, actor, generation, count) in [
        (
            "owner_id",
            owner(2).principal_id.as_uuid(),
            upload(1),
            owner(2),
            account(1),
            3,
        ),
        (
            "account_id",
            account(2).as_uuid(),
            upload(1),
            owner(2),
            account(2),
            4,
        ),
        (
            "upload_id",
            upload(2).as_uuid(),
            upload(2),
            owner(2),
            account(2),
            5,
        ),
    ] {
        sqlx_core::query::query(&format!("UPDATE {LEDGER} SET {column}=$1"))
            .bind(value)
            .execute(&fixture.pool)
            .await
            .unwrap();
        let before = fixture.rows().await;
        let mut attributed = fixture
            .adapter
            .prepare(id, actor, generation, 0)
            .await
            .unwrap();
        fixture
            .adapter
            .stage_chunk(&mut attributed, Pattern::new(0, 17))
            .await
            .unwrap();
        assert_eq!(tree(&fixture.root).len(), count);
        assert_eq!(fixture.rows().await, before);
    }
    let installation = uuid::Uuid::new_v4();
    fixture
        .sql(&format!(
            "UPDATE {MARKER} SET installation_id='{installation}'"
        ))
        .await;
    let source = cyanrex_engine::services::file_upload_store::FileUploadStore::new(
        fixture.pool.clone(),
        scope(),
        installation,
        &fixture.schema,
        limits(),
    )
    .unwrap();
    let changed_installation =
        FileUploadStaging::new(source.clone(), fixture.staging.clone(), 4).unwrap();
    let before = fixture.rows().await;
    let mut prepared = changed_installation
        .prepare(upload(2), owner(2), account(2), 0)
        .await
        .unwrap();
    changed_installation
        .stage_chunk(&mut prepared, Pattern::new(0, 17))
        .await
        .unwrap();
    assert_eq!(tree(&fixture.root).len(), 6);
    assert_eq!(fixture.rows().await, before);
    fixture
        .sql(&format!("ALTER TABLE {MARKER} RENAME TO old_upload_marker"))
        .await;
    fixture
        .sql(&format!(
            "CREATE TABLE {MARKER}(LIKE old_upload_marker INCLUDING ALL)"
        ))
        .await;
    fixture
        .sql(&format!(
            "INSERT INTO {MARKER} SELECT * FROM old_upload_marker"
        ))
        .await;
    fixture
        .sql(&format!(
            "UPDATE {MARKER} SET schema_oid='{MARKER}'::regclass::oid::bigint"
        ))
        .await;
    let before = fixture.rows().await;
    let mut prepared = changed_installation
        .prepare(upload(2), owner(2), account(2), 0)
        .await
        .unwrap();
    changed_installation
        .stage_chunk(&mut prepared, Pattern::new(0, 17))
        .await
        .unwrap();
    let old_files = tree(&fixture.root);
    assert_eq!(old_files.len(), 7);
    assert_eq!(fixture.rows().await, before);
    let moved = fixture.directory.join("old-root");
    std::fs::rename(&fixture.root, &moved).unwrap();
    std::fs::create_dir(&fixture.root).unwrap();
    std::fs::set_permissions(
        &fixture.root,
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .unwrap();
    let new_staging = cyanrex_engine::services::file_transfer::staging::ChunkStaging::open(
        scope(),
        &fixture.root,
        1,
    )
    .unwrap();
    let new_root = FileUploadStaging::new(source, new_staging, 4).unwrap();
    let mut fresh = new_root
        .prepare(upload(2), owner(2), account(2), 0)
        .await
        .unwrap();
    new_root
        .stage_chunk(&mut fresh, Pattern::new(0, 17))
        .await
        .unwrap();
    let new_files = tree(&fixture.root);
    assert_eq!(new_files.len(), 1);
    assert!(
        !old_files
            .iter()
            .any(|entry| entry.name == new_files[0].name),
        "fixed key must include original root device/inode"
    );
    assert_eq!(tree(&moved), old_files);
    assert_eq!(fixture.rows().await, before);
    fixture.cleanup().await;
}
