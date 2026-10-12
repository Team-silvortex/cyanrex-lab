use crate::{fixture::Fixture, *};
use futures_util::FutureExt;
use std::{future::Future, panic::AssertUnwindSafe, pin::Pin, time::Duration};
pub const MARKER: &str = sharing_fixture::MARKER;
pub const HEADS: &str = sharing_fixture::HEADS;
pub const CHANGES: &str = sharing_fixture::CHANGES;
pub const PUB_MARKER: &str = "collaboration_file_publication_schema";
pub const PUB_ROWS: &str = "collaboration_file_publications";
pub type Tx<'a> = sqlx_core::transaction::Transaction<'a, sqlx_postgres::Postgres>;
#[derive(Debug, PartialEq, Eq)]
pub struct Observed {
    pub file: FileVersionRef,
    pub index: u32,
    pub bytes: Vec<u8>,
}
pub type ReadResult = Result<Observed, SessionFileChunkReadError>;
pub fn auth(f: &Fixture) -> &identity_fixture::Fixture {
    &f.sharing.base.grants.auth
}
pub fn metadata(e: SessionFileDownloadError) -> SessionFileChunkReadError {
    SessionFileChunkReadError::Metadata(e)
}
pub fn session(e: SessionCommandError) -> SessionFileChunkReadError {
    metadata(SessionFileDownloadError::Session(e))
}
pub fn identity(e: IdentityStoreError) -> SessionFileChunkReadError {
    session(SessionCommandError::Identity(e))
}
pub fn recipient(e: FileRecipientGrantStoreError) -> SessionFileChunkReadError {
    metadata(SessionFileDownloadError::Recipient(e))
}
pub fn publication(e: FileUploadPublicationStoreError) -> SessionFileChunkReadError {
    metadata(SessionFileDownloadError::Publication(e))
}
pub fn denied() -> SessionFileChunkReadError {
    metadata(SessionFileDownloadError::PermissionDenied)
}
pub fn observe(result: Result<SessionFileChunkRead, SessionFileChunkReadError>) -> ReadResult {
    result.map(|r| Observed {
        file: r.file().clone(),
        index: r.chunk_index(),
        bytes: r.bytes().to_vec(),
    })
}
pub async fn checked<F>(mut f: Fixture, work: F)
where
    F: for<'a> FnOnce(&'a mut Fixture) -> Pin<Box<dyn Future<Output = ()> + 'a>>,
{
    let result = AssertUnwindSafe(work(&mut f)).catch_unwind().await;
    f.cleanup().await;
    if let Err(p) = result {
        std::panic::resume_unwind(p);
    }
}
pub async fn read_as(
    f: &Fixture,
    source: &DurableAuthSource,
    token: &str,
    w: &SessionFileChunkReadWorkspace,
    r: &SessionFileChunkReadRequest,
) -> ReadResult {
    let before = f.sharing.snapshot().await;
    let result = observe(source.read_session_file_chunk(token, w, r).await);
    assert_eq!(f.sharing.snapshot().await,before,"byte calls must not write source/Registry/grant/upload/journal/Published rows/OIDs or staged file bytes/identity");
    result
}
pub async fn read(f: &Fixture, r: &SessionFileChunkReadRequest) -> ReadResult {
    read_as(f, &auth(f).source, &auth(f).target_token, &f.workspace, r).await
}
pub fn expected(file: FileVersionRef, index: u32, length: u64) -> Observed {
    Observed {
        file,
        index,
        bytes: (index as u64 * FILE_TRANSFER_CHUNK_BYTES
            ..index as u64 * FILE_TRANSFER_CHUNK_BYTES + length)
            .map(bytes::byte)
            .collect(),
    }
}
pub async fn healthy(f: &Fixture) {
    assert_eq!(
        read(f, &f.request()).await.unwrap(),
        expected(f.sharing.expected_file.clone(), 0, 17)
    );
}
pub fn request(f: &Fixture, seed: u128) -> SessionFileChunkReadRequest {
    let mut r = f.request();
    r.download.grant_id = sharing_fixture::recipient_grant(seed);
    r
}
pub async fn issue(
    f: &Fixture,
    seed: u128,
    publisher: FileAccountRef,
    target: FileAccountRef,
    file: FileVersionRef,
    validity: FileGrantValidity,
) -> StoredFileRecipientGrant {
    let before = f.sharing.snapshot().await;
    let issuer = f.sharing.actor();
    let record = f
        .sharing
        .store
        .issue(
            sharing_fixture::recipient_grant(seed),
            issuer,
            file.clone(),
            publisher,
            target,
            validity,
        )
        .await
        .unwrap();
    let after = f.sharing.snapshot().await;
    sharing_fixture::recipient_delta(&f.sharing.schema, &before, &after, seed, false);
    let expected = FileRecipientGrant::new(
        sharing_fixture::recipient_grant(seed),
        sharing_fixture::revision(1),
        file,
        publisher,
        target,
        validity,
        FileGrantState::Active,
    )
    .unwrap();
    assert_eq!(record.grant(), &expected);
    assert_eq!(record.issuer(), issuer);
    sharing_fixture::assert_record_rows(&after, &f.sharing.schema, &expected, issuer, issuer);
    record
}
pub async fn own_grant(f: &Fixture, seed: u128, validity: FileGrantValidity) {
    issue(
        f,
        seed,
        f.sharing.actor(),
        f.sharing.recipient,
        f.sharing.expected_file.clone(),
        validity,
    )
    .await;
}
pub async fn sql(f: &Fixture, s: &str) {
    query(s).execute(&auth(f).base.admin).await.unwrap();
}
pub async fn backend(tx: &mut Tx<'_>) -> i32 {
    query("SELECT pg_backend_pid() AS n")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
        .get("n")
}
pub async fn blocked(f: &Fixture, pid: i32) -> bool {
    tokio::time::timeout(Duration::from_secs(1),async {loop {
        let yes:bool=query("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name=$1 AND state='active' AND wait_event_type='Lock' AND $2=ANY(pg_blocking_pids(pid))) AS yes")
            .bind(&auth(f).base.schema).bind(pid).fetch_one(&auth(f).base.admin).await.unwrap().get("yes");
        if yes{return true;}tokio::task::yield_now().await;
    }}).await.unwrap_or(false)
}
pub async fn holder<'a>(f: &'a Fixture, schema: &str, table: &str) -> Tx<'a> {
    let mut tx = auth(f).base.admin.begin().await.unwrap();
    query(&format!("SELECT * FROM {schema}.{table} FOR UPDATE"))
        .fetch_all(&mut *tx)
        .await
        .unwrap();
    tx
}
pub async fn snapshot_in_tx(f: &Fixture, tx: &mut Tx<'_>) -> sharing_fixture::Snapshot {
    let mut database = Vec::new();
    for schema in [
        &auth(f).base.schema,
        &f.sharing.base.grants.schema,
        &f.sharing.base.upload_schema,
        &f.sharing.base.journal_schema,
        &f.sharing.base.publication_schema,
    ] {
        database.extend(rows(schema, tx).await);
    }
    sharing_fixture::Snapshot {
        base: publication_fixture::Snapshot {
            database: grant_fixture::Snapshot(database),
            root: bytes::snapshot(&f.sharing.base.root),
            files: bytes::tree(&f.sharing.base.root),
        },
        recipient: rows(&f.sharing.schema, tx).await,
    }
}
async fn rows(schema: &str, tx: &mut Tx<'_>) -> Vec<(String, i64, Vec<String>)> {
    let tables=query("SELECT relname AS n,oid::bigint AS oid FROM pg_class WHERE relnamespace=to_regnamespace($1) AND relkind='r' ORDER BY relname").bind(schema).fetch_all(&mut **tx).await.unwrap();
    let mut result = Vec::new();
    for t in tables {
        let n: String = t.get("n");
        assert!(n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
        let rows = query(&format!(
            "SELECT to_jsonb(t)::text AS v FROM {schema}.{n} t ORDER BY to_jsonb(t)::text"
        ))
        .fetch_all(&mut **tx)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.get("v"))
        .collect();
        result.push((format!("{schema}.{n}"), t.get("oid"), rows));
    }
    result
}
pub async fn drain(f: &Fixture) {
    tokio::time::timeout(Duration::from_secs(5),async {
        let mut tx=auth(f).base.admin.begin().await.unwrap();query(&format!("LOCK TABLE {}.{MARKER},{}.{HEADS},{}.{CHANGES},{}.{PUB_MARKER},{}.{PUB_ROWS} IN ACCESS EXCLUSIVE MODE",f.sharing.schema,f.sharing.schema,f.sharing.schema,f.sharing.base.publication_schema,f.sharing.base.publication_schema)).execute(&mut *tx).await.unwrap();tx.rollback().await.unwrap();auth(f).base.drain().await;
    }).await.expect("cancelled bytes must release SQL fences/private response capacity");
}
pub async fn membership(f: &Fixture, target: PrincipalId, status: MembershipStatus) {
    let current = auth(f).current(target).await;
    let mut desired = current.policy;
    desired.membership.status = status;
    auth(f)
        .source
        .apply_session_policy_command(
            &auth(f).manager_token,
            &SessionPolicyCommand {
                command_id: id(),
                expected_revision: Some(current.revision),
                desired,
            },
        )
        .await
        .unwrap();
}
pub async fn retire(f: &Fixture, bound: &StoredLegacyIdentity) {
    auth(f)
        .store
        .apply_legacy_identity_command(&LegacyIdentityCommand {
            command_id: id(),
            actor: auth(f).manager.principal.reference,
            workspace: scope(),
            action: LegacyIdentityAction::Retire {
                username: bound.binding.username.clone(),
                account_id: bound.account_id,
                expected_principal: bound.binding.principal_id,
            },
        })
        .await
        .unwrap();
}
pub async fn configure(f: &Fixture, readonly: bool) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = auth(f).base.pool.acquire().await.unwrap();
        for statement in [
            "SET default_transaction_isolation='repeatable read'",
            "SET synchronous_commit='off'",
            if readonly {
                "SET default_transaction_read_only='on'"
            } else {
                "SET default_transaction_read_only='off'"
            },
        ] {
            query(statement).execute(&mut *c).await.unwrap();
        }
        held.push(c);
    }
}
pub async fn settings(f: &Fixture, readonly: bool) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = auth(f).base.pool.acquire().await.unwrap();
        let r=query("SELECT current_setting('default_transaction_isolation') AS i,current_setting('default_transaction_read_only') AS r,current_setting('synchronous_commit') AS s").fetch_one(&mut *c).await.unwrap();
        assert_eq!(r.get::<String, _>("i"), "repeatable read");
        assert_eq!(r.get::<String, _>("r"), if readonly { "on" } else { "off" });
        assert_eq!(r.get::<String, _>("s"), "off");
        held.push(c);
    }
}
pub async fn now(f: &Fixture) -> u64 {
    query("SELECT floor(extract(epoch FROM clock_timestamp()))::bigint AS n")
        .fetch_one(&auth(f).base.admin)
        .await
        .unwrap()
        .get::<i64, _>("n") as u64
}
pub async fn arrived(f: &Fixture, second: u64) -> bool {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if now(f).await >= second {
                return true;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or(false)
}
pub async fn expire_soon(f: &Fixture) {
    query("UPDATE sessions SET expires_at=clock_timestamp()+INTERVAL '1.3 seconds' WHERE token=$1")
        .bind(source_fixture::token_hash(&auth(f).target_token))
        .execute(&auth(f).base.pool)
        .await
        .unwrap();
}
pub async fn expired(f: &Fixture) -> bool {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let yes: bool = query(&format!(
                "SELECT expires_at<=clock_timestamp() AS yes FROM {}.sessions WHERE token=$1",
                auth(f).base.schema
            ))
            .bind(source_fixture::token_hash(&auth(f).target_token))
            .fetch_one(&auth(f).base.admin)
            .await
            .unwrap()
            .get("yes");
            if yes {
                return true;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or(false)
}
pub async fn refresh_expiry(f: &Fixture) {
    query("UPDATE sessions SET expires_at=clock_timestamp()+INTERVAL '12 hours' WHERE token=$1")
        .bind(source_fixture::token_hash(&auth(f).target_token))
        .execute(&auth(f).base.pool)
        .await
        .unwrap();
}
