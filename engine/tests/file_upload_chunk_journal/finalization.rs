use super::{bytes::*, control::*, fixture::*, support::*};
use cyanrex_engine::services::{
    file_upload_chunk_journal::PersistentChunkState,
    file_upload_staging::{JournaledUploadChunkError, UploadChunkState},
};
use sqlx_core::query::query;
use std::{
    fs::{self, OpenOptions, Permissions},
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
};

fn substitute(fixture: &Fixture) -> (std::path::PathBuf, Entry, Entry) {
    let original = tree(&fixture.root);
    assert_eq!(original.len(), 1);
    assert_eq!(original[0].length, 17);
    let path = fixture.root.join(&original[0].name);
    let saved = fixture.directory.join("retained-original-inode");
    fs::rename(&path, &saved).unwrap();
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .unwrap();
    let mut bytes = Vec::new();
    Pattern::new(0, 17).read_to_end(&mut bytes).unwrap();
    output.write_all(&bytes).unwrap();
    output
        .set_permissions(Permissions::from_mode(0o400))
        .unwrap();
    drop(output);
    let current = snapshot(&path);
    assert_ne!(current.inode, original[0].inode);
    assert_eq!(current.digest, original[0].digest);
    assert_eq!(current.length, original[0].length);
    (saved.clone(), snapshot(&saved), current)
}

