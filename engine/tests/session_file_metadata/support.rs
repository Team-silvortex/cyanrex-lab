use super::{fixture::*, *};
use std::time::Duration;

pub type Tx<'a> = sqlx_core::transaction::Transaction<'a, sqlx_postgres::Postgres>;
#[derive(Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub rows: Vec<Vec<(String, Vec<String>)>>,
    pub oids: Vec<Vec<(String, i64)>>,
}
pub async fn snapshot(f: &Fixture) -> Snapshot {
    let mut rows = Vec::new();
    let mut oids = Vec::new();
    for schema in [&f.auth.base.schema, &f.upload_schema, &f.publication_schema] {
        rows.push(f.rows(schema).await);
        oids.push(f.oids(schema).await);
    }
    Snapshot { rows, oids }
}
/// Capture injected rows/OIDs on the writer's own transaction BEFORE acknowledging its commit.
pub async fn snapshot_in_tx(f: &Fixture, tx: &mut Tx<'_>) -> Snapshot {
    let mut rows = Vec::new();
    let mut oids = Vec::new();
    for schema in [&f.auth.base.schema, &f.upload_schema, &f.publication_schema] {
        let objects:Vec<(String,i64)>=query("SELECT relname AS n,oid::bigint AS oid FROM pg_class WHERE relnamespace=to_regnamespace($1) AND relkind='r' ORDER BY relname")
            .bind(schema).fetch_all(&mut **tx).await.unwrap().into_iter().map(|r|(r.get("n"),r.get("oid"))).collect();
        let mut tables = Vec::new();
        for (name, _) in &objects {
            assert!(name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_'));
            let mut values: Vec<String> = query(&format!(
                "SELECT row_to_json(t)::text AS v FROM {schema}.{name} t"
            ))
            .fetch_all(&mut **tx)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.get("v"))
            .collect();
            values.sort();
            tables.push((name.clone(), values));
        }
        rows.push(tables);
        oids.push(objects);
    }
    Snapshot { rows, oids }
}
pub async fn reserve(
    f: &Fixture,
    token: &str,
    seed: u128,
    m: &FileTransferManifest,
) -> Result<FileUploadReservation, SessionFileMetadataError> {
    let before = snapshot(f).await;
    let result = f
        .auth
        .source
        .reserve_session_file_upload(token, &f.upload_workspace, upload(seed), m)
        .await;
    let after = snapshot(f).await;
    assert_eq!(after.rows[0], before.rows[0]);
    assert_eq!(after.rows[2], before.rows[2]);
    assert_eq!(after.oids, before.oids);
    if result.is_err() {
        assert_eq!(after, before);
    } else {
        let before_marker = before.rows[1]
            .iter()
            .find(|(name, _)| name == UPLOAD_MARKER)
            .unwrap();
        let after_marker = after.rows[1]
            .iter()
            .find(|(name, _)| name == UPLOAD_MARKER)
            .unwrap();
        assert_eq!(after_marker, before_marker);
        let old = &before.rows[1]
            .iter()
            .find(|(name, _)| name == UPLOAD_ROWS)
            .unwrap()
            .1;
        let new = &after.rows[1]
            .iter()
            .find(|(name, _)| name == UPLOAD_ROWS)
            .unwrap()
            .1;
        assert_eq!(new.len(), old.len() + 1);
        for row in old {
            assert!(new.contains(row));
        }
    }
    result
}
pub async fn get_upload(
    f: &Fixture,
    token: &str,
    seed: u128,
) -> Result<Option<FileUploadReservation>, SessionFileMetadataError> {
    let before = snapshot(f).await;
    let result = f
        .auth
        .source
        .get_session_file_upload(token, &f.upload_workspace, upload(seed))
        .await;
    assert_eq!(snapshot(f).await, before);
    result
}
pub async fn get_publication(
    f: &Fixture,
    token: &str,
    seed: u128,
) -> Result<Option<FileUploadPublicationRecord>, SessionFileMetadataError> {
    let before = snapshot(f).await;
    let result = f
        .auth
        .source
        .get_session_file_publication(token, &f.publication_workspace, upload(seed))
        .await;
    assert_eq!(snapshot(f).await, before);
    result
}
pub async fn backend(tx: &mut Tx<'_>) -> i32 {
    query("SELECT pg_backend_pid() AS n")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
        .get("n")
}
pub async fn fault(f: &Fixture, body: &str, deferred: bool) {
    query("CREATE SEQUENCE metadata_fault_hits")
        .execute(&f.upload_pool)
        .await
        .unwrap();
    query(&format!(
        "CREATE FUNCTION metadata_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.metadata_fault_hits'); {body} RETURN NEW; END $$",
        f.upload_schema
    ))
    .execute(&f.upload_pool)
    .await
    .unwrap();
    query(&if deferred{format!("CREATE CONSTRAINT TRIGGER metadata_fault AFTER INSERT ON {UPLOAD_ROWS} DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION metadata_fault()")}
        else{format!("CREATE TRIGGER metadata_fault BEFORE INSERT ON {UPLOAD_ROWS} FOR EACH ROW EXECUTE FUNCTION metadata_fault()")})
        .execute(&f.upload_pool).await.unwrap();
}
pub async fn hit(f: &Fixture) -> bool {
    query("SELECT is_called FROM metadata_fault_hits")
        .fetch_one(&f.upload_pool)
        .await
        .unwrap()
        .get("is_called")
}
pub async fn remove_fault(f: &Fixture) {
    for sql in [
        format!("DROP TRIGGER metadata_fault ON {UPLOAD_ROWS}"),
        "DROP FUNCTION metadata_fault()".into(),
        "DROP SEQUENCE metadata_fault_hits".into(),
    ] {
        query(&sql).execute(&f.upload_pool).await.unwrap();
    }
}
pub async fn advisory(f: &Fixture) -> (Tx<'_>, i32) {
    let mut tx = f.auth.base.admin.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext($1),778)")
        .bind(&f.upload_schema)
        .execute(&mut *tx)
        .await
        .unwrap();
    let pid = backend(&mut tx).await;
    (tx, pid)
}
pub async fn drain(f: &Fixture) {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut tx = f.auth.base.admin.begin().await.unwrap();
        query(&format!(
            "LOCK TABLE {}.{UPLOAD_MARKER},{}.{UPLOAD_ROWS} IN ACCESS EXCLUSIVE MODE",
            f.upload_schema, f.upload_schema
        ))
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.rollback().await.unwrap();
        f.pool_restored().await;
    })
    .await
    .expect("cancelled source transaction did not fully release its resource locks");
}
