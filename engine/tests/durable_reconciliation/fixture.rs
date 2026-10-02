use super::*;
use sqlx_core::transaction::Transaction;

pub fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: authority(),
        workspace_id: "60a9845d-0687-4f19-a4f5-2ebf64bed4ba".parse().unwrap(),
    }
}
pub async fn guard() -> Transaction<'static, sqlx_postgres::Postgres> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&std::env::var("CYANREX_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let mut guard = pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext(current_database()), hashtext('cyanrex.test.bootstrap.event-hooks'))")
        .execute(&mut *guard).await.unwrap();
    guard
}
pub struct CheckFixture {
    pub db: Fixture,
    pub owner: AuthorityBootstrap,
    _guard: Transaction<'static, sqlx_postgres::Postgres>,
}
impl CheckFixture {
    pub async fn new() -> Self {
        let guard = guard().await;
        let db = Fixture::new().await;
        let owner = db
            .source
            .bootstrap_empty_authority(scope(), &username(), PASSWORD)
            .await
            .unwrap();
        Self {
            db,
            owner,
            _guard: guard,
        }
    }
    pub async fn owner_login(&self) -> String {
        self.db
            .source
            .login(
                &username(),
                PASSWORD,
                &otp(&self.owner.registration.bootstrap.secret),
            )
            .await
            .unwrap()
            .token
    }
    pub async fn bound(&self, name: &str) -> (DurableRegistration, StoredLegacyIdentity) {
        let created = self
            .db
            .source
            .register(&name.parse().unwrap(), PASSWORD)
            .await
            .unwrap();
        let token = self.owner_login().await;
        let identity = self
            .db
            .source
            .bind_session_account(
                &token,
                &SessionBindCommand {
                    command_id: id(),
                    workspace: scope(),
                    target: created.account.clone(),
                },
            )
            .await
            .unwrap()
            .entry
            .after;
        (created, identity)
    }
    pub async fn policy(&self, principal: PrincipalId, grant: bool) -> StoredLegacyAccessPolicy {
        let registry = CollaborationIdentityStore::new(self.db.pool.clone());
        let prior = registry.legacy_access(scope(), principal).await.unwrap();
        let token = self.owner_login().await;
        self.db
            .source
            .apply_session_policy_command(
                &token,
                &SessionPolicyCommand {
                    command_id: id(),
                    expected_revision: prior.map(|p| p.revision),
                    desired: LegacyAccessPolicy {
                        membership: Membership {
                            workspace: scope(),
                            principal_id: principal,
                            role_refs: vec!["cyanrex.teaching.learner".parse().unwrap()],
                            status: MembershipStatus::Suspended,
                        },
                        deployment_granted: grant,
                    },
                },
            )
            .await
            .unwrap()
            .entry
            .after
    }
    pub async fn reconcile(&self) -> Result<AuthorityReconciliation, ReconciliationError> {
        self.db.source.reconcile_authority(scope()).await
    }
    pub async fn digest(&self) -> String {
        // Includes credentials only inside the disposable DB; no row contents leave this helper.
        let mut chunks = Vec::new();
        for table in [
            "users",
            "sessions",
            "collaboration_auth_source_schema",
            "collaboration_identity_schema",
            "collaboration_access_schema",
            "collaboration_authorities",
            "collaboration_workspaces",
            "collaboration_legacy_workspaces",
            "collaboration_principals",
            "collaboration_legacy_identities",
            "collaboration_memberships",
            "collaboration_deployment_grants",
            "collaboration_identity_audit",
            "collaboration_policy_audit",
        ] {
            let digest: String = query(&format!("SELECT md5(COALESCE(string_agg(row::text, '' ORDER BY row::text), '')) AS digest FROM (SELECT to_jsonb(t) AS row FROM {table} t) s"))
                .fetch_one(&self.db.pool).await.unwrap().get("digest");
            chunks.push(digest);
        }
        chunks.join(":")
    }
    pub async fn cleanup(self) {
        self.db.cleanup().await;
    }
}
