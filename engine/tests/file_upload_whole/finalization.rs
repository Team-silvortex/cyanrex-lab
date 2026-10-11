use super::{bytes::*, control::signal, fixture::*, pool_gate::captured_pool, support::*};
use cyanrex_engine::services::file_upload_staging::UploadFileVerificationError;
use sqlx_core::query::query;
use std::{
    fs::{self, OpenOptions, Permissions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, PermissionsExt},
};

async fn mutate_source(fixture: &Fixture, holder: &mut Tx<'_>, drift: &str) {
    match drift {
        "owner" => {
            query(&format!(
                "UPDATE {}.{LEDGER} SET owner_id=$1",
                fixture.schema
            ))
            .bind(owner(2).principal_id.as_uuid())
            .execute(&mut **holder)
            .await
            .unwrap();
        }
        "account" => {
            query(&format!(
                "UPDATE {}.{LEDGER} SET account_id=$1",
                fixture.schema
            ))
            .bind(account(2).as_uuid())
            .execute(&mut **holder)
            .await
            .unwrap();
        }
        "manifest" => {
            let mut wire = serde_json::to_value(&fixture.manifest).unwrap();
            wire["display_name"] = serde_json::json!("changed.bin");
            let changed = cyanrex_engine::models::collaboration::FileTransferManifest::parse_json(
                &serde_json::to_vec(&wire).unwrap(),
            )
            .unwrap();
            query(&format!(
                "UPDATE {}.{LEDGER} SET manifest=$1",
                fixture.schema
            ))
            .bind(serde_json::to_vec(&changed).unwrap())
            .execute(&mut **holder)
            .await
            .unwrap();
        }
        "installation" => {
            query(&format!(
                "UPDATE {}.{MARKER} SET installation_id=$1",
                fixture.schema
            ))
            .bind(uuid::Uuid::new_v4())
            .execute(&mut **holder)
            .await
            .unwrap();
        }
        "relation" => {
            for statement in [
                format!(
                    "ALTER TABLE {}.{LEDGER} RENAME TO original_uploads",
                    fixture.schema
                ),
                format!(
                    "CREATE TABLE {}.{LEDGER}(LIKE {}.original_uploads INCLUDING ALL)",
                    fixture.schema, fixture.schema
                ),
                format!(
                    "INSERT INTO {}.{LEDGER} SELECT * FROM {}.original_uploads",
                    fixture.schema, fixture.schema
                ),
                format!(
                    "UPDATE {}.{MARKER} SET uploads_oid='{}.{LEDGER}'::regclass::oid::bigint",
                    fixture.schema, fixture.schema
                ),
            ] {
                query(&statement).execute(&mut **holder).await.unwrap();
            }
        }
        _ => unreachable!(),
    }
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_postio_source_wait_drift_cannot_return_an_observation() {
    for drift in ["owner", "account", "manifest", "installation", "relation"] {
        let fixture = Fixture::ready(17).await;
        stage_all(&fixture).await;
        let original = fixture.rows().await;
        let original_oids = fixture.oids().await;
        let files = tree(&fixture.root);
        let (adapter, pool, release, started) = captured_pool(&fixture, 4).await;
        let mut pending = Box::pin(adapter.verify_file(upload(1), owner(1), account(1)));
        tokio::select! {result=&mut pending=>panic!("initial source COMMIT gate was not reached: {result:?}"),_=signal(started)=>{}}
        // The initial read transaction is really over: this exclusive lock can now be acquired.
        let (mut holder, pid) = lock(&fixture).await;
        drop(release);
        tokio::select! {result=&mut pending=>panic!("fresh post-IO SQL check did not wait: {result:?}"),_=fixture.wait_blocked(pid)=>{}}
        assert_eq!(tree(&fixture.root), files);
        mutate_source(&fixture, &mut holder, drift).await;
        let (injected, injected_oids) = state_in_tx(&fixture, &mut holder).await;
        holder.commit().await.unwrap();
        assert_eq!(
            pending.await.unwrap_err(),
            UploadFileVerificationError::Unconfirmed,
            "post-IO {drift} cannot produce an observation"
        );
        assert_ne!(injected, original);
        assert_eq!(tree(&fixture.root), files);
        assert_eq!(fixture.rows().await, injected);
        assert_eq!(fixture.oids().await, injected_oids);
        if drift == "relation" {
            fixture
                .sql(&format!(
                    "ALTER TABLE {LEDGER} RENAME TO replacement_uploads"
                ))
                .await;
            fixture
                .sql(&format!("ALTER TABLE original_uploads RENAME TO {LEDGER}"))
                .await;
            fixture
                .sql(&format!(
                    "UPDATE {MARKER} SET uploads_oid='{LEDGER}'::regclass::oid::bigint"
                ))
                .await;
        }
        query(&format!(
            "UPDATE {LEDGER} SET owner_id=$1,account_id=$2,manifest=$3"
        ))
        .bind(owner(1).principal_id.as_uuid())
        .bind(account(1).as_uuid())
        .bind(serde_json::to_vec(&fixture.manifest).unwrap())
        .execute(&fixture.pool)
        .await
        .unwrap();
        query(&format!("UPDATE {MARKER} SET installation_id=$1"))
            .bind(fixture.installation_id)
            .execute(&fixture.pool)
            .await
            .unwrap();
        assert_eq!(fixture.rows().await, original);
        assert_eq!(fixture.oids().await, original_oids);
        good(&fixture, &adapter).await;
        pool.close().await;
        fixture.pool_restored().await;
        fixture.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_postio_original_files_or_root_replacement_are_unconfirmed() {
    for drift in ["inode", "bytes", "mode", "hardlink", "root"] {
        let fixture = Fixture::ready(17).await;
        stage_all(&fixture).await;
        let rows = fixture.rows().await;
        let oids = fixture.oids().await;
        let original = tree(&fixture.root);
        let path = fixture.root.join(&original[0].name);
        let saved = fixture.directory.join("saved-original");
        let alias = fixture.directory.join("extra-link");
        let (adapter, pool, release, started) = captured_pool(&fixture, 4).await;
        let mut pending = Box::pin(adapter.verify_file(upload(1), owner(1), account(1)));
        tokio::select! {result=&mut pending=>panic!("initial source COMMIT gate was not reached: {result:?}"),_=signal(started)=>{}}
        let (holder, pid) = lock(&fixture).await;
        drop(release);
        tokio::select! {result=&mut pending=>panic!("fresh post-IO SQL check did not wait: {result:?}"),_=fixture.wait_blocked(pid)=>{}}
        match drift {
            "inode" => {
                fs::rename(&path, &saved).unwrap();
                let mut bytes = Vec::new();
                Pattern::new(0, 17).read_to_end(&mut bytes).unwrap();
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .unwrap()
                    .write_all(&bytes)
                    .unwrap();
                fs::set_permissions(&path, Permissions::from_mode(0o400)).unwrap();
                assert_ne!(snapshot(&path).inode, original[0].inode);
                assert_eq!(snapshot(&path).digest, original[0].digest);
            }
            "bytes" => {
                fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
                let mut bytes = Vec::new();
                Pattern::new(0, 17).read_to_end(&mut bytes).unwrap();
                bytes[0] ^= 1;
                OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(&path)
                    .unwrap()
                    .write_all(&bytes)
                    .unwrap();
                fs::set_permissions(&path, Permissions::from_mode(0o400)).unwrap();
            }
            "mode" => fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap(),
            "hardlink" => fs::hard_link(&path, &alias).unwrap(),
            "root" => {
                fs::rename(&fixture.root, &saved).unwrap();
                fs::DirBuilder::new()
                    .mode(0o700)
                    .create(&fixture.root)
                    .unwrap();
                fs::copy(saved.join(&original[0].name), &path).unwrap();
                assert_ne!(snapshot(&fixture.root).inode, snapshot(&saved).inode);
            }
            _ => unreachable!(),
        }
        let injected = tree(&fixture.root);
        let saved_before = if saved.exists() {
            Some(snapshot(&saved))
        } else {
            None
        };
        let saved_files = if saved.is_dir() {
            Some(tree(&saved))
        } else {
            None
        };
        let alias_before = if alias.exists() {
            Some(snapshot(&alias))
        } else {
            None
        };
        holder.rollback().await.unwrap();
        assert_eq!(
            pending.await.unwrap_err(),
            UploadFileVerificationError::Unconfirmed
        );
        assert_eq!(fixture.rows().await, rows);
        assert_eq!(fixture.oids().await, oids);
        assert_eq!(tree(&fixture.root), injected);
        if let Some(saved_before) = saved_before {
            assert_eq!(snapshot(&saved), saved_before);
        }
        if let Some(saved_files) = saved_files {
            assert_eq!(tree(&saved), saved_files);
        }
        if let Some(alias_before) = alias_before {
            assert_eq!(snapshot(&alias), alias_before);
        }
        match drift {
            "inode" => {
                fs::remove_file(&path).unwrap();
                fs::rename(&saved, &path).unwrap();
            }
            "bytes" => {
                fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
                let mut output = OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(&path)
                    .unwrap();
                std::io::copy(&mut Pattern::new(0, 17), &mut output).unwrap();
                drop(output);
                fs::set_permissions(&path, Permissions::from_mode(0o400)).unwrap();
            }
            "mode" => fs::set_permissions(&path, Permissions::from_mode(0o400)).unwrap(),
            "hardlink" => fs::remove_file(&alias).unwrap(),
            "root" => {
                fs::rename(
                    &fixture.root,
                    fixture.directory.join("rejected-replacement-root"),
                )
                .unwrap();
                fs::rename(&saved, &fixture.root).unwrap();
            }
            _ => unreachable!(),
        }
        good(&fixture, &adapter).await;
        pool.close().await;
        fixture.pool_restored().await;
        fixture.cleanup().await;
    }
}
