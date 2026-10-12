use super::*;

pub const MARKER: &str = "collaboration_file_publisher_grant_schema";
pub const HEADS: &str = "collaboration_file_publisher_grants";
pub const CHANGES: &str = "collaboration_file_publisher_grant_changes";

pub fn grant(seed: u128) -> FileGrantId {
    Uuid::from_u128(seed + 300).try_into().unwrap()
}
pub fn validity() -> FileGrantValidity {
    FileGrantValidity::new(0, MAX_FILE_GRANT_UNIX_SECONDS).unwrap()
}
pub async fn pool_for(schema: &str) -> PgPool {
    let selected = schema.to_owned();
    PgPoolOptions::new().max_connections(4).after_connect(move |connection, _| {
        let selected = selected.clone();
        Box::pin(async move {
            query("SELECT set_config('search_path',$1,false),set_config('application_name',$1,false)")
                .bind(selected).execute(connection).await?;
            Ok(())
        })
    }).connect(&std::env::var("CYANREX_TEST_DATABASE_URL").expect("explicit disposable database")).await.unwrap()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot(pub Vec<(String, i64, Vec<String>)>);

pub struct Fixture {
    pub auth: identity_fixture::Fixture,
    pub store: FilePublisherGrantStore,
    pub workspace: SessionFilePublisherGrantWorkspace,
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
        let schema = format!("session_publisher_grant_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {schema}"))
            .execute(&auth.base.admin)
            .await
            .unwrap();
        let pool = pool_for(&schema).await;
        let installation_id = Uuid::new_v4();
        let store =
            FilePublisherGrantStore::new(pool.clone(), scope(), installation_id, &schema, 16)
                .unwrap();
        store.install_empty_namespace().await.unwrap();
        let workspace = auth
            .source
            .file_publisher_grant_workspace(&schema, scope(), installation_id, 16)
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
    pub fn subject(&self) -> FileAccountRef {
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
    pub async fn issue(&self, seed: u128, subject: FileAccountRef) -> StoredFilePublisherGrant {
        self.store
            .issue(grant(seed), self.issuer(), subject, validity())
            .await
            .unwrap()
    }
    pub async fn snapshot(&self) -> Snapshot {
        let mut result = Vec::new();
        for schema in [&self.auth.base.schema, &self.schema] {
            let tables = query("SELECT c.relname,c.oid::bigint AS oid FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=$1 AND c.relkind='r' ORDER BY c.relname")
                .bind(schema).fetch_all(&self.auth.base.admin).await.unwrap();
            for table in tables {
                let name: String = table.get("relname");
                assert!(name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
                let rows = query(&format!("SELECT to_jsonb(t)::text AS row FROM {schema}.{name} t ORDER BY to_jsonb(t)::text"))
                    .fetch_all(&self.auth.base.admin).await.unwrap().into_iter().map(|r| r.get("row")).collect();
                result.push((format!("{schema}.{name}"), table.get("oid"), rows));
            }
        }
        Snapshot(result)
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
