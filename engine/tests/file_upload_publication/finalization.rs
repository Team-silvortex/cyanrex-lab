use super::{bytes::*, control::signal, fixture::*, pool_gate, support::*};
use cyanrex_engine::services::file_upload_staging::{
    FileUploadPublicationError, FileUploadPublicationState,
};
use sqlx_core::query::query;
use std::{
    fs::{self, OpenOptions, Permissions},
    io::Write,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
};

async fn mutate(f: &Fixture, store: Store, tx: &mut Tx<'_>, drift: &str) {
    let schema = f.schema(store);
    let [marker, rows] = Fixture::tables(store);
    match drift {
        "owner" => {
            query(&format!("UPDATE {schema}.{rows} SET owner_id=$1"))
                .bind(owner(2).principal_id.as_uuid())
                .execute(&mut **tx)
                .await
                .unwrap();
        }
        "manifest" => {
            let mut wire = serde_json::to_value(&f.manifest).unwrap();
            wire["display_name"] = serde_json::json!("after-ack.bin");
            let changed = cyanrex_engine::models::collaboration::FileTransferManifest::parse_json(
                &serde_json::to_vec(&wire).unwrap(),
            )
            .unwrap();
            query(&format!("UPDATE {schema}.{rows} SET manifest=$1"))
                .bind(serde_json::to_vec(&changed).unwrap())
                .execute(&mut **tx)
                .await
                .unwrap();
        }
        "nonce" => {
            query(&format!("UPDATE {schema}.{rows} SET nonce=$1"))
                .bind(uuid::Uuid::new_v4())
                .execute(&mut **tx)
                .await
                .unwrap();
        }
        "unknown" => {
            query(&format!("UPDATE {schema}.{rows} SET confirmed=false"))
                .execute(&mut **tx)
                .await
                .unwrap();
        }
        "installation" => {
            query(&format!("UPDATE {schema}.{marker} SET installation_id=$1"))
                .bind(uuid::Uuid::new_v4())
                .execute(&mut **tx)
                .await
                .unwrap();
        }
        "relation" => {
            let oid_column = if matches!(store, Store::Source) {
                "uploads_oid"
            } else {
                "chunks_oid"
            };
            for statement in [format!("ALTER TABLE {schema}.{rows} RENAME TO retained_original"),
                format!("CREATE TABLE {schema}.{rows}(LIKE {schema}.retained_original INCLUDING ALL)"),
                format!("INSERT INTO {schema}.{rows} SELECT * FROM {schema}.retained_original"),
                format!("UPDATE {schema}.{marker} SET {oid_column}='{schema}.{rows}'::regclass::oid::bigint")] {
                query(&statement).execute(&mut **tx).await.unwrap();
            }
        }
        _ => unreachable!(),
    }
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_independently_visible_ack_then_source_or_journal_drift_is_unconfirmed() {
    for (store, drift) in [
        (Store::Source, "owner"),
        (Store::Source, "manifest"),
        (Store::Source, "installation"),
        (Store::Source, "relation"),
        (Store::Journal, "nonce"),
        (Store::Journal, "unknown"),
        (Store::Journal, "relation"),
    ] {
        let f = Fixture::ready(17).await;
        stage_all(&f).await;
        let original = snapshot_all(&f).await;
        let (publisher, pool, release, started) = pool_gate::publisher(&f, store).await;
        let mut attempt = publisher
            .prepare(upload(1), owner(1), account(1))
            .await
            .unwrap();
        release.arm(&pool).await;
        let mut pending = Box::pin(publisher.publish(&mut attempt));
        tokio::select! {result=&mut pending=>panic!("missing original pre-INSERT COMMIT return barrier: {result:?}"),_=signal(started)=>{}}
        // Source/journal original short transaction has really committed and released its locks.
        let (mut holder, pid) = lock(&f, store).await;
        drop(release);
        tokio::select! {result=&mut pending=>panic!("missing true post-publication ACK SQL wait: {result:?}"),_=f.wait_blocked(store,pid)=>{}}
        // Independent pool sees Published BEFORE any fault mutation. A deferred trigger wait alone
        // is deliberately not used as a substitute for this committed acknowledgement evidence.
        let acknowledged = f.rows(Store::Publication).await;
        assert_eq!(acknowledged[0], original.rows[2][0]);
        assert_eq!(acknowledged[1].len(), 1);
        record_metadata(
            &f.publication
                .get(upload(1), owner(1), account(1))
                .await
                .unwrap()
                .unwrap(),
            1,
            &f.manifest,
        );
        assert_eq!(tree(&f.root), original.files);
        mutate(&f, store, &mut holder, drift).await;
        let (injected, injected_oids) = state_in_tx(&f, store, &mut holder).await;
        holder.commit().await.unwrap();
        assert_eq!(
            pending.await.unwrap_err(),
            FileUploadPublicationError::Unconfirmed,
            "{store:?} {drift}"
        );
        assert_eq!(attempt.state(), FileUploadPublicationState::Unknown);
        assert_eq!(f.rows(store).await, injected);
        assert_eq!(f.oids(store).await, injected_oids);
        let other = if matches!(store, Store::Source) {
            Store::Journal
        } else {
            Store::Source
        };
        let other_index = if matches!(other, Store::Source) { 0 } else { 1 };
        assert_eq!(f.rows(other).await, original.rows[other_index]);
        assert_eq!(f.oids(other).await, original.oids[other_index]);
        assert_eq!(f.rows(Store::Publication).await, acknowledged);
        assert_eq!(f.oids(Store::Publication).await, original.oids[2]);
        assert_eq!(tree(&f.root), original.files);
        assert_eq!(snapshot(&f.root), original.root);
        let retained = snapshot_all(&f).await;
        assert_eq!(
            publisher.publish(&mut attempt).await.unwrap_err(),
            FileUploadPublicationError::InvalidInput
        );
        assert_eq!(snapshot_all(&f).await, retained);
        // Historical metadata survives source/journal loss; it cannot recreate a receipt.
        f.source_pool.close().await;
        f.journal_pool.close().await;
        record_metadata(
            &f.publication
                .get(upload(1), owner(1), account(1))
                .await
                .unwrap()
                .unwrap(),
            1,
            &f.manifest,
        );
        assert_eq!(snapshot_all(&f).await, retained);
        pool.close().await;
        f.cleanup().await;
    }
    let f = Fixture::ready(17).await;
    stage_all(&f).await;
    publish(&f, &f.publisher, 1, &f.manifest).await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_independently_visible_ack_then_original_inode_or_root_drift_is_unconfirmed() {
    for drift in ["inode", "root"] {
        let f = Fixture::ready(17).await;
        stage_all(&f).await;
        let original = snapshot_all(&f).await;
        let (publisher, pool, release, started) = pool_gate::publisher(&f, Store::Source).await;
        let mut attempt = publisher
            .prepare(upload(1), owner(1), account(1))
            .await
            .unwrap();
        release.arm(&pool).await;
        let mut pending = Box::pin(publisher.publish(&mut attempt));
        tokio::select! {result=&mut pending=>panic!("missing original COMMIT return barrier: {result:?}"),_=signal(started)=>{}}
        let (holder, pid) = lock(&f, Store::Source).await;
        drop(release);
        tokio::select! {result=&mut pending=>panic!("missing actual post-ACK source wait: {result:?}"),_=f.wait_blocked(Store::Source,pid)=>{}}
        let acknowledged = f.rows(Store::Publication).await;
        assert_eq!(acknowledged[1].len(), 1);
        record_metadata(
            &f.publication
                .get(upload(1), owner(1), account(1))
                .await
                .unwrap()
                .unwrap(),
            1,
            &f.manifest,
        );
        let path = f.root.join(&original.files[0].name);
        let saved = f.directory.join("retained-original");
        if drift == "inode" {
            fs::rename(&path, &saved).unwrap();
            let mut output = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
                .unwrap();
            output.write_all(&fs::read(&saved).unwrap()).unwrap();
            output
                .set_permissions(Permissions::from_mode(0o400))
                .unwrap();
            assert_ne!(snapshot(&path).inode, original.files[0].inode);
            assert_eq!(snapshot(&path).digest, original.files[0].digest);
        } else {
            fs::rename(&f.root, &saved).unwrap();
            fs::DirBuilder::new().mode(0o700).create(&f.root).unwrap();
            fs::copy(saved.join(&original.files[0].name), &path).unwrap();
            assert_ne!(snapshot(&f.root).inode, snapshot(&saved).inode);
        }
        let injected = tree(&f.root);
        let injected_root = snapshot(&f.root);
        let saved_entry = snapshot(&saved);
        let saved_files = saved.is_dir().then(|| tree(&saved));
        holder.rollback().await.unwrap();
        assert_eq!(
            pending.await.unwrap_err(),
            FileUploadPublicationError::Unconfirmed,
            "late {drift}"
        );
        assert_eq!(attempt.state(), FileUploadPublicationState::Unknown);
        assert_eq!(tree(&f.root), injected);
        assert_eq!(snapshot(&f.root), injected_root);
        assert_eq!(snapshot(&saved), saved_entry);
        if let Some(saved_files) = saved_files {
            assert_eq!(tree(&saved), saved_files);
        }
        assert_eq!(f.rows(Store::Source).await, original.rows[0]);
        assert_eq!(f.rows(Store::Journal).await, original.rows[1]);
        assert_eq!(f.rows(Store::Publication).await, acknowledged);
        for (index, store) in [Store::Source, Store::Journal, Store::Publication]
            .into_iter()
            .enumerate()
        {
            assert_eq!(f.oids(store).await, original.oids[index]);
        }
        let retained = snapshot_all(&f).await;
        assert_eq!(
            publisher.clone().publish(&mut attempt).await.unwrap_err(),
            FileUploadPublicationError::InvalidInput
        );
        assert_eq!(snapshot_all(&f).await, retained);
        // Current namespace damage cannot turn a historical get into a file health assertion.
        record_metadata(
            &f.publication
                .get(upload(1), owner(1), account(1))
                .await
                .unwrap()
                .unwrap(),
            1,
            &f.manifest,
        );
        assert_eq!(snapshot_all(&f).await, retained);
        pool.close().await;
        f.cleanup().await;
    }
    let f = Fixture::ready(17).await;
    stage_all(&f).await;
    publish(&f, &f.publisher, 1, &f.manifest).await;
    f.cleanup().await;
}
