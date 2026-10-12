use super::{fixture::*, *};
use sqlx_postgres::Postgres;
use std::time::Duration;

pub type Tx<'a> = sqlx_core::transaction::Transaction<'a, Postgres>;
pub const MARKER: &str = "collaboration_file_publisher_grant_schema";
pub const HEADS: &str = "collaboration_file_publisher_grants";
pub const CHANGES: &str = "collaboration_file_publisher_grant_changes";
#[derive(Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub rows: Vec<(String, Vec<String>)>,
    pub oids: Vec<(String, i64)>,
}
pub async fn snapshot(f: &Fixture) -> Snapshot {
    let mut tx = f.admin.begin().await.unwrap();
    let snapshot = snapshot_in_tx(f, &mut tx).await;
    tx.rollback().await.unwrap();
    snapshot
}
pub async fn snapshot_in_tx(f: &Fixture, tx: &mut Tx<'_>) -> Snapshot {
    let oids:Vec<(String,i64)>=query("SELECT relname AS n,oid::bigint AS oid FROM pg_class WHERE relnamespace=to_regnamespace($1) AND relkind='r' ORDER BY relname")
        .bind(&f.schema).fetch_all(&mut **tx).await.unwrap().into_iter().map(|r|(r.get("n"),r.get("oid"))).collect();
    let mut rows = Vec::new();
    for (table, _) in &oids {
        assert!(table
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_'));
        let mut values: Vec<String> = query(&format!(
            "SELECT row_to_json(t)::text AS v FROM {}.{table} t",
            f.schema
        ))
        .fetch_all(&mut **tx)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.get("v"))
        .collect();
        values.sort();
        rows.push((table.clone(), values));
    }
    Snapshot { rows, oids }
}
pub fn rows<'a>(snapshot: &'a Snapshot, table: &str) -> &'a Vec<String> {
    &snapshot
        .rows
        .iter()
        .find(|(name, _)| name == table)
        .expect("exact ordinary table exists")
        .1
}
pub async fn ready(max: u32) -> Fixture {
    let f = Fixture::new(max).await;
    f.store.install_empty_namespace().await.unwrap();
    f
}
pub async fn issue(
    f: &Fixture,
    seed: u128,
    issuer: FileAccountRef,
    subject: FileAccountRef,
    v: FileGrantValidity,
) -> Result<StoredFilePublisherGrant, FilePublisherGrantStoreError> {
    let before = snapshot(f).await;
    let result = f.store.issue(grant(seed), issuer, subject, v).await;
    let after = snapshot(f).await;
    if result.is_err() {
        assert_eq!(after, before);
    } else {
        assert_eq!(after.oids, before.oids);
        assert_eq!(rows(&after, MARKER), rows(&before, MARKER));
        for table in [HEADS, CHANGES] {
            assert_eq!(rows(&after, table).len(), rows(&before, table).len() + 1);
            for row in rows(&before, table) {
                assert!(rows(&after, table).contains(row));
            }
        }
    }
    result
}
pub async fn get(
    f: &Fixture,
    seed: u128,
    subject: FileAccountRef,
) -> Result<Option<StoredFilePublisherGrant>, FilePublisherGrantStoreError> {
    let before = snapshot(f).await;
    let result = f.store.get(grant(seed), subject).await;
    assert_eq!(snapshot(f).await, before);
    result
}
pub async fn revoke(
    f: &Fixture,
    seed: u128,
    actor: FileAccountRef,
    revision: RevisionNumber,
) -> Result<StoredFilePublisherGrant, FilePublisherGrantStoreError> {
    let before = snapshot(f).await;
    let result = f.store.revoke(grant(seed), actor, revision).await;
    let after = snapshot(f).await;
    if result.is_err() {
        assert_eq!(after, before);
    } else {
        assert_eq!(after.oids, before.oids);
        assert_eq!(rows(&after, MARKER), rows(&before, MARKER));
        assert_eq!(rows(&after, HEADS).len(), rows(&before, HEADS).len());
        assert_eq!(
            rows(&after, CHANGES).len(),
            rows(&before, CHANGES).len() + 1
        );
        for row in rows(&before, CHANGES) {
            assert!(rows(&after, CHANGES).contains(row));
        }
        for old in rows(&before, HEADS) {
            let parsed: serde_json::Value = serde_json::from_str(old).unwrap();
            if parsed["grant_id"] != serde_json::json!(grant(seed)) {
                assert!(rows(&after, HEADS).contains(old));
            }
        }
    }
    result
}
pub async fn sql(f: &Fixture, statement: &str) {
    query(statement).execute(&f.pool).await.unwrap();
}
pub async fn backend(tx: &mut Tx<'_>) -> i32 {
    query("SELECT pg_backend_pid() AS n")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
        .get("n")
}
pub async fn blocked(f: &Fixture, holder: i32, count: i64) -> bool {
    tokio::time::timeout(Duration::from_secs(1),async{loop{
        let n:i64=query("SELECT count(*) AS n FROM pg_stat_activity WHERE application_name=$1 AND $2=ANY(pg_blocking_pids(pid))")
            .bind(&f.schema).bind(holder).fetch_one(&f.admin).await.unwrap().get("n");
        if n>=count{return true;}tokio::task::yield_now().await;
    }}).await.unwrap_or(false)
}
pub async fn restored(f: &Fixture) {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut held = Vec::new();
        for _ in 0..4 {
            let mut c = f.pool.acquire().await.unwrap();
            let row = query("SELECT current_schema() AS n,current_setting('search_path') AS p")
                .fetch_one(&mut *c)
                .await
                .unwrap();
            assert_eq!(row.get::<String, _>("n"), f.schema);
            assert_eq!(row.get::<String, _>("p"), f.schema);
            held.push(c);
        }
    })
    .await
    .expect("grant operation leaked or changed the original pool");
}
pub async fn drain(f: &Fixture) {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut tx = f.admin.begin().await.unwrap();
        query(&format!(
            "LOCK TABLE {}.{MARKER},{}.{HEADS},{}.{CHANGES} IN ACCESS EXCLUSIVE MODE",
            f.schema, f.schema, f.schema
        ))
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.rollback().await.unwrap();
        restored(f).await;
    })
    .await
    .expect("cancelled grant transaction did not fully release its relation locks");
}
pub async fn configure(pool: &PgPool, statement: &str) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = pool.acquire().await.unwrap();
        query(statement).execute(&mut *c).await.unwrap();
        held.push(c);
    }
}
pub async fn settings(pool: &PgPool, isolation: &str, readonly: &str, sync: &str) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = pool.acquire().await.unwrap();
        let r=query("SELECT current_setting('default_transaction_isolation') AS i,current_setting('default_transaction_read_only') AS r,current_setting('synchronous_commit') AS s")
            .fetch_one(&mut *c).await.unwrap();
        assert_eq!(r.get::<String, _>("i"), isolation);
        assert_eq!(r.get::<String, _>("r"), readonly);
        assert_eq!(r.get::<String, _>("s"), sync);
        held.push(c);
    }
}
pub async fn update_payload(f: &Fixture, table: &str, seed: u128, rev: u64, payload: &[u8]) {
    query(&format!(
        "UPDATE {}.{table} SET grant_payload=$1 WHERE grant_id=$2 AND revision=$3",
        f.schema
    ))
    .bind(payload)
    .bind(grant(seed).as_uuid())
    .bind(rev as i64)
    .execute(&f.pool)
    .await
    .unwrap();
}
pub async fn head_payload(f: &Fixture, seed: u128) -> Vec<u8> {
    query(&format!(
        "SELECT grant_payload AS p FROM {}.{HEADS} WHERE grant_id=$1",
        f.schema
    ))
    .bind(grant(seed).as_uuid())
    .fetch_one(&f.pool)
    .await
    .unwrap()
    .get("p")
}
pub async fn install_trigger(f: &Fixture, table: &str, event: &str, body: &str, deferred: bool) {
    sql(f, &format!("CREATE SEQUENCE {}.fault_hits", f.schema)).await;
    sql(f,&format!("CREATE FUNCTION {}.fault() RETURNS trigger LANGUAGE plpgsql AS $body$ BEGIN PERFORM nextval('{}.fault_hits'); {body} END $body$",f.schema,f.schema)).await;
    let kind = if deferred { "CONSTRAINT " } else { "" };
    let timing = if deferred { "AFTER" } else { "BEFORE" };
    let deferral = if deferred {
        "DEFERRABLE INITIALLY DEFERRED"
    } else {
        ""
    };
    sql(f,&format!("CREATE {kind}TRIGGER fault {timing} {event} ON {}.{table} {deferral} FOR EACH ROW EXECUTE FUNCTION {}.fault()",f.schema,f.schema)).await;
}
pub async fn hits(f: &Fixture) -> bool {
    query(&format!(
        "SELECT is_called AS v FROM {}.fault_hits",
        f.schema
    ))
    .fetch_one(&f.pool)
    .await
    .unwrap()
    .get("v")
}
pub async fn drop_trigger(f: &Fixture, table: &str) {
    sql(f, &format!("DROP TRIGGER fault ON {}.{table}", f.schema)).await;
    sql(f, &format!("DROP FUNCTION {}.fault()", f.schema)).await;
    sql(f, &format!("DROP SEQUENCE {}.fault_hits", f.schema)).await;
}
pub async fn insert_raw(f: &Fixture, seed: u128, issuer: FileAccountRef, subject: FileAccountRef) {
    let g = FilePublisherGrant::new(
        grant(seed),
        revision(1),
        scope(),
        subject,
        validity(),
        FileGrantState::Active,
    )
    .unwrap();
    let payload = serde_json::to_vec(&g).unwrap();
    query(&format!("INSERT INTO {}.{HEADS} (grant_id,subject_id,subject_account_id,issuer_id,issuer_account_id,revision,not_before,expires_at,grant_payload) VALUES($1,$2,$3,$4,$5,1,100,200,$6)",f.schema))
        .bind(grant(seed).as_uuid()).bind(subject.principal().principal_id.as_uuid()).bind(subject.account_id().as_uuid())
        .bind(issuer.principal().principal_id.as_uuid()).bind(issuer.account_id().as_uuid()).bind(&payload).execute(&f.pool).await.unwrap();
    query(&format!("INSERT INTO {}.{CHANGES} (grant_id,revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload) VALUES($1,1,$2,$3,100,200,100,$4)",f.schema))
        .bind(grant(seed).as_uuid()).bind(issuer.principal().principal_id.as_uuid()).bind(issuer.account_id().as_uuid()).bind(payload).execute(&f.pool).await.unwrap();
}