async fn mutate(fixture: &Fixture, holder: &mut Tx<'_>, drift: &str) {
    match drift {
        "owner"=> {query(&format!("UPDATE {}.{SOURCE_ROWS} SET owner_id=$1",fixture.source_schema)).bind(owner(2).principal_id.as_uuid()).execute(&mut **holder).await.unwrap();}
        "manifest"=> {
            let mut wire=serde_json::to_value(&fixture.manifest).unwrap();wire["display_name"]=serde_json::json!("changed.bin");
            let changed=cyanrex_engine::models::collaboration::FileTransferManifest::parse_json(&serde_json::to_vec(&wire).unwrap()).unwrap();
            query(&format!("UPDATE {}.{SOURCE_ROWS} SET manifest=$1",fixture.source_schema)).bind(serde_json::to_vec(&changed).unwrap()).execute(&mut **holder).await.unwrap();
        }
        "installation"=> {query(&format!("UPDATE {}.{SOURCE_MARKER} SET installation_id=$1",fixture.source_schema)).bind(uuid::Uuid::new_v4()).execute(&mut **holder).await.unwrap();}
        "relation"=> {
            for statement in [format!("ALTER TABLE {}.{SOURCE_ROWS} RENAME TO original_uploads",fixture.source_schema),
                format!("CREATE TABLE {}.{SOURCE_ROWS}(LIKE {}.original_uploads INCLUDING ALL)",fixture.source_schema,fixture.source_schema),
                format!("INSERT INTO {}.{SOURCE_ROWS} SELECT * FROM {}.original_uploads",fixture.source_schema,fixture.source_schema),
                format!("UPDATE {}.{SOURCE_MARKER} SET uploads_oid='{}.{SOURCE_ROWS}'::regclass::oid::bigint",fixture.source_schema,fixture.source_schema)] {
                query(&statement).execute(&mut **holder).await.unwrap();
            }
        }
        "inode"=> {},_=>unreachable!()
    }
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_postwrite_source_or_original_inode_drift_cannot_confirm() {
    for drift in ["owner", "manifest", "installation", "relation", "inode"] {
        let fixture = Fixture::ready(17).await;
        let source = fixture.rows(false).await;
        let journal_oids = fixture.oids(true).await;
        let mut attempt = fixture
            .journaled
            .prepare(upload(1), owner(1), account(1), 0)
            .await
            .unwrap();
        let (reader, release, started, finished) = held(0, 17, 0);
        let mut pending = Box::pin(fixture.journaled.stage_chunk(&mut attempt, reader));
        tokio::select! {result=&mut pending=>panic!("missing real worker barrier: {result:?}"),_=signal(started)=>{}}
        let unknown = fixture.rows(true).await;
        let (mut holder, pid) = lock(&fixture, false).await;
        drop(release);
        tokio::select! {result=&mut pending=>panic!("missing postwrite SQL wait: {result:?}"),_=fixture.wait_blocked(false,pid)=>{}}
        signal(finished).await;
        let original = tree(&fixture.root);
        assert_eq!(original.len(), 1);
        assert_eq!(original[0].length, 17);
        let replacement = if drift == "inode" {
            Some(substitute(&fixture))
        } else {
            None
        };
        mutate(&fixture, &mut holder, drift).await;
        holder.commit().await.unwrap();
        assert!(
            matches!(pending.await, Err(JournaledUploadChunkError::Unconfirmed)),
            "late {drift} must never manufacture confirmation"
        );
        assert_eq!(attempt.state(), UploadChunkState::Unknown);
        let after_source = fixture.rows(false).await;
        if drift == "inode" {
            assert_eq!(after_source, source);
        } else {
            assert_ne!(after_source, source);
        }
        assert_eq!(fixture.rows(true).await, unknown);
        assert_eq!(fixture.oids(true).await, journal_oids);
        let retained = tree(&fixture.root);
        if let Some((saved, saved_entry, current)) = replacement {
            assert_eq!(snapshot(&saved), saved_entry);
            assert_eq!(retained, vec![current]);
        } else {
            assert_eq!(retained, original);
        }
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
        assert_eq!(fixture.rows(true).await, unknown);
        assert_eq!(fixture.rows(false).await, after_source);
        assert_eq!(tree(&fixture.root), retained);
        fixture.pool_restored(false).await;
        fixture.pool_restored(true).await;
        fixture.cleanup().await;
    }
    let healthy_fixture = Fixture::ready(17).await;
    healthy(&healthy_fixture, 2).await;
    healthy_fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_postconfirmation_acknowledged_row_survives_late_caller_uncertainty() {
    for drift in ["owner", "inode"] {
        let fixture = Fixture::ready(17).await;
        let source = fixture.rows(false).await;
        let journal_oids = fixture.oids(true).await;
        let (journal_holder, journal_pid) = advisory(&fixture).await;
        fault(
            &fixture,
            "UPDATE",
            &format!(
                "PERFORM pg_advisory_xact_lock(hashtext('{}'),998);",
                fixture.journal_schema
            ),
            true,
        )
        .await;
        let mut attempt = fixture
            .journaled
            .prepare(upload(1), owner(1), account(1), 0)
            .await
            .unwrap();
        let mut pending = Box::pin(
            fixture
                .journaled
                .stage_chunk(&mut attempt, Pattern::new(0, 17)),
        );
        tokio::select! {result=&mut pending=>panic!("missing final confirmation SQL barrier: {result:?}"),_=fixture.wait_blocked(true,journal_pid)=>{}}
        let unknown = fixture.rows(true).await;
        assert!(hit(&fixture).await);
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
        assert_eq!(fixture.rows(true).await, unknown);
        let original = tree(&fixture.root);
        assert_eq!(original[0].length, 17);
        let (mut source_holder, source_pid) = lock(&fixture, false).await;
        journal_holder.rollback().await.unwrap();
        tokio::select! {result=&mut pending=>panic!("missing post-ACK source SQL barrier: {result:?}"),_=fixture.wait_blocked(false,source_pid)=>{}}
        // Independent reader observes the acknowledged final COMMIT while the caller still waits.
        let confirmed = fixture.rows(true).await;
        assert_ne!(confirmed, unknown);
        assert_eq!(confirmed[0], unknown[0]);
        metadata(
            &fixture
                .journal
                .get(upload(1), owner(1), account(1), 0)
                .await
                .unwrap()
                .unwrap(),
            0,
            &fixture.manifest,
            PersistentChunkState::Confirmed,
        );
        assert_eq!(fixture.rows(true).await, confirmed);
        let replacement = if drift == "inode" {
            Some(substitute(&fixture))
        } else {
            None
        };
        mutate(&fixture, &mut source_holder, drift).await;
        source_holder.commit().await.unwrap();
        assert!(matches!(
            pending.await,
            Err(JournaledUploadChunkError::Unconfirmed)
        ));
        assert_eq!(attempt.state(), UploadChunkState::Unknown);
        assert_eq!(fixture.rows(true).await, confirmed);
        assert_eq!(fixture.oids(true).await, journal_oids);
        let after_source = fixture.rows(false).await;
        let retained = tree(&fixture.root);
        if let Some((saved, saved_entry, current)) = replacement {
            assert_eq!(after_source, source);
            assert_eq!(snapshot(&saved), saved_entry);
            assert_eq!(retained, vec![current]);
        } else {
            assert_ne!(after_source, source);
            assert_eq!(retained, original);
        }
        metadata(
            &fixture
                .journal
                .get(upload(1), owner(1), account(1), 0)
                .await
                .unwrap()
                .unwrap(),
            0,
            &fixture.manifest,
            PersistentChunkState::Confirmed,
        );
        assert_eq!(fixture.rows(true).await, confirmed);
        assert_eq!(fixture.rows(false).await, after_source);
        assert_eq!(tree(&fixture.root), retained);
        remove_fault(&fixture).await;
        // The original in-memory attempt is terminal, even though history already says Confirmed.
        assert!(matches!(
            fixture
                .journaled
                .stage_chunk(&mut attempt, Pattern::new(0, 17))
                .await,
            Err(JournaledUploadChunkError::Local(
                cyanrex_engine::services::file_upload_staging::UploadChunkError::InvalidInput
            ))
        ));
        assert_eq!(fixture.rows(true).await, confirmed);
        fixture.pool_restored(false).await;
        fixture.pool_restored(true).await;
        fixture.cleanup().await;
    }
    let healthy_fixture = Fixture::ready(17).await;
    healthy(&healthy_fixture, 2).await;
    healthy_fixture.cleanup().await;
}
