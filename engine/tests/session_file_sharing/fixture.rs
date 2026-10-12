pub use super::identity_fixture::scope;
use super::*;
pub const MARKER: &str = "collaboration_file_recipient_grant_schema";
pub const HEADS: &str = "collaboration_file_recipient_grants";
pub const CHANGES: &str = "collaboration_file_recipient_grant_changes";
pub fn recipient_grant(seed: u128) -> FileGrantId {
    Uuid::from_u128(seed + 1000).try_into().unwrap()
}
pub fn revision(n: u64) -> RevisionNumber {
    n.try_into().unwrap()
}
#[derive(Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub base: publication_fixture::Snapshot,
    pub recipient: Vec<(String, i64, Vec<String>)>,
}
pub struct Fixture {
    pub base: publication_fixture::Fixture,
    pub schema: String,
    pub pool: PgPool,
    pub store: FileRecipientGrantStore,
    pub installation_id: Uuid,
    pub recipient_workspace: SessionFileRecipientGrantWorkspace,
    pub workspace: SessionFileSharingWorkspace,
    pub recipient: FileAccountRef,
    pub expected_file: FileVersionRef,
    pub max_grants: u32,
    auxiliary_pools: std::sync::Mutex<Vec<PgPool>>,
}
impl Fixture {
    pub async fn ready() -> Self {
        Self::with_max(16).await
    }
    pub async fn with_max(max_grants: u32) -> Self {
        let base = publication_fixture::Fixture::ready(17).await;
        base.grants.issue(1, base.actor()).await;
        let mut attempt = base
            .publisher
            .prepare(
                publication_fixture::upload(1),
                base.actor().principal(),
                base.actor().account_id(),
            )
            .await
            .unwrap();
        drop(
            base.grants
                .auth
                .source
                .publish_session_file_upload(
                    &base.grants.auth.learner_token,
                    &base.workspace,
                    grant_fixture::grant(1),
                    &mut attempt,
                )
                .await
                .unwrap(),
        );
        // A real committed exact-owner Published row, not a client manifest or injected row.
        let row = base
            .publication
            .get(
                publication_fixture::upload(1),
                base.actor().principal(),
                base.actor().account_id(),
            )
            .await
            .unwrap()
            .unwrap();
        let expected_file = FileVersionRef::from_manifest(row.manifest());
        let target = base.grants.auth.bind_target().await;
        base.grants
            .auth
            .source
            .apply_session_policy_command(
                &base.grants.auth.manager_token,
                &SessionPolicyCommand {
                    command_id: id(),
                    expected_revision: None,
                    desired: LegacyAccessPolicy {
                        membership: Membership {
                            workspace: scope(),
                            principal_id: target.binding.principal_id,
                            role_refs: vec![],
                            status: MembershipStatus::Active,
                        },
                        deployment_granted: false,
                    },
                },
            )
            .await
            .unwrap();
        let recipient = FileAccountRef::new(target.principal.reference, target.account_id);
        let schema = format!("session_share_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {schema}"))
            .execute(&base.grants.auth.base.admin)
            .await
            .unwrap();
        let pool = grant_fixture::pool_for(&schema).await;
        let installation_id = Uuid::new_v4();
        let store = FileRecipientGrantStore::new(
            pool.clone(),
            scope(),
            installation_id,
            &schema,
            max_grants,
        )
        .unwrap();
        store.install_empty_namespace().await.unwrap();
        let recipient_workspace = base
            .grants
            .auth
            .source
            .file_recipient_grant_workspace(&schema, scope(), installation_id, max_grants)
            .unwrap();
        let workspace = base
            .grants
            .auth
            .source
            .file_sharing_workspace(
                base.publication.clone(),
                base.grants.workspace.clone(),
                recipient_workspace.clone(),
            )
            .unwrap();
        Self {
            base,
            schema,
            pool,
            store,
            installation_id,
            recipient_workspace,
            workspace,
            recipient,
            expected_file,
            max_grants,
            auxiliary_pools: std::sync::Mutex::new(Vec::new()),
        }
    }
    pub fn actor(&self) -> FileAccountRef {
        self.base.actor()
    }
    pub fn command(&self, seed: u128) -> SessionFileRecipientGrantIssue {
        SessionFileRecipientGrantIssue {
            grant_id: recipient_grant(seed),
            publisher_grant_id: grant_fixture::grant(1),
            upload_id: publication_fixture::upload(1),
            expected_file: self.expected_file.clone(),
            recipient: self.recipient,
            validity: grant_fixture::validity(),
        }
    }
    pub async fn reopen(&self) -> (FileRecipientGrantStore, PgPool) {
        let pool = grant_fixture::pool_for(&self.schema).await;
        self.auxiliary_pools.lock().unwrap().push(pool.clone());
        let store = FileRecipientGrantStore::new(
            pool.clone(),
            scope(),
            self.installation_id,
            &self.schema,
            self.max_grants,
        )
        .unwrap();
        (store, pool)
    }
    pub async fn auxiliary_pool(&self, schema: &str) -> PgPool {
        let pool = grant_fixture::pool_for(schema).await;
        self.auxiliary_pools.lock().unwrap().push(pool.clone());
        pool
    }
    pub async fn snapshot(&self) -> Snapshot {
        let base = self.base.snapshot().await;
        let tables=query("SELECT relname AS n,oid::bigint AS oid FROM pg_class WHERE relnamespace=to_regnamespace($1) AND relkind='r' ORDER BY relname").bind(&self.schema).fetch_all(&self.base.grants.auth.base.admin).await.unwrap();
        let mut recipient = Vec::new();
        for t in tables {
            let name: String = t.get("n");
            assert!(name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
            let rows = query(&format!(
                "SELECT to_jsonb(t)::text AS v FROM {}.{name} t ORDER BY to_jsonb(t)::text",
                self.schema
            ))
            .fetch_all(&self.base.grants.auth.base.admin)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.get("v"))
            .collect();
            recipient.push((format!("{}.{name}", self.schema), t.get("oid"), rows));
        }
        Snapshot { base, recipient }
    }
    pub async fn cleanup(self) {
        self.pool.close().await;
        for pool in self.auxiliary_pools.into_inner().unwrap() {
            pool.close().await;
        }
        query(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .execute(&self.base.grants.auth.base.admin)
            .await
            .unwrap();
        self.base.cleanup().await;
    }
}
pub fn recipient_delta(
    schema: &str,
    before: &Snapshot,
    after: &Snapshot,
    seed: u128,
    revoked: bool,
) {
    assert_eq!(after.base,before.base,"sharing may not mutate source/Registry/publisher grant/upload/journal/Published rows, OIDs or staged files");
    assert_eq!(before.recipient.len(), after.recipient.len());
    for (name, oid, old) in &before.recipient {
        let (_, new_oid, new) = after.recipient.iter().find(|(n, _, _)| n == name).unwrap();
        assert_eq!(new_oid, oid);
        if name == &format!("{schema}.{HEADS}") {
            assert_eq!(new.len(), old.len() + usize::from(!revoked));
            for r in old {
                let v: serde_json::Value = serde_json::from_str(r).unwrap();
                if !revoked || v["grant_id"] != serde_json::json!(recipient_grant(seed)) {
                    assert!(new.contains(r));
                }
            }
            let (target, current) = recipient_head(after, schema, recipient_grant(seed))
                .expect("the exact target head must remain present");
            assert_eq!(current.id(), recipient_grant(seed));
            assert_eq!(current.revision(), revision(if revoked { 2 } else { 1 }));
            assert_eq!(
                current.state(),
                if revoked {
                    FileGrantState::Revoked
                } else {
                    FileGrantState::Active
                }
            );
            assert_eq!(
                target["revision"],
                serde_json::json!(if revoked { 2 } else { 1 })
            );
            let delta: Vec<_> = new.iter().filter(|r| !old.contains(r)).collect();
            assert_eq!(delta.len(), 1);
            let changed: serde_json::Value = serde_json::from_str(delta[0]).unwrap();
            assert_eq!(
                changed["grant_id"],
                serde_json::json!(recipient_grant(seed))
            );
            if revoked {
                let (mut expected, original) =
                    recipient_head(before, schema, recipient_grant(seed))
                        .expect("a revoke must replace the original exact target");
                assert_eq!(original.revision(), revision(1));
                assert_eq!(original.state(), FileGrantState::Active);
                let expected_grant = FileRecipientGrant::new(
                    original.id(),
                    revision(2),
                    original.file().clone(),
                    original.publisher(),
                    original.recipient(),
                    original.validity(),
                    FileGrantState::Revoked,
                )
                .unwrap();
                expected["revision"] = serde_json::json!(2);
                expected["grant_payload"] = serde_json::json!(canonical_bytea(&expected_grant));
                assert_eq!(target, expected, "only target revision/payload may change");
            } else {
                assert!(recipient_head(before, schema, recipient_grant(seed)).is_none());
            }
        } else if name == &format!("{schema}.{CHANGES}") {
            assert_eq!(new.len(), old.len() + 1);
            for r in old {
                assert!(new.contains(r));
            }
            let delta: Vec<_> = new.iter().filter(|r| !old.contains(r)).collect();
            assert_eq!(delta.len(), 1);
            let change: serde_json::Value = serde_json::from_str(delta[0]).unwrap();
            let grant = grant_payload(&change);
            assert_eq!(change["grant_id"], serde_json::json!(recipient_grant(seed)));
            assert_eq!(
                change["revision"],
                serde_json::json!(if revoked { 2 } else { 1 })
            );
            assert_eq!(grant.id(), recipient_grant(seed));
            assert_eq!(grant.revision(), revision(if revoked { 2 } else { 1 }));
            assert_eq!(
                grant.state(),
                if revoked {
                    FileGrantState::Revoked
                } else {
                    FileGrantState::Active
                }
            );
            let (_, head) = recipient_head(after, schema, recipient_grant(seed)).unwrap();
            assert_eq!(
                grant, head,
                "the new history must describe exactly the current target head"
            );
        } else {
            assert_eq!(name, &format!("{schema}.{MARKER}"));
            assert_eq!(new, old);
        }
    }
}
pub fn canonical_bytea(grant: &FileRecipientGrant) -> String {
    let hex = serde_json::to_vec(grant)
        .unwrap()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    format!("\\x{hex}")
}
fn grant_payload(row: &serde_json::Value) -> FileRecipientGrant {
    let raw = row["grant_payload"]
        .as_str()
        .expect("fixture bytea is hex text");
    let hex = raw.strip_prefix("\\x").unwrap();
    assert_eq!(hex.len() % 2, 0);
    let bytes = hex
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect::<Vec<_>>();
    let grant = FileRecipientGrant::parse_json(&bytes).unwrap();
    assert_eq!(
        bytes,
        serde_json::to_vec(&grant).unwrap(),
        "stored grant must be canonical, not merely valid JSON"
    );
    grant
}
pub fn recipient_head(
    snapshot: &Snapshot,
    schema: &str,
    id: FileGrantId,
) -> Option<(serde_json::Value, FileRecipientGrant)> {
    let (_, _, rows) = snapshot
        .recipient
        .iter()
        .find(|(name, _, _)| name == &format!("{schema}.{HEADS}"))
        .unwrap();
    let mut selected = rows
        .iter()
        .map(|raw| serde_json::from_str::<serde_json::Value>(raw).unwrap())
        .filter(|row| row["grant_id"] == serde_json::json!(id));
    let row = selected.next()?;
    assert!(selected.next().is_none());
    let grant = grant_payload(&row);
    Some((row, grant))
}
pub fn assert_record_rows(
    snapshot: &Snapshot,
    schema: &str,
    expected: &FileRecipientGrant,
    issuer: FileAccountRef,
    actor: FileAccountRef,
) {
    let (head, grant) =
        recipient_head(snapshot, schema, expected.id()).expect("expected exact grant head");
    assert_eq!(&grant, expected);
    let expected_head = serde_json::json!({"grant_id":expected.id(),"recipient_id":expected.recipient().principal().principal_id,"recipient_account_id":expected.recipient().account_id(),"publisher_id":expected.publisher().principal().principal_id,"publisher_account_id":expected.publisher().account_id(),"issuer_id":issuer.principal().principal_id,"issuer_account_id":issuer.account_id(),"revision":u64::from(expected.revision()),"not_before":expected.validity().not_before(),"expires_at":expected.validity().expires_at(),"grant_payload":canonical_bytea(expected)});
    assert_eq!(head, expected_head);
    let (_, _, rows) = snapshot
        .recipient
        .iter()
        .find(|(name, _, _)| name == &format!("{schema}.{CHANGES}"))
        .unwrap();
    let mut selected = rows
        .iter()
        .map(|raw| serde_json::from_str::<serde_json::Value>(raw).unwrap())
        .filter(|row| {
            row["grant_id"] == serde_json::json!(expected.id())
                && row["revision"] == serde_json::json!(u64::from(expected.revision()))
        });
    let change = selected.next().expect("exact expected history revision");
    assert!(selected.next().is_none());
    assert_eq!(grant_payload(&change), *expected);
    assert_eq!(
        change["actor_id"],
        serde_json::json!(actor.principal().principal_id)
    );
    assert_eq!(
        change["actor_account_id"],
        serde_json::json!(actor.account_id())
    );
    assert_eq!(
        change["not_before"],
        serde_json::json!(expected.validity().not_before())
    );
    assert_eq!(
        change["expires_at"],
        serde_json::json!(expected.validity().expires_at())
    );
    assert!((0..=MAX_FILE_GRANT_UNIX_SECONDS).contains(&change["recorded_at"].as_u64().unwrap()));
}
pub fn publisher_revocation_delta(before: &Snapshot, after: &Snapshot) {
    assert_eq!(after.recipient, before.recipient);
    assert_eq!(after.base.root, before.base.root);
    assert_eq!(after.base.files, before.base.files);
    assert_eq!(after.base.database.0.len(), before.base.database.0.len());
    for (name, oid, old) in &before.base.database.0 {
        let (_, new_oid, new) = after
            .base
            .database
            .0
            .iter()
            .find(|(n, _, _)| n == name)
            .unwrap();
        assert_eq!(new_oid, oid);
        if name.ends_with(&format!(".{}", grant_fixture::CHANGES)) {
            assert_eq!(new.len(), old.len() + 1);
            for r in old {
                assert!(new.contains(r));
            }
        } else if name.ends_with(&format!(".{}", grant_fixture::HEADS)) {
            assert_eq!(new.len(), old.len());
            for r in old {
                let v: serde_json::Value = serde_json::from_str(r).unwrap();
                if v["grant_id"] != serde_json::json!(grant_fixture::grant(1)) {
                    assert!(new.contains(r));
                }
            }
        } else {
            assert_eq!(
                new, old,
                "publisher revocation does not mutate any other metadata or object identity"
            );
        }
    }
}
