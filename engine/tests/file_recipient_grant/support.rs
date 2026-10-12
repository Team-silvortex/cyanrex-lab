use super::{fixture::*, *};
use sqlx_postgres::Postgres;
use std::time::Duration;

pub type Tx<'a> = sqlx_core::transaction::Transaction<'a, Postgres>;
pub async fn ready(max: u32) -> Fixture {
    let f = Fixture::new(max).await;
    if let Err(error) = f.store.install_empty_namespace().await {
        f.cleanup().await;
        panic!("fresh recipient fixture installer failed: {error:?}");
    }
    f
}
pub async fn snapshot_in_tx(f: &Fixture, tx: &mut Tx<'_>) -> Snapshot {
    let oids: Vec<(String,i64)> = query("SELECT relname AS n,oid::bigint AS oid FROM pg_class WHERE relnamespace=to_regnamespace($1) AND relkind='r' ORDER BY relname")
        .bind(&f.schema).fetch_all(&mut **tx).await.unwrap().into_iter().map(|r|(r.get("n"),r.get("oid"))).collect();
    let mut values = Vec::new();
    for (name, _) in &oids {
        assert!(name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
        let mut rows: Vec<String> = query(&format!(
            "SELECT row_to_json(t)::text AS v FROM {}.{name} t",
            f.schema
        ))
        .fetch_all(&mut **tx)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.get("v"))
        .collect();
        rows.sort();
        values.push((name.clone(), rows));
    }
    Snapshot { rows: values, oids }
}
pub async fn issue(
    f: &Fixture,
    seed: u128,
    issuer: FileAccountRef,
    publisher: FileAccountRef,
    recipient: FileAccountRef,
    v: FileGrantValidity,
) -> Result<StoredFileRecipientGrant, FileRecipientGrantStoreError> {
    issue_file(f, seed, issuer, file(seed), publisher, recipient, v).await
}
pub async fn issue_file(
    f: &Fixture,
    seed: u128,
    issuer: FileAccountRef,
    file: FileVersionRef,
    publisher: FileAccountRef,
    recipient: FileAccountRef,
    v: FileGrantValidity,
) -> Result<StoredFileRecipientGrant, FileRecipientGrantStoreError> {
    let before = f.snapshot().await;
    let result = f
        .store
        .issue(grant(seed), issuer, file.clone(), publisher, recipient, v)
        .await;
    let after = f.snapshot().await;
    match &result {
        Err(_) => assert_eq!(after, before),
        Ok(record) => {
            assert_eq!(record.issuer(), issuer);
            assert_eq!(record.grant().id(), grant(seed));
            assert_eq!(record.grant().revision(), revision(1));
            assert_eq!(record.grant().file(), &file);
            assert_eq!(record.grant().publisher(), publisher);
            assert_eq!(record.grant().recipient(), recipient);
            assert_eq!(record.grant().validity(), v);
            assert_eq!(record.grant().state(), FileGrantState::Active);
            assert_eq!(after.oids, before.oids);
            assert_eq!(rows(&after, MARKER), rows(&before, MARKER));
            for table in [HEADS, CHANGES] {
                assert_eq!(rows(&after, table).len(), rows(&before, table).len() + 1);
                for old in rows(&before, table) {
                    assert!(rows(&after, table).contains(old));
                }
            }
        }
    }
    result
}
pub async fn get(
    f: &Fixture,
    seed: u128,
    recipient: FileAccountRef,
) -> Result<Option<StoredFileRecipientGrant>, FileRecipientGrantStoreError> {
    let before = f.snapshot().await;
    let result = f.store.get(grant(seed), recipient).await;
    assert_eq!(f.snapshot().await, before);
    result
}
pub async fn revoke(
    f: &Fixture,
    seed: u128,
    actor: FileAccountRef,
    expected: RevisionNumber,
) -> Result<StoredFileRecipientGrant, FileRecipientGrantStoreError> {
    let before = f.snapshot().await;
    let result = f.store.revoke(grant(seed), actor, expected).await;
    let after = f.snapshot().await;
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
        for old in rows(&before, CHANGES) {
            assert!(rows(&after, CHANGES).contains(old));
        }
        for old in rows(&before, HEADS) {
            let raw: serde_json::Value = serde_json::from_str(old).unwrap();
            if raw["grant_id"] != serde_json::json!(grant(seed)) {
                assert!(rows(&after, HEADS).contains(old));
            }
        }
    }
    result
}
pub async fn sql(f: &Fixture, statement: &str) {
    query(statement).execute(&f.pool).await.unwrap();
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
        let r=query("SELECT current_setting('default_transaction_isolation') AS i,current_setting('default_transaction_read_only') AS r,current_setting('synchronous_commit') AS s").fetch_one(&mut *c).await.unwrap();
        assert_eq!(r.get::<String, _>("i"), isolation);
        assert_eq!(r.get::<String, _>("r"), readonly);
        assert_eq!(r.get::<String, _>("s"), sync);
        held.push(c);
    }
}
pub async fn restored(f: &Fixture) {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut held = Vec::new();
        for _ in 0..4 {
            let mut c = f.pool.acquire().await.unwrap();
            let r = query("SELECT current_schema() AS n,current_setting('search_path') AS p")
                .fetch_one(&mut *c)
                .await
                .unwrap();
            assert_eq!(r.get::<String, _>("n"), f.schema);
            assert_eq!(r.get::<String, _>("p"), f.schema);
            held.push(c);
        }
    })
    .await
    .expect("original pool must be usable and retain its path");
}
pub async fn backend(tx: &mut Tx<'_>) -> i32 {
    query("SELECT pg_backend_pid() AS n")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
        .get("n")
}
pub async fn blocked(f: &Fixture, holder: i32) -> bool {
    tokio::time::timeout(Duration::from_secs(1),async{loop{let n:i64=query("SELECT count(*) AS n FROM pg_stat_activity WHERE application_name=$1 AND state='active' AND wait_event_type='Lock' AND $2=ANY(pg_blocking_pids(pid))").bind(&f.schema).bind(holder).fetch_one(&f.admin).await.unwrap().get("n");if n>=1{return true;}tokio::task::yield_now().await;}}).await.unwrap_or(false)
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
    .expect("cancelled transaction locks must drain");
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
pub async fn remove_check(f: &Fixture, table: &str, mentions: &str) -> (String, String) {
    let r=query("SELECT conname AS n,pg_get_constraintdef(oid) AS d FROM pg_constraint WHERE conrelid=to_regclass($1) AND contype='c' AND pg_get_constraintdef(oid) LIKE $2").bind(format!("{}.{table}",f.schema)).bind(format!("%{mentions}%")).fetch_one(&f.pool).await.unwrap();
    let name: String = r.get("n");
    let definition: String = r.get("d");
    sql(f, &format!("ALTER TABLE {table} DROP CONSTRAINT {name}")).await;
    (name, definition)
}
pub async fn restore_check(f: &Fixture, table: &str, name: &str, definition: &str) {
    sql(
        f,
        &format!("ALTER TABLE {table} ADD CONSTRAINT {name} {definition}"),
    )
    .await;
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
pub async fn insert_raw(
    f: &Fixture,
    seed: u128,
    issuer: FileAccountRef,
    publisher: FileAccountRef,
    recipient: FileAccountRef,
) {
    let g = FileRecipientGrant::new(
        grant(seed),
        revision(1),
        file(seed),
        publisher,
        recipient,
        validity(),
        FileGrantState::Active,
    )
    .unwrap();
    let raw = serde_json::to_vec(&g).unwrap();
    query(&format!("INSERT INTO {}.{HEADS}(grant_id,recipient_id,recipient_account_id,publisher_id,publisher_account_id,issuer_id,issuer_account_id,revision,not_before,expires_at,grant_payload) VALUES($1,$2,$3,$4,$5,$6,$7,1,100,200,$8)",f.schema))
        .bind(grant(seed).as_uuid()).bind(recipient.principal().principal_id.as_uuid()).bind(recipient.account_id().as_uuid()).bind(publisher.principal().principal_id.as_uuid()).bind(publisher.account_id().as_uuid()).bind(issuer.principal().principal_id.as_uuid()).bind(issuer.account_id().as_uuid()).bind(&raw).execute(&f.pool).await.unwrap();
    query(&format!("INSERT INTO {}.{CHANGES}(grant_id,revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload) VALUES($1,1,$2,$3,100,200,100,$4)",f.schema)).bind(grant(seed).as_uuid()).bind(issuer.principal().principal_id.as_uuid()).bind(issuer.account_id().as_uuid()).bind(raw).execute(&f.pool).await.unwrap();
}
