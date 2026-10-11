use super::{
    fixture::*,
    schema::{checks, restore},
    support::*,
};
use cyanrex_engine::models::collaboration::MAX_FILE_TRANSFER_MANIFEST_BYTES;
use cyanrex_engine::services::{
    file_upload_publication::FileUploadPublicationStoreError,
    file_upload_staging::{FileUploadPublicationError, FileUploadPublicationState},
};
use sqlx_core::{query::query, row::Row};

pub async fn exercise() {
    for damage in [
        "manifest",
        "oversized",
        "noncanonical",
        "source",
        "root",
        "journal",
        "nonce",
        "file",
        "version",
    ] {
        let f = Fixture::ready(17).await;
        stage_all(&f).await;
        healthy(&f, 2).await;
        let row=query(&format!("SELECT manifest,source_key,root_key,journal_key,nonce,file_id,version_id FROM {PUBLICATION_ROWS} WHERE upload_id=$1"))
            .bind(upload(2).as_uuid()).fetch_one(&f.publication_pool).await.unwrap();
        let saved = checks(&f).await;
        let assignment = match damage {
            "manifest" => "manifest=decode('7b7d','hex')".to_owned(),
            "oversized" => format!(
                "manifest=decode(repeat('00',{}),'hex')",
                MAX_FILE_TRANSFER_MANIFEST_BYTES + 1
            ),
            "noncanonical" => "manifest=manifest||decode('20','hex')".to_owned(),
            "source" => "source_key=decode('ab','hex')".to_owned(),
            "root" => "root_key=decode('cd','hex')".to_owned(),
            "journal" => "journal_key=decode('ef','hex')".to_owned(),
            "nonce" => "nonce='00000000-0000-0000-0000-000000000000'::uuid".to_owned(),
            "file" => format!("file_id='{}'::uuid", uuid::Uuid::new_v4()),
            "version" => format!("version_id='{}'::uuid", uuid::Uuid::new_v4()),
            _ => unreachable!(),
        };
        f.sql(
            Store::Publication,
            &format!(
                "UPDATE {PUBLICATION_ROWS} SET {assignment} WHERE upload_id='{}'::uuid",
                upload(2).as_uuid()
            ),
        )
        .await;
        let before = snapshot_all(&f).await;
        let mut attempt = f
            .publisher
            .prepare(upload(1), owner(1), account(1))
            .await
            .unwrap();
        assert_eq!(
            f.publisher.publish(&mut attempt).await.unwrap_err(),
            FileUploadPublicationError::Publication(FileUploadPublicationStoreError::InvalidRecord),
            "{damage}"
        );
        assert_eq!(attempt.state(), FileUploadPublicationState::Unknown);
        assert_eq!(
            f.publication.get(upload(2), owner(2), account(2)).await,
            Err(FileUploadPublicationStoreError::InvalidRecord)
        );
        assert_eq!(snapshot_all(&f).await, before);
        query(&format!("UPDATE {PUBLICATION_ROWS} SET manifest=$1,source_key=$2,root_key=$3,journal_key=$4,nonce=$5,file_id=$6,version_id=$7 WHERE upload_id=$8"))
            .bind(row.get::<Vec<u8>,_>("manifest")).bind(row.get::<Vec<u8>,_>("source_key")).bind(row.get::<Vec<u8>,_>("root_key"))
            .bind(row.get::<Vec<u8>,_>("journal_key")).bind(row.get::<uuid::Uuid,_>("nonce"))
            .bind(row.get::<uuid::Uuid,_>("file_id")).bind(row.get::<uuid::Uuid,_>("version_id")).bind(upload(2).as_uuid())
            .execute(&f.publication_pool).await.unwrap();
        restore(&f, saved).await;
        publish(&f, &f.publisher, 1, &f.manifest).await;
        f.cleanup().await;
    }
    overflow().await;
}

async fn overflow() {
    let f = Fixture::with_limit(17, 1).await;
    stage_all(&f).await;
    healthy(&f, 2).await;
    let m = manifest(0, 3);
    query(&format!("INSERT INTO {PUBLICATION_ROWS}(upload_id,file_id,version_id,owner_id,account_id,manifest,source_key,root_key,journal_key,nonce)
        SELECT $1,$2,$3,$4,$5,$6,source_key,root_key,journal_key,$7 FROM {PUBLICATION_ROWS} WHERE upload_id=$8"))
        .bind(upload(3).as_uuid()).bind(m.file_id().as_uuid()).bind(m.version_id().as_uuid())
        .bind(owner(3).principal_id.as_uuid()).bind(account(3).as_uuid()).bind(serde_json::to_vec(&m).unwrap()).bind(uuid::Uuid::new_v4())
        .bind(upload(2).as_uuid()).execute(&f.publication_pool).await.unwrap();
    let before = snapshot_all(&f).await;
    let mut attempt = f
        .publisher
        .prepare(upload(1), owner(1), account(1))
        .await
        .unwrap();
    assert_eq!(
        f.publisher.publish(&mut attempt).await.unwrap_err(),
        FileUploadPublicationError::Publication(FileUploadPublicationStoreError::InvalidRecord)
    );
    assert_eq!(
        snapshot_all(&f).await,
        before,
        "configured max plus one persisted row is corrupt, never a zero or normal quota response"
    );
    query(&format!(
        "DELETE FROM {PUBLICATION_ROWS} WHERE upload_id=$1"
    ))
    .bind(upload(3).as_uuid())
    .execute(&f.publication_pool)
    .await
    .unwrap();
    let restored = snapshot_all(&f).await;
    let mut fresh = f
        .publisher
        .prepare(upload(1), owner(1), account(1))
        .await
        .unwrap();
    assert_eq!(
        f.publisher.publish(&mut fresh).await.unwrap_err(),
        FileUploadPublicationError::Publication(FileUploadPublicationStoreError::CapacityExhausted)
    );
    record_metadata(
        &f.publication
            .get(upload(2), owner(2), account(2))
            .await
            .unwrap()
            .unwrap(),
        2,
        &manifest(17, 2),
    );
    assert_eq!(snapshot_all(&f).await, restored);
    f.cleanup().await;
}
