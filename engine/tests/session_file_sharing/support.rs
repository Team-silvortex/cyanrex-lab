use crate::{fixture::*, *};
use futures_util::FutureExt;
use std::{future::Future, panic::AssertUnwindSafe, pin::Pin, time::Duration};
pub const PUB_MARKER: &str = "collaboration_file_publication_schema";
pub const PUB_ROWS: &str = "collaboration_file_publications";
pub type Tx<'a> = sqlx_core::transaction::Transaction<'a, sqlx_postgres::Postgres>;
pub type CommandResult = Result<StoredFileRecipientGrant, SessionFileSharingError>;
pub fn session(e: SessionCommandError) -> SessionFileSharingError {
    SessionFileSharingError::Session(e)
}
pub fn identity(e: IdentityStoreError) -> SessionFileSharingError {
    session(SessionCommandError::Identity(e))
}
pub fn recipient(e: FileRecipientGrantStoreError) -> SessionFileSharingError {
    SessionFileSharingError::Recipient(e)
}
pub fn publisher(e: FilePublisherGrantStoreError) -> SessionFileSharingError {
    SessionFileSharingError::Publisher(e)
}
pub fn publication(e: FileUploadPublicationStoreError) -> SessionFileSharingError {
    SessionFileSharingError::Publication(e)
}
pub async fn checked<T, F>(mut f: Fixture, work: F) -> T
where
    F: for<'a> FnOnce(&'a mut Fixture) -> Pin<Box<dyn Future<Output = T> + 'a>>,
{
    let result = AssertUnwindSafe(work(&mut f)).catch_unwind().await;
    f.cleanup().await;
    match result {
        Ok(v) => v,
        Err(p) => std::panic::resume_unwind(p),
    }
}
pub async fn issue_as(
    f: &Fixture,
    token: &str,
    w: &SessionFileSharingWorkspace,
    c: &SessionFileRecipientGrantIssue,
) -> CommandResult {
    let before = f.snapshot().await;
    let result = f
        .base
        .grants
        .auth
        .source
        .issue_session_file_recipient_grant(token, w, c)
        .await;
    let after = f.snapshot().await;
    if let Ok(record) = &result {
        let seed = c.grant_id.as_uuid().as_u128() - 1000;
        recipient_delta(&f.schema, &before, &after, seed, false);
        // Expected values come from the original command + fixture's selected server owner,
        // never from a post-command database observation or returned payload.
        let expected = FileRecipientGrant::new(
            c.grant_id,
            revision(1),
            c.expected_file.clone(),
            f.actor(),
            c.recipient,
            c.validity,
            FileGrantState::Active,
        )
        .unwrap();
        assert_eq!(record.grant(), &expected);
        assert_eq!(record.issuer(), f.actor());
        assert_record_rows(&after, &f.schema, &expected, f.actor(), f.actor());
    } else {
        assert_eq!(after, before);
    }
    result
}
pub async fn issue(f: &Fixture, seed: u128) -> CommandResult {
    issue_as(
        f,
        &f.base.grants.auth.learner_token,
        &f.workspace,
        &f.command(seed),
    )
    .await
}
pub async fn revoke_as(
    f: &Fixture,
    token: &str,
    seed: u128,
    expected: RevisionNumber,
) -> CommandResult {
    let before = f.snapshot().await;
    let result = f
        .base
        .grants
        .auth
        .source
        .revoke_session_file_recipient_grant(token, &f.workspace, recipient_grant(seed), expected)
        .await;
    let after = f.snapshot().await;
    if let Ok(record) = &result {
        recipient_delta(&f.schema, &before, &after, seed, true);
        // Retain the complete immutable tuple from the snapshot captured BEFORE the command.
        let (old, original) = recipient_head(&before, &f.schema, recipient_grant(seed)).unwrap();
        let issuer = FileAccountRef::new(
            PrincipalRef {
                authority_id: scope().authority_id,
                principal_id: old["issuer_id"].as_str().unwrap().parse().unwrap(),
            },
            old["issuer_account_id"].as_str().unwrap().parse().unwrap(),
        );
        let expected = FileRecipientGrant::new(
            original.id(),
            revision(2),
            original.file().clone(),
            original.publisher(),
            original.recipient(),
            original.validity(),
            FileGrantState::Revoked,
        )
        .unwrap();
        assert_eq!(record.grant(), &expected);
        assert_eq!(record.issuer(), issuer);
        assert_record_rows(&after, &f.schema, &expected, issuer, f.actor());
    } else {
        assert_eq!(after, before);
    }
    result
}
pub async fn revoke(f: &Fixture, seed: u128, expected: RevisionNumber) -> CommandResult {
    revoke_as(f, &f.base.grants.auth.learner_token, seed, expected).await
}
pub async fn get(
    f: &Fixture,
    seed: u128,
    subject: FileAccountRef,
) -> Result<Option<StoredFileRecipientGrant>, FileRecipientGrantStoreError> {
    let before = f.snapshot().await;
    let result = f.store.get(recipient_grant(seed), subject).await;
    assert_eq!(f.snapshot().await, before);
    result
}
pub async fn sql(f: &Fixture, statement: &str) {
    query(statement)
        .execute(&f.base.grants.auth.base.admin)
        .await
        .unwrap();
}
pub async fn backend(tx: &mut Tx<'_>) -> i32 {
    query("SELECT pg_backend_pid() AS n")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
        .get("n")
}
pub async fn blocked(f: &Fixture, pid: i32) -> bool {
    tokio::time::timeout(Duration::from_secs(1),async{loop{let yes:bool=query("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name=$1 AND state='active' AND wait_event_type='Lock' AND $2=ANY(pg_blocking_pids(pid))) AS yes").bind(&f.base.grants.auth.base.schema).bind(pid).fetch_one(&f.base.grants.auth.base.admin).await.unwrap().get("yes");if yes{return true;}tokio::task::yield_now().await;}}).await.unwrap_or(false)
}
pub async fn marker_holder<'a>(f: &'a Fixture, schema: &str, table: &str, write: bool) -> Tx<'a> {
    let mut tx = f.base.grants.auth.base.admin.begin().await.unwrap();
    if write {
        query(&format!(
            "LOCK TABLE {schema}.{table} IN ROW EXCLUSIVE MODE"
        ))
        .execute(&mut *tx)
        .await
        .unwrap();
    }
    query(&format!(
        "SELECT singleton FROM {schema}.{table} FOR UPDATE"
    ))
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    tx
}
pub async fn drain(f: &Fixture) {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut tx = f.base.grants.auth.base.admin.begin().await.unwrap();
        query(&format!(
            "LOCK TABLE {}.{MARKER},{}.{HEADS},{}.{CHANGES} IN ACCESS EXCLUSIVE MODE",
            f.schema, f.schema, f.schema
        ))
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.rollback().await.unwrap();
        f.base.grants.auth.base.drain().await;
    })
    .await
    .expect("sharing cancellation must release its source and all ledger locks");
}
pub async fn configure(f: &Fixture, readonly: bool) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = f.base.grants.auth.base.pool.acquire().await.unwrap();
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
        let mut c = f.base.grants.auth.base.pool.acquire().await.unwrap();
        let r=query("SELECT current_setting('default_transaction_isolation') AS i,current_setting('default_transaction_read_only') AS r,current_setting('synchronous_commit') AS s").fetch_one(&mut *c).await.unwrap();
        assert_eq!(r.get::<String, _>("i"), "repeatable read");
        assert_eq!(r.get::<String, _>("r"), if readonly { "on" } else { "off" });
        assert_eq!(r.get::<String, _>("s"), "off");
        held.push(c);
    }
}
pub async fn snapshot_in_tx(f: &Fixture, tx: &mut Tx<'_>) -> Snapshot {
    let mut database = Vec::new();
    for schema in [
        &f.base.grants.auth.base.schema,
        &f.base.grants.schema,
        &f.base.upload_schema,
        &f.base.journal_schema,
        &f.base.publication_schema,
    ] {
        database.extend(rows_in_tx(schema, tx).await);
    }
    Snapshot {
        base: publication_fixture::Snapshot {
            database: grant_fixture::Snapshot(database),
            root: bytes::snapshot(&f.base.root),
            files: bytes::tree(&f.base.root),
        },
        recipient: rows_in_tx(&f.schema, tx).await,
    }
}
async fn rows_in_tx(schema: &str, tx: &mut Tx<'_>) -> Vec<(String, i64, Vec<String>)> {
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
pub async fn fault(f: &Fixture, table: &str, event: &str, body: &str, deferred: bool) {
    sql(f, &format!("CREATE SEQUENCE {}.sharing_hits", f.schema)).await;
    sql(f,&format!("CREATE FUNCTION {}.sharing_fault() RETURNS trigger LANGUAGE plpgsql AS $body$ BEGIN PERFORM nextval('{}.sharing_hits'); {body} END $body$",f.schema,f.schema)).await;
    let kind = if deferred { "CONSTRAINT " } else { "" };
    let timing = if deferred { "AFTER" } else { "BEFORE" };
    let defer = if deferred {
        "DEFERRABLE INITIALLY DEFERRED"
    } else {
        ""
    };
    sql(f,&format!("CREATE {kind}TRIGGER sharing_fault {timing} {event} ON {}.{table} {defer} FOR EACH ROW EXECUTE FUNCTION {}.sharing_fault()",f.schema,f.schema)).await;
}
pub async fn hit(f: &Fixture) -> bool {
    query(&format!(
        "SELECT is_called AS yes FROM {}.sharing_hits",
        f.schema
    ))
    .fetch_one(&f.base.grants.auth.base.admin)
    .await
    .unwrap()
    .get("yes")
}
pub async fn remove_fault(f: &Fixture, table: &str) {
    sql(
        f,
        &format!("DROP TRIGGER sharing_fault ON {}.{table}", f.schema),
    )
    .await;
    sql(f, &format!("DROP FUNCTION {}.sharing_fault()", f.schema)).await;
    sql(f, &format!("DROP SEQUENCE {}.sharing_hits", f.schema)).await;
}
pub async fn membership(f: &Fixture, principal: PrincipalId, status: MembershipStatus) {
    let current = f.base.grants.auth.current(principal).await;
    let mut desired = current.policy;
    desired.membership.status = status;
    f.base
        .grants
        .auth
        .source
        .apply_session_policy_command(
            &f.base.grants.auth.manager_token,
            &SessionPolicyCommand {
                command_id: id(),
                expected_revision: Some(current.revision),
                desired,
            },
        )
        .await
        .unwrap();
}
pub async fn retire(f: &Fixture, identity: &StoredLegacyIdentity) {
    f.base
        .grants
        .auth
        .store
        .apply_legacy_identity_command(&LegacyIdentityCommand {
            command_id: id(),
            actor: f.base.grants.auth.manager.principal.reference,
            workspace: scope(),
            action: LegacyIdentityAction::Retire {
                username: identity.binding.username.clone(),
                account_id: identity.account_id,
                expected_principal: identity.binding.principal_id,
            },
        })
        .await
        .unwrap();
}
pub async fn expire_soon(f: &Fixture) {
    query("UPDATE sessions SET expires_at=clock_timestamp()+INTERVAL '1.3 seconds' WHERE token=$1")
        .bind(source_fixture::token_hash(
            &f.base.grants.auth.learner_token,
        ))
        .execute(&f.base.grants.auth.base.pool)
        .await
        .unwrap();
}
pub async fn expired(f: &Fixture) -> bool {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let yes: bool = query(&format!(
                "SELECT expires_at<=clock_timestamp() AS yes FROM {}.sessions WHERE token=$1",
                f.base.grants.auth.base.schema
            ))
            .bind(source_fixture::token_hash(
                &f.base.grants.auth.learner_token,
            ))
            .fetch_one(&f.base.grants.auth.base.admin)
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
        .bind(source_fixture::token_hash(
            &f.base.grants.auth.learner_token,
        ))
        .execute(&f.base.grants.auth.base.pool)
        .await
        .unwrap();
}
