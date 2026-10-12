use super::{
    fixture::*,
    publisher_fixture::{grant, validity, Snapshot, CHANGES, HEADS, MARKER},
    *,
};
use std::time::Duration;
pub type Tx<'a> = sqlx_core::transaction::Transaction<'a, sqlx_postgres::Postgres>;
pub type CommandResult = Result<StoredFilePublisherGrant, SessionFilePublisherGrantError>;
pub fn revision(n: u64) -> RevisionNumber {
    n.try_into().unwrap()
}
pub fn identity_error(e: IdentityStoreError) -> SessionFilePublisherGrantError {
    SessionFilePublisherGrantError::Session(SessionCommandError::Identity(e))
}
pub fn session_error(e: SessionCommandError) -> SessionFilePublisherGrantError {
    SessionFilePublisherGrantError::Session(e)
}
pub fn grant_error(e: FilePublisherGrantStoreError) -> SessionFilePublisherGrantError {
    SessionFilePublisherGrantError::Grant(e)
}
pub fn rows<'a>(s: &'a Snapshot, schema: &str, table: &str) -> &'a Vec<String> {
    &s.0.iter()
        .find(|(n, _, _)| n == &format!("{schema}.{table}"))
        .unwrap()
        .2
}
pub fn delta(f: &Fixture, before: &Snapshot, after: &Snapshot, seed: u128, revoke: bool) {
    assert_eq!(before.0.len(), after.0.len());
    for (name, oid, old) in &before.0 {
        let (_, new_oid, new) = after.0.iter().find(|(n, _, _)| n == name).unwrap();
        assert_eq!(new_oid, oid);
        if name == &format!("{}.{CHANGES}", f.base.schema) {
            assert_eq!(new.len(), old.len() + 1);
            for r in old {
                assert!(new.contains(r));
            }
        } else if name == &format!("{}.{HEADS}", f.base.schema) {
            assert_eq!(new.len(), old.len() + usize::from(!revoke));
            for r in old {
                let v: serde_json::Value = serde_json::from_str(r).unwrap();
                if !revoke || v["grant_id"] != serde_json::json!(grant(seed)) {
                    assert!(new.contains(r));
                }
            }
        } else {
            assert_eq!(
                new, old,
                "source/Registry/marker/foreign rows must remain exact"
            );
        }
    }
}
pub async fn issue_as(
    f: &Fixture,
    token: &str,
    workspace: &SessionFileGrantManagementWorkspace,
    seed: u128,
    subject: FileAccountRef,
    v: FileGrantValidity,
) -> CommandResult {
    let before = f.base.snapshot().await;
    let result = f
        .base
        .auth
        .source
        .issue_session_file_publisher_grant(token, workspace, grant(seed), subject, v)
        .await;
    let after = f.base.snapshot().await;
    if result.is_ok() {
        delta(f, &before, &after, seed, false);
    } else {
        assert_eq!(after, before);
    }
    result
}
pub async fn issue(f: &Fixture, seed: u128, subject: FileAccountRef) -> CommandResult {
    issue_as(
        f,
        &f.base.auth.learner_token,
        &f.management,
        seed,
        subject,
        validity(),
    )
    .await
}
pub async fn revoke_as(
    f: &Fixture,
    token: &str,
    workspace: &SessionFileGrantManagementWorkspace,
    seed: u128,
    expected: RevisionNumber,
) -> CommandResult {
    let before = f.base.snapshot().await;
    let result = f
        .base
        .auth
        .source
        .revoke_session_file_publisher_grant(token, workspace, grant(seed), expected)
        .await;
    let after = f.base.snapshot().await;
    if result.is_ok() {
        delta(f, &before, &after, seed, true);
    } else {
        assert_eq!(after, before);
    }
    result
}
pub async fn revoke(f: &Fixture, seed: u128, expected: RevisionNumber) -> CommandResult {
    revoke_as(f, &f.base.auth.learner_token, &f.management, seed, expected).await
}
pub async fn read(
    f: &Fixture,
    token: &str,
    seed: u128,
) -> Result<Option<StoredFilePublisherGrant>, SessionFilePublisherGrantError> {
    let before = f.base.snapshot().await;
    let result = f
        .base
        .auth
        .source
        .get_session_file_publisher_grant(token, &f.base.workspace, grant(seed))
        .await;
    assert_eq!(f.base.snapshot().await, before);
    result
}
pub async fn active_target(f: &Fixture) -> FileAccountRef {
    let bound = f.base.auth.bind_target().await;
    f.base
        .auth
        .source
        .apply_session_policy_command(
            &f.base.auth.manager_token,
            &SessionPolicyCommand {
                command_id: id(),
                expected_revision: None,
                desired: LegacyAccessPolicy {
                    membership: Membership {
                        workspace: scope(),
                        principal_id: bound.binding.principal_id,
                        role_refs: vec![],
                        status: MembershipStatus::Active,
                    },
                    deployment_granted: false,
                },
            },
        )
        .await
        .unwrap();
    FileAccountRef::new(bound.principal.reference, bound.account_id)
}
pub async fn sql(f: &Fixture, source: bool, statement: &str) {
    query(statement)
        .execute(if source {
            &f.base.auth.base.pool
        } else {
            &f.base.pool
        })
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
    tokio::time::timeout(Duration::from_secs(1),async{loop{let found:bool=query("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name=$1 AND wait_event_type='Lock' AND $2=ANY(pg_blocking_pids(pid))) AS yes")
        .bind(&f.base.auth.base.schema).bind(pid).fetch_one(&f.base.auth.base.admin).await.unwrap().get("yes");if found{return true;}tokio::task::yield_now().await;}}).await.unwrap_or(false)
}
pub async fn restored(f: &Fixture) {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut held = Vec::new();
        for _ in 0..4 {
            let mut c = f.base.auth.base.pool.acquire().await.unwrap();
            let r = query("SELECT current_schema() AS n,current_setting('search_path') AS p")
                .fetch_one(&mut *c)
                .await
                .unwrap();
            assert_eq!(r.get::<String, _>("n"), f.base.auth.base.schema);
            assert_eq!(r.get::<String, _>("p"), f.base.auth.base.schema);
            held.push(c);
        }
    })
    .await
    .expect("source connection and settings restored");
}
pub async fn drain(f: &Fixture) {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut tx = f.base.auth.base.admin.begin().await.unwrap();
        query(&format!(
            "LOCK TABLE {}.{MARKER},{}.{HEADS},{}.{CHANGES} IN ACCESS EXCLUSIVE MODE",
            f.base.schema, f.base.schema, f.base.schema
        ))
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.rollback().await.unwrap();
        restored(f).await;
    })
    .await
    .expect("cancelled source transaction released its resource locks");
}
pub async fn snapshot_in_tx(f: &Fixture, tx: &mut Tx<'_>) -> Snapshot {
    let mut values = Vec::new();
    for schema in [&f.base.auth.base.schema, &f.base.schema] {
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
    Snapshot(values)
}
pub async fn fault(f: &Fixture, table: &str, event: &str, body: &str, deferred: bool) {
    sql(f, false, "CREATE SEQUENCE management_hits").await;
    sql(f,false,&format!("CREATE FUNCTION management_fault() RETURNS trigger LANGUAGE plpgsql AS $b$ BEGIN PERFORM nextval('{}.management_hits'); {body} END $b$",f.base.schema)).await;
    let constraint = if deferred { "CONSTRAINT " } else { "" };
    let timing = if deferred { "AFTER" } else { "BEFORE" };
    let defer = if deferred {
        "DEFERRABLE INITIALLY DEFERRED"
    } else {
        ""
    };
    sql(f,false,&format!("CREATE {constraint}TRIGGER management_fault {timing} {event} ON {table} {defer} FOR EACH ROW EXECUTE FUNCTION management_fault()")).await;
}
pub async fn hit(f: &Fixture) -> bool {
    query(&format!(
        "SELECT is_called AS n FROM {}.management_hits",
        f.base.schema
    ))
    .fetch_one(&f.base.auth.base.admin)
    .await
    .unwrap()
    .get("n")
}
pub async fn remove_fault(f: &Fixture, table: &str) {
    sql(
        f,
        false,
        &format!("DROP TRIGGER management_fault ON {table}"),
    )
    .await;
    sql(f, false, "DROP FUNCTION management_fault()").await;
    sql(f, false, "DROP SEQUENCE management_hits").await;
}
pub async fn policy_status(f: &Fixture, principal: PrincipalId, status: MembershipStatus) {
    let current = f.base.auth.current(principal).await;
    let mut desired = current.policy;
    desired.membership.status = status;
    f.base
        .auth
        .source
        .apply_session_policy_command(
            &f.base.auth.manager_token,
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
        .auth
        .store
        .apply_legacy_identity_command(&LegacyIdentityCommand {
            command_id: id(),
            actor: f.base.auth.manager.principal.reference,
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
pub async fn direct_get(
    f: &Fixture,
    seed: u128,
    subject: FileAccountRef,
) -> Result<Option<StoredFilePublisherGrant>, FilePublisherGrantStoreError> {
    let before = f.base.snapshot().await;
    let result = f.base.store.get(grant(seed), subject).await;
    assert_eq!(f.base.snapshot().await, before);
    result
}
