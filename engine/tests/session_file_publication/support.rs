use super::{fixture::*, *};
use std::time::Duration;
pub const PUBLICATION_MARKER: &str = "collaboration_file_publication_schema";
pub const PUBLICATIONS: &str = "collaboration_file_publications";
pub type Tx<'a> = sqlx_core::transaction::Transaction<'a, sqlx_postgres::Postgres>;
pub type PublishResult = Result<FileUploadPublicationReceipt, SessionFilePublishingError>;
pub fn session_error(e: SessionCommandError) -> SessionFilePublishingError {
    SessionFilePublishingError::Session(e)
}
pub fn identity_error(e: IdentityStoreError) -> SessionFilePublishingError {
    session_error(SessionCommandError::Identity(e))
}
pub fn grant_error(e: FilePublisherGrantStoreError) -> SessionFilePublishingError {
    SessionFilePublishingError::Grant(e)
}
pub fn publisher_error(e: FileUploadPublicationError) -> SessionFilePublishingError {
    SessionFilePublishingError::Publisher(e)
}
pub async fn prepare(f: &Fixture, seed: u128) -> FileUploadPublicationAttempt {
    f.publisher
        .prepare(upload(seed), f.actor().principal(), f.actor().account_id())
        .await
        .unwrap()
}
pub async fn publish(f: &Fixture, a: &mut FileUploadPublicationAttempt) -> PublishResult {
    publish_as(f, &f.grants.auth.learner_token, &f.workspace, 1, a).await
}
pub async fn publish_as(
    f: &Fixture,
    token: &str,
    w: &SessionFilePublishingWorkspace,
    grant_seed: u128,
    a: &mut FileUploadPublicationAttempt,
) -> PublishResult {
    f.grants
        .auth
        .source
        .publish_session_file_upload(token, w, grant_fixture::grant(grant_seed), a)
        .await
}
pub fn receipt(
    r: &FileUploadPublicationReceipt,
    seed: u128,
    actor: FileAccountRef,
    m: &FileTransferManifest,
) {
    assert_eq!(r.upload_id(), upload(seed));
    assert_eq!(r.owner(), actor.principal());
    assert_eq!(r.account_id(), actor.account_id());
    assert_eq!(r.manifest(), m);
}
pub fn delta(f: &Fixture, before: &Snapshot, after: &Snapshot, seed: u128) {
    assert_eq!(after.root, before.root);
    assert_eq!(after.files, before.files);
    assert_eq!(after.database.0.len(), before.database.0.len());
    for (name, oid, old) in &before.database.0 {
        let (_, new_oid, new) = after.database.0.iter().find(|(n, _, _)| n == name).unwrap();
        assert_eq!(new_oid, oid);
        if name == &format!("{}.{PUBLICATIONS}", f.publication_schema) {
            assert_eq!(new.len(), old.len() + 1);
            for r in old {
                assert!(new.contains(r));
            }
            let v: serde_json::Value =
                serde_json::from_str(new.iter().find(|r| !old.contains(r)).unwrap()).unwrap();
            assert_eq!(v["upload_id"], serde_json::json!(upload(seed)));
        } else {
            assert_eq!(
                new, old,
                "read-only source/Registry/grant/intent/journal/foreign rows"
            );
        }
    }
}
pub async fn healthy(f: &Fixture) -> FileUploadPublicationReceipt {
    let mut a = prepare(f, 1).await;
    let before = f.snapshot().await;
    let r = publish(f, &mut a).await.unwrap();
    delta(f, &before, &f.snapshot().await, 1);
    assert_eq!(a.state(), FileUploadPublicationState::Confirmed);
    receipt(&r, 1, f.actor(), &f.manifest);
    r
}
pub async fn backend(tx: &mut Tx<'_>) -> i32 {
    query("SELECT pg_backend_pid() AS n")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
        .get("n")
}
pub async fn blocked(f: &Fixture, pid: i32) -> bool {
    tokio::time::timeout(Duration::from_secs(2),async{loop{let yes:bool=query("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name=$1 AND wait_event_type='Lock' AND $2=ANY(pg_blocking_pids(pid))) AS yes")
        .bind(&f.grants.auth.base.schema).bind(pid).fetch_one(&f.grants.auth.base.admin).await.unwrap().get("yes");if yes{return true;}tokio::task::yield_now().await;}}).await.unwrap_or(false)
}
pub async fn marker_holder(f: &Fixture, write: bool) -> Tx<'_> {
    let mut tx = f.grants.auth.base.admin.begin().await.unwrap();
    if write {
        query(&format!(
            "LOCK TABLE {}.{PUBLICATION_MARKER} IN ROW EXCLUSIVE MODE",
            f.publication_schema
        ))
        .execute(&mut *tx)
        .await
        .unwrap();
    }
    query(&format!(
        "SELECT singleton FROM {}.{PUBLICATION_MARKER} FOR UPDATE",
        f.publication_schema
    ))
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    tx
}
pub async fn sql(f: &Fixture, statement: &str) {
    query(statement)
        .execute(&f.grants.auth.base.admin)
        .await
        .unwrap();
}
pub async fn fault(f: &Fixture, body: &str, deferred: bool) {
    sql(
        f,
        &format!("CREATE SEQUENCE {}.publication_hits", f.publication_schema),
    )
    .await;
    sql(f,&format!("CREATE FUNCTION {}.publication_fault() RETURNS trigger LANGUAGE plpgsql AS $b$ BEGIN PERFORM nextval('{}.publication_hits'); {body} END $b$",f.publication_schema,f.publication_schema)).await;
    let c = if deferred { "CONSTRAINT " } else { "" };
    let t = if deferred { "AFTER" } else { "BEFORE" };
    let d = if deferred {
        "DEFERRABLE INITIALLY DEFERRED"
    } else {
        ""
    };
    sql(f,&format!("CREATE {c}TRIGGER publication_fault {t} INSERT ON {}.{PUBLICATIONS} {d} FOR EACH ROW EXECUTE FUNCTION {}.publication_fault()",f.publication_schema,f.publication_schema)).await;
}
pub async fn hit(f: &Fixture) -> bool {
    query(&format!(
        "SELECT is_called AS n FROM {}.publication_hits",
        f.publication_schema
    ))
    .fetch_one(&f.grants.auth.base.admin)
    .await
    .unwrap()
    .get("n")
}
pub async fn remove_fault(f: &Fixture) {
    sql(
        f,
        &format!(
            "DROP TRIGGER publication_fault ON {}.{PUBLICATIONS}",
            f.publication_schema
        ),
    )
    .await;
    sql(
        f,
        &format!("DROP FUNCTION {}.publication_fault()", f.publication_schema),
    )
    .await;
    sql(
        f,
        &format!("DROP SEQUENCE {}.publication_hits", f.publication_schema),
    )
    .await;
}
pub async fn drain(f: &Fixture) {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut tx = f.grants.auth.base.admin.begin().await.unwrap();
        query(&format!(
            "LOCK TABLE {}.{PUBLICATIONS} IN ACCESS EXCLUSIVE MODE",
            f.publication_schema
        ))
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.rollback().await.unwrap();
        f.grants.auth.base.drain().await;
    })
    .await
    .expect("cancelled source command must release all SQL fences");
}
pub async fn policy_status(f: &Fixture, status: MembershipStatus) {
    let current = f
        .grants
        .auth
        .current(f.actor().principal().principal_id)
        .await;
    let mut desired = current.policy;
    desired.membership.status = status;
    f.grants
        .auth
        .source
        .apply_session_policy_command(
            &f.grants.auth.manager_token,
            &SessionPolicyCommand {
                command_id: id(),
                expected_revision: Some(current.revision),
                desired,
            },
        )
        .await
        .unwrap();
}
pub async fn snapshot_in_tx(f: &Fixture, tx: &mut Tx<'_>) -> Snapshot {
    let mut values = Vec::new();
    for schema in [
        &f.grants.auth.base.schema,
        &f.grants.schema,
        &f.upload_schema,
        &f.journal_schema,
        &f.publication_schema,
    ] {
        let tables=query("SELECT relname,oid::bigint AS oid FROM pg_class WHERE relnamespace=to_regnamespace($1) AND relkind='r' ORDER BY relname").bind(schema).fetch_all(&mut **tx).await.unwrap();
        for t in tables {
            let name: String = t.get("relname");
            assert!(name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
            let rows = query(&format!(
                "SELECT to_jsonb(t)::text AS row FROM {schema}.{name} t ORDER BY to_jsonb(t)::text"
            ))
            .fetch_all(&mut **tx)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.get("row"))
            .collect();
            values.push((format!("{schema}.{name}"), t.get("oid"), rows));
        }
    }
    Snapshot {
        database: grant_fixture::Snapshot(values),
        root: bytes::snapshot(&f.root),
        files: bytes::tree(&f.root),
    }
}
