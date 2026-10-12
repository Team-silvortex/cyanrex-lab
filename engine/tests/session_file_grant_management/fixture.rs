use super::*;

pub struct Fixture {
    pub base: publisher_fixture::Fixture,
    pub management: SessionFileGrantManagementWorkspace,
}
impl Fixture {
    pub async fn ready() -> Self {
        let base = publisher_fixture::Fixture::ready().await;
        // The old fixture audited learner policy down to role-free, active membership.
        // It is deliberately NOT the teacher/deployment manager from that fixture.
        let management = base
            .auth
            .source
            .file_grant_management_workspace(base.workspace.clone(), base.subject())
            .unwrap();
        Self { base, management }
    }
    pub async fn with_max(max: u32) -> Self {
        let mut base = publisher_fixture::Fixture::ready().await;
        base.pool.close().await;
        query(&format!("DROP SCHEMA {} CASCADE", base.schema))
            .execute(&base.auth.base.admin)
            .await
            .unwrap();
        base.schema = format!("session_file_grant_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {}", base.schema))
            .execute(&base.auth.base.admin)
            .await
            .unwrap();
        base.pool = publisher_fixture::pool_for(&base.schema).await;
        base.installation_id = Uuid::new_v4();
        base.store = FilePublisherGrantStore::new(
            base.pool.clone(),
            scope(),
            base.installation_id,
            &base.schema,
            max,
        )
        .unwrap();
        base.store.install_empty_namespace().await.unwrap();
        base.workspace = base
            .auth
            .source
            .file_publisher_grant_workspace(&base.schema, scope(), base.installation_id, max)
            .unwrap();
        let management = base
            .auth
            .source
            .file_grant_management_workspace(base.workspace.clone(), base.subject())
            .unwrap();
        Self { base, management }
    }
    pub fn manager(&self) -> FileAccountRef {
        self.base.subject()
    }
    pub fn target(&self) -> FileAccountRef {
        self.base.issuer()
    }
    pub async fn cleanup(self) {
        self.base.cleanup().await;
    }
}
