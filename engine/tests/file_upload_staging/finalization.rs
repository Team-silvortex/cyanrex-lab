use super::{bytes::*, control::*, fixture::*};
use cyanrex_engine::services::file_upload_staging::{UploadChunkError, UploadChunkState};
use sqlx_core::{query::query, row::Row};
use std::{
    fs::{self, OpenOptions, Permissions},
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
};

async fn lock_source(
    fixture: &Fixture,
) -> (
    sqlx_core::transaction::Transaction<'_, sqlx_postgres::Postgres>,
    i32,
) {
    let mut holder = fixture.admin.begin().await.unwrap();
    query(&format!(
        "LOCK TABLE {}.{MARKER},{}.{LEDGER} IN ACCESS EXCLUSIVE MODE",
        fixture.schema, fixture.schema
    ))
    .execute(&mut *holder)
    .await
    .unwrap();
    let pid: i32 = query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut *holder)
        .await
        .unwrap()
        .get("pid");
    (holder, pid)
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_postwrite_real_sql_wait_then_source_drift_is_unconfirmed() {
    for drift in ["owner", "account", "manifest", "installation", "relation"] {
        let fixture = Fixture::ready(17).await;
        let mut attempt = fixture
            .adapter
            .prepare(upload(1), owner(1), account(1), 0)
            .await
            .unwrap();
        let (reader, release, started, finished) = held(0, 17, 0);
        let mut pending = Box::pin(fixture.adapter.stage_chunk(&mut attempt, reader));
        tokio::select! { result=&mut pending=>panic!("stage completed before real reader barrier: {result:?}"),_ =signal(started)=>{} }
        let (mut holder, pid) = lock_source(&fixture).await;
        drop(release);
        tokio::select! { result=&mut pending=>panic!("stage completed before intended final SQL wait: {result:?}"),_ =fixture.wait_blocked(pid)=>{} }
        signal(finished).await;
        let before_files = tree(&fixture.root);
        assert_eq!(before_files.len(), 1);
        assert_eq!(before_files[0].length, 17);
        match drift {
            "owner" => {
                query(&format!(
                    "UPDATE {}.{LEDGER} SET owner_id=$1",
                    fixture.schema
                ))
                .bind(owner(2).principal_id.as_uuid())
                .execute(&mut *holder)
                .await
                .unwrap();
            }
            "account" => {
                query(&format!(
                    "UPDATE {}.{LEDGER} SET account_id=$1",
                    fixture.schema
                ))
                .bind(account(2).as_uuid())
                .execute(&mut *holder)
                .await
                .unwrap();
            }
            "manifest" => {
                let mut wire = serde_json::to_value(&fixture.manifest).unwrap();
                wire["display_name"] = serde_json::json!("changed.bin");
                let changed =
                    cyanrex_engine::models::collaboration::FileTransferManifest::parse_json(
                        &serde_json::to_vec(&wire).unwrap(),
                    )
                    .unwrap();
                query(&format!(
                    "UPDATE {}.{LEDGER} SET manifest=$1",
                    fixture.schema
                ))
                .bind(serde_json::to_vec(&changed).unwrap())
                .execute(&mut *holder)
                .await
                .unwrap();
            }
            "installation" => {
                query(&format!(
                    "UPDATE {}.{MARKER} SET installation_id=$1",
                    fixture.schema
                ))
                .bind(uuid::Uuid::new_v4())
                .execute(&mut *holder)
                .await
                .unwrap();
            }
            "relation" => {
                for statement in [
                    format!(
                        "ALTER TABLE {}.{LEDGER} RENAME TO original_ledger",
                        fixture.schema
                    ),
                    format!(
                        "CREATE TABLE {}.{LEDGER}(LIKE {}.original_ledger INCLUDING ALL)",
                        fixture.schema, fixture.schema
                    ),
                    format!(
                        "INSERT INTO {}.{LEDGER} SELECT * FROM {}.original_ledger",
                        fixture.schema, fixture.schema
                    ),
                    format!(
                        "UPDATE {}.{MARKER} SET uploads_oid='{}.{LEDGER}'::regclass::oid::bigint",
                        fixture.schema, fixture.schema
                    ),
                ] {
                    query(&statement).execute(&mut *holder).await.unwrap();
                }
            }
            _ => unreachable!(),
        }
        holder.commit().await.unwrap();
        assert!(
            matches!(pending.await, Err(UploadChunkError::Unconfirmed)),
            "postwrite {drift} must observe committed change after lock wait"
        );
        assert_eq!(attempt.state(), UploadChunkState::Unknown);
        assert_eq!(tree(&fixture.root), before_files);
        let before = fixture.rows().await;
        let reader = Pattern::new(0, 17);
        let calls = reader.calls.clone();
        assert!(matches!(
            fixture.adapter.stage_chunk(&mut attempt, reader).await,
            Err(UploadChunkError::InvalidInput)
        ));
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(fixture.rows().await, before);
        assert_eq!(tree(&fixture.root), before_files);
        fixture.pool_restored().await;
        fixture.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_same_bytes_new_inode_after_stage_receipt_never_confirms() {
    let fixture = Fixture::ready(17).await;
    let before_rows = fixture.rows().await;
    let mut attempt = fixture
        .adapter
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    let (reader, release, started, finished) = held(0, 17, 0);
    let mut pending = Box::pin(fixture.adapter.stage_chunk(&mut attempt, reader));
    tokio::select! {result=&mut pending=>panic!("stage completed before reader barrier: {result:?}"),_ =signal(started)=>{} }
    let (holder, pid) = lock_source(&fixture).await;
    drop(release);
    tokio::select! {result=&mut pending=>panic!("stage completed before final SQL wait: {result:?}"),_ =fixture.wait_blocked(pid)=>{} }
    signal(finished).await;
    let first = tree(&fixture.root);
    assert_eq!(first.len(), 1);
    let path = fixture.root.join(&first[0].name);
    let saved = fixture.directory.join("original-inode");
    fs::rename(&path, &saved).unwrap();
    let mut replacement = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    let mut bytes = Vec::new();
    Pattern::new(0, 17).read_to_end(&mut bytes).unwrap();
    replacement.write_all(&bytes).unwrap();
    replacement
        .set_permissions(Permissions::from_mode(0o400))
        .unwrap();
    drop(replacement);
    let replaced = snapshot(&path);
    assert_ne!(replaced.inode, first[0].inode);
    assert_eq!(replaced.digest, first[0].digest);
    let saved_before = snapshot(&saved);
    holder.rollback().await.unwrap();
    assert!(
        matches!(pending.await, Err(UploadChunkError::Unconfirmed)),
        "current identical bytes must not recapture the original receipt inode"
    );
    assert_eq!(attempt.state(), UploadChunkState::Unknown);
    assert_eq!(snapshot(&path), replaced);
    assert_eq!(snapshot(&saved), saved_before);
    assert_eq!(fixture.rows().await, before_rows);
    fixture.pool_restored().await;
    let mut new = fixture
        .adapter
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    assert!(matches!(
        fixture
            .adapter
            .stage_chunk(&mut new, Pattern::new(0, 17))
            .await,
        Err(UploadChunkError::Conflict)
    ));
    assert_eq!(snapshot(&path), replaced);
    assert_eq!(snapshot(&saved), saved_before);
    fixture.cleanup().await;
}
