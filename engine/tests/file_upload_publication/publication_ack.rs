use super::{bytes::*, control::signal, fixture::*, pool_gate, support::*};
use cyanrex_engine::services::{
    file_upload_publication::FileUploadPublicationStoreError,
    file_upload_staging::{FileUploadPublicationError, FileUploadPublicationState},
};
use sqlx_core::query::query;

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_independently_visible_ack_then_publication_row_or_schema_drift_is_unconfirmed() {
    for drift in ["nonce", "owner", "manifest", "relation"] {
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
        tokio::select! {result=&mut pending=>panic!("missing pre-INSERT connection-return barrier: {result:?}"),_=signal(started)=>{}}
        let (source_holder, source_pid) = lock(&f, Store::Source).await;
        drop(release);
        tokio::select! {result=&mut pending=>panic!("missing post-ACK source SQL wait: {result:?}"),_=f.wait_blocked(Store::Source,source_pid)=>{}}
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
        // Publication is already committed and independently visible. No source/journal/file fault
        // is injected: those post-ACK checks must pass before verify_ack rejects the original token.
        let mut mutation = f.publication_admin.begin().await.unwrap();
        let ns = &f.publication_schema;
        match drift {
            "nonce"=>{query(&format!("UPDATE {ns}.{PUBLICATION_ROWS} SET nonce=$1")).bind(uuid::Uuid::new_v4()).execute(&mut *mutation).await.unwrap();},
            "owner"=>{query(&format!("UPDATE {ns}.{PUBLICATION_ROWS} SET owner_id=$1")).bind(owner(2).principal_id.as_uuid()).execute(&mut *mutation).await.unwrap();},
            "manifest"=>{
                let mut wire=serde_json::to_value(&f.manifest).unwrap();wire["display_name"]=serde_json::json!("published-admin-change.bin");
                let changed=cyanrex_engine::models::collaboration::FileTransferManifest::parse_json(&serde_json::to_vec(&wire).unwrap()).unwrap();
                query(&format!("UPDATE {ns}.{PUBLICATION_ROWS} SET manifest=$1")).bind(serde_json::to_vec(&changed).unwrap()).execute(&mut *mutation).await.unwrap();
            },
            "relation"=>{
                for statement in [format!("ALTER TABLE {ns}.{PUBLICATION_ROWS} RENAME TO retained_published_original"),
                    format!("CREATE TABLE {ns}.{PUBLICATION_ROWS}(LIKE {ns}.retained_published_original INCLUDING ALL)"),
                    format!("INSERT INTO {ns}.{PUBLICATION_ROWS} SELECT * FROM {ns}.retained_published_original"),
                    format!("UPDATE {ns}.{PUBLICATION_MARKER} SET publications_oid='{ns}.{PUBLICATION_ROWS}'::regclass::oid::bigint")] {
                    query(&statement).execute(&mut *mutation).await.unwrap();
                }
            },_=>unreachable!()
        }
        let (injected, injected_oids) = state_in_tx(&f, Store::Publication, &mut mutation).await;
        mutation.commit().await.unwrap();
        source_holder.rollback().await.unwrap();
        assert_eq!(
            pending.await.unwrap_err(),
            FileUploadPublicationError::Unconfirmed,
            "verify original ACK after {drift}"
        );
        assert_eq!(attempt.state(), FileUploadPublicationState::Unknown);
        assert_eq!(f.rows(Store::Publication).await, injected);
        assert_eq!(f.oids(Store::Publication).await, injected_oids);
        assert_eq!(f.rows(Store::Source).await, original.rows[0]);
        assert_eq!(f.oids(Store::Source).await, original.oids[0]);
        assert_eq!(f.rows(Store::Journal).await, original.rows[1]);
        assert_eq!(f.oids(Store::Journal).await, original.oids[1]);
        assert_eq!(tree(&f.root), original.files);
        assert_eq!(snapshot(&f.root), original.root);
        let retained = snapshot_all(&f).await;
        assert_eq!(
            publisher.publish(&mut attempt).await.unwrap_err(),
            FileUploadPublicationError::InvalidInput
        );
        assert_eq!(snapshot_all(&f).await, retained);
        let mut fresh = publisher
            .prepare(upload(1), owner(1), account(1))
            .await
            .unwrap();
        assert_eq!(
            publisher.publish(&mut fresh).await.unwrap_err(),
            FileUploadPublicationError::Publication(FileUploadPublicationStoreError::Conflict)
        );
        assert_eq!(
            snapshot_all(&f).await,
            retained,
            "current matching metadata never adopts the original ACK or overwrites the row"
        );
        pool.close().await;
        f.cleanup().await;
    }
    let f = Fixture::ready(17).await;
    stage_all(&f).await;
    publish(&f, &f.publisher, 1, &f.manifest).await;
    f.cleanup().await;
}
