use super::{bytes::*, fixture::*};
use cyanrex_engine::{
    models::collaboration::*,
    services::file_upload_staging::{
        FileUploadStaging, UploadFileObservation, UploadFileVerificationError,
    },
};
use sqlx_core::{query::query, row::Row};

pub async fn stage_all(fixture: &Fixture) {
    use std::io::{Read, Seek, SeekFrom};
    for index in 0..fixture.manifest.chunk_sha256().len() as u32 {
        let mut file = fixture.real_file(fixture.manifest.byte_length());
        file.seek(SeekFrom::Start(
            u64::from(index) * FILE_TRANSFER_CHUNK_BYTES,
        ))
        .unwrap();
        let mut attempt = fixture
            .adapter
            .prepare(upload(1), owner(1), account(1), index)
            .await
            .unwrap();
        let receipt = fixture
            .adapter
            .stage_chunk(
                &mut attempt,
                file.take(fixture.manifest.chunk(index).unwrap().byte_length),
            )
            .await
            .unwrap();
        drop(receipt);
    }
}
pub fn metadata(observation: &UploadFileObservation, manifest: &FileTransferManifest) {
    assert_eq!(observation.upload_id(), upload(1));
    assert_eq!(observation.owner(), owner(1));
    assert_eq!(observation.account_id(), account(1));
    assert_eq!(observation.manifest(), manifest);
}
pub async fn good(fixture: &Fixture, adapter: &FileUploadStaging) {
    let rows = fixture.rows().await;
    let oids = fixture.oids().await;
    let files = tree(&fixture.root);
    metadata(
        &adapter
            .verify_file(upload(1), owner(1), account(1))
            .await
            .unwrap(),
        &fixture.manifest,
    );
    assert_eq!(fixture.rows().await, rows);
    assert_eq!(fixture.oids().await, oids);
    assert_eq!(tree(&fixture.root), files);
}
pub async fn bad(fixture: &Fixture, error: UploadFileVerificationError) {
    let rows = fixture.rows().await;
    let oids = fixture.oids().await;
    let files = tree(&fixture.root);
    assert_eq!(
        fixture
            .adapter
            .verify_file(upload(1), owner(1), account(1))
            .await
            .unwrap_err(),
        error
    );
    assert_eq!(fixture.rows().await, rows);
    assert_eq!(fixture.oids().await, oids);
    assert_eq!(tree(&fixture.root), files);
}
pub type Tx<'a> = sqlx_core::transaction::Transaction<'a, sqlx_postgres::Postgres>;
pub async fn lock(fixture: &Fixture) -> (Tx<'_>, i32) {
    let mut tx = fixture.admin.begin().await.unwrap();
    query(&format!(
        "LOCK TABLE {}.{MARKER},{}.{LEDGER} IN ACCESS EXCLUSIVE MODE",
        fixture.schema, fixture.schema
    ))
    .execute(&mut *tx)
    .await
    .unwrap();
    let pid: i32 = query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut *tx)
        .await
        .unwrap()
        .get("pid");
    (tx, pid)
}
pub async fn state_in_tx(fixture: &Fixture, tx: &mut Tx<'_>) -> (Vec<Vec<String>>, Vec<i64>) {
    let mut rows = Vec::new();
    let mut oids = Vec::new();
    for table in [MARKER, LEDGER] {
        let mut values: Vec<String> = query(&format!(
            "SELECT row_to_json(t)::text AS value FROM {}.{table} t",
            fixture.schema
        ))
        .fetch_all(&mut **tx)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get("value"))
        .collect();
        values.sort();
        rows.push(values);
        oids.push(
            query("SELECT to_regclass($1)::oid::bigint AS value")
                .bind(format!("{}.{table}", fixture.schema))
                .fetch_one(&mut **tx)
                .await
                .unwrap()
                .get("value"),
        );
    }
    (rows, oids)
}
