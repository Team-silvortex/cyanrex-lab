use super::*;

pub fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: authority(),
        workspace_id: "96699048-1d67-4ffd-b35f-ce22e287a4c7".parse().unwrap(),
    }
}
pub fn policy(principal_id: PrincipalId, grant: bool) -> LegacyAccessPolicy {
    LegacyAccessPolicy {
        membership: Membership {
            workspace: scope(),
            principal_id,
            role_refs: vec!["cyanrex.teaching.teacher".parse().unwrap()],
            status: MembershipStatus::Active,
        },
        deployment_granted: grant,
    }
}
pub fn policy_command(principal_id: PrincipalId, grant: bool) -> SessionPolicyCommand {
    SessionPolicyCommand {
        command_id: id(),
        expected_revision: None,
        desired: policy(principal_id, grant),
    }
}
pub struct Fixture {
    pub base: source_fixture::Fixture,
    pub source: DurableAuthSource,
    pub store: CollaborationIdentityStore,
    pub manager: StoredLegacyIdentity,
    pub learner: StoredLegacyIdentity,
    pub target: DurableAccountRef,
    pub manager_token: String,
    pub learner_token: String,
    pub target_token: String,
}
impl Fixture {
    pub async fn seeded() -> Self {
        let base = source_fixture::Fixture::ready().await;
        let source = base.source.clone();
        let store = CollaborationIdentityStore::new(base.pool.clone());
        store.install_schema().await.unwrap();
        store.provision_legacy_workspace(scope()).await.unwrap();
        store.install_access_schema().await.unwrap();
        let mut identities = Vec::new();
        let mut accounts = Vec::new();
        let mut tokens = Vec::new();
        for name in ["source-manager", "source-learner", "source-target"] {
            let created = source
                .register(&name.parse().unwrap(), PASSWORD)
                .await
                .unwrap();
            tokens.push(
                source
                    .login(
                        &created.account.username,
                        PASSWORD,
                        &otp(&created.bootstrap.secret),
                    )
                    .await
                    .unwrap()
                    .token,
            );
            if name != "source-target" {
                let identity = store
                    .bind_legacy_account(
                        scope(),
                        &created.account.username,
                        created.account.account_id,
                    )
                    .await
                    .unwrap();
                store
                    .replace_legacy_access(
                        None,
                        &policy(identity.binding.principal_id, name == "source-manager"),
                    )
                    .await
                    .unwrap();
                identities.push(identity);
            }
            accounts.push(created.account);
        }
        Self {
            base,
            source,
            store,
            manager: identities.remove(0),
            learner: identities.remove(0),
            target: accounts.remove(2),
            manager_token: tokens.remove(0),
            learner_token: tokens.remove(0),
            target_token: tokens.remove(0),
        }
    }
    pub async fn ready() -> Self {
        let f = Self::seeded().await;
        f.store.upgrade_policy_audit_schema().await.unwrap();
        f.store.upgrade_identity_audit_schema().await.unwrap();
        f
    }
    pub fn bind_command(&self) -> SessionBindCommand {
        SessionBindCommand {
            command_id: id(),
            workspace: scope(),
            target: self.target.clone(),
        }
    }
    pub async fn bind_target(&self) -> StoredLegacyIdentity {
        self.source
            .bind_session_account(&self.manager_token, &self.bind_command())
            .await
            .unwrap()
            .entry
            .after
    }
    pub async fn current(&self, principal: PrincipalId) -> StoredLegacyAccessPolicy {
        self.store
            .legacy_access(scope(), principal)
            .await
            .unwrap()
            .unwrap()
    }
    pub async fn count(&self, table: &str) -> i64 {
        assert!(matches!(
            table,
            "collaboration_identity_audit"
                | "collaboration_policy_audit"
                | "collaboration_principals"
        ));
        query(&format!("SELECT count(*) AS count FROM {table}"))
            .fetch_one(&self.base.pool)
            .await
            .unwrap()
            .get("count")
    }
    pub async fn sql(&self, statement: &str) {
        self.base.sql(statement).await;
    }
    pub async fn cleanup(self) {
        self.base.cleanup().await;
    }
}
