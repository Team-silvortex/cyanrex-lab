pub(super) use super::catalog_fixture::{within, Fixture};
use super::{
    schema,
    storage_helpers::{self, Mode, Root},
};
use cyanrex_engine::{
    models::collaboration::*,
    platform_host::{PlatformStorageConnector, PlatformTaskIndexConnector},
    services::{
        auth_service::durable_source::{DurableAuthError, SessionCommandError, SessionTaskError},
        collaboration_identity_store::{CollaborationIdentityStore, LegacyPolicyCommand},
        task_store::{
            TaskContentIndexEntry, TaskContentIndexPage, TaskContentSnapshot, TaskContentStore,
        },
    },
};
use sha2::{Digest, Sha256};
use sqlx_core::{query::query, row::Row};

pub(super) fn token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
pub(super) fn task_id(n: u128) -> TaskId {
    uuid::Uuid::from_u128(8400 + n).try_into().unwrap()
}
pub(super) fn unavailable() -> SessionTaskError {
    DurableAuthError::StorageUnavailable.into()
}
pub(super) fn invalid_session() -> SessionTaskError {
    SessionCommandError::InvalidSession.into()
}
pub(super) fn entry(task: &TaskContentSnapshot) -> TaskContentIndexEntry {
    TaskContentIndexEntry {
        reference: task.task.reference,
        revision: task.task.revision,
        title: task.task.title.clone(),
        status: task.task.status,
        definition_ref: None,
        updated_at: task.task.updated_at,
        payload_items: task.manifest.payload().len(),
    }
}

pub(super) struct Prepared<'a> {
    pub(super) f: &'a Fixture,
    pub(super) root: Root,
    pub(super) tokens: [String; 3],
    pub(super) owners: [PrincipalRef; 2],
}
impl<'a> Prepared<'a> {
    pub(super) async fn new(f: &'a Fixture) -> Self {
        let tokens = std::array::from_fn(|_| uuid::Uuid::new_v4().to_string());
        let owners = schema::prepare(f, &tokens).await;
        Self {
            f,
            root: Root::new(),
            tokens,
            owners,
        }
    }
    pub(super) async fn storage(&self) -> PlatformStorageConnector {
        storage_helpers::binding(self.f, &self.root, Mode::Expected)
            .await
            .unwrap()
    }
    pub(super) async fn connector(&self) -> PlatformTaskIndexConnector {
        PlatformTaskIndexConnector::new(self.storage().await)
    }
    pub(super) async fn task(
        &self,
        number: u128,
        owner: usize,
        title: &str,
    ) -> TaskContentSnapshot {
        self.task_manifest(
            number,
            owner,
            TaskContentManifest::new(title, vec![]).unwrap(),
        )
        .await
    }
    pub(super) async fn task_manifest(
        &self,
        number: u128,
        owner: usize,
        manifest: TaskContentManifest,
    ) -> TaskContentSnapshot {
        schema::path(self.f, &self.f.task).await;
        let result = TaskContentStore::new(self.f.owner().clone(), self.f.config().scope())
            .create(task_id(number), self.owners[owner], manifest)
            .await
            .unwrap();
        schema::path(self.f, &self.f.source).await;
        result
    }
    pub(super) async fn suspend(&self, owner: usize) {
        schema::path(self.f, &self.f.source).await;
        let registry = CollaborationIdentityStore::new(self.f.owner().clone());
        let current = registry
            .legacy_access(self.f.config().scope(), self.owners[owner].principal_id)
            .await
            .unwrap()
            .unwrap();
        let mut desired = current.policy;
        desired.membership.status = MembershipStatus::Suspended;
        registry
            .apply_legacy_policy_command(&LegacyPolicyCommand {
                command_id: uuid::Uuid::new_v4().try_into().unwrap(),
                actor: self.owners[0],
                expected_revision: Some(current.revision),
                desired,
            })
            .await
            .unwrap();
    }
    pub(super) async fn page(
        &self,
        connector: &PlatformTaskIndexConnector,
        owner: usize,
        after: Option<TaskRef>,
        limit: Option<usize>,
    ) -> Result<TaskContentIndexPage, SessionTaskError> {
        let before = snapshot(self.f).await;
        let catalog = self.f.snapshot().await;
        let result = connector
            .list_session_content_tasks(&self.tokens[owner], after, limit)
            .await;
        assert!(
            before == snapshot(self.f).await,
            "index changed source/resource rows"
        );
        assert!(
            catalog == self.f.snapshot().await,
            "index changed original catalog metadata"
        );
        result
    }
    pub(super) fn finish(self) -> bool {
        self.root.finish()
    }
}

pub(super) async fn snapshot(f: &Fixture) -> Vec<(String, String)> {
    let rows = within(
        query(
            "SELECT n.nspname::text AS schema,c.relname::text AS name
        FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
        WHERE n.nspname=ANY($1) AND c.relkind='r' ORDER BY n.nspname,c.relname",
        )
        .bind(vec![f.source.clone(), f.task.clone(), f.artifacts.clone()])
        .fetch_all(f.owner()),
    )
    .await
    .unwrap();
    let mut state = Vec::new();
    for row in rows {
        let table = format!(
            "{}.{}",
            row.get::<String, _>("schema"),
            row.get::<String, _>("name")
        );
        let digest = within(query(&format!("SELECT md5(coalesce(string_agg(to_jsonb(t)::text,'|' ORDER BY to_jsonb(t)::text),'')) AS digest FROM {table} t"))
            .fetch_one(f.owner())).await.unwrap().get("digest");
        state.push((table, digest));
    }
    state
}
