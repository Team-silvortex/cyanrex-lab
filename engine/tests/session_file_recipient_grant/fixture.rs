pub use super::ledger_fixture::{grant, revision, validity, CHANGES, HEADS, MARKER};
use super::*;
#[derive(Debug, PartialEq, Eq)]
pub struct Snapshot(pub Vec<(String, i64, Vec<String>)>);
pub struct Fixture {
    pub auth: identity_fixture::Fixture,
    pub store: FileRecipientGrantStore,
    pub workspace: SessionFileRecipientGrantWorkspace,
    pub schema: String,
    pub pool: PgPool,
    pub installation_id: Uuid,
}
impl Fixture {
    pub async fn ready() -> Self {
        let auth = identity_fixture::Fixture::ready().await;
        let current = auth.current(auth.learner.binding.principal_id).await;
        let mut desired = current.policy;
        desired.membership.role_refs.clear();
        auth.source
            .apply_session_policy_command(
                &auth.manager_token,
                &SessionPolicyCommand {
                    command_id: id(),
                    expected_revision: Some(current.revision),
                    desired,
                },
            )
            .await
            .unwrap();
        let schema = format!("session_recipient_grant_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {schema}"))
            .execute(&auth.base.admin)
            .await
            .unwrap();
        let pool = ledger_fixture::pool_for(
            &std::env::var("CYANREX_TEST_DATABASE_URL").expect("explicit disposable database"),
            &schema,
        )
        .await;
        let installation_id = Uuid::new_v4();
        let store =
            FileRecipientGrantStore::new(pool.clone(), scope(), installation_id, &schema, 16)
                .unwrap();
        store.install_empty_namespace().await.unwrap();
        let workspace = auth
            .source
            .file_recipient_grant_workspace(&schema, scope(), installation_id, 16)
            .unwrap();
        Self {
            auth,
            store,
            workspace,
            schema,
            pool,
            installation_id,
        }
    }
    pub fn recipient(&self) -> FileAccountRef {
        FileAccountRef::new(
            self.auth.learner.principal.reference,
            self.auth.learner.account_id,
        )
    }
    pub fn issuer(&self) -> FileAccountRef {
        FileAccountRef::new(
            self.auth.manager.principal.reference,
            self.auth.manager.account_id,
        )
    }
    pub fn publisher(&self) -> FileAccountRef {
        FileAccountRef::new(
            PrincipalRef {
                authority_id: scope().authority_id,
                principal_id: Uuid::from_u128(711).try_into().unwrap(),
            },
            Uuid::from_u128(811).try_into().unwrap(),
        )
    }
    pub async fn issue(&self, seed: u128, recipient: FileAccountRef) -> StoredFileRecipientGrant {
        self.store
            .issue(
                grant(seed),
                self.issuer(),
                ledger_fixture::file_for(scope(), seed),
                self.publisher(),
                recipient,
                validity(),
            )
            .await
            .unwrap()
    }
    pub async fn snapshot(&self) -> Snapshot {
        let mut values = Vec::new();
        for schema in [&self.auth.base.schema, &self.schema] {
            let tables=query("SELECT relname AS n,oid::bigint AS oid FROM pg_class WHERE relnamespace=to_regnamespace($1) AND relkind='r' ORDER BY relname").bind(schema).fetch_all(&self.auth.base.admin).await.unwrap();
            for t in tables {
                let name: String = t.get("n");
                assert!(name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
                let rows=query(&format!("SELECT to_jsonb(t)::text AS v FROM {schema}.{name} t ORDER BY to_jsonb(t)::text")).fetch_all(&self.auth.base.admin).await.unwrap().into_iter().map(|r|r.get("v")).collect();
                values.push((format!("{schema}.{name}"), t.get("oid"), rows));
            }
        }
        Snapshot(values)
    }
    pub async fn cleanup(self) {
        self.pool.close().await;
        query(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .execute(&self.auth.base.admin)
            .await
            .unwrap();
        self.auth.cleanup().await;
    }
}
