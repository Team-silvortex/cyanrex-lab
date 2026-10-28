pub(super) use super::content_fixture::{manifest, task_id};
pub(super) use super::retained_fixture::{
    artifact_id, revision_id, snapshot, token_hash, within, Fixture,
};
use super::{retained_fixture, schema};
use cyanrex_engine::{
    models::collaboration::*,
    platform_host::PlatformTaskTextArtifactConnector,
    services::{
        auth_service::durable_source::{DurableAuthError, SessionCommandError, SessionTaskError},
        task_store::{TaskContentSnapshot, TaskContentStore},
    },
};
use sqlx_core::{query::query, row::Row};

pub(super) fn unavailable() -> SessionTaskError {
    DurableAuthError::StorageUnavailable.into()
}
pub(super) fn invalid_session() -> SessionTaskError {
    SessionCommandError::InvalidSession.into()
}
pub(super) struct Prepared<'a> {
    base: retained_fixture::Prepared<'a>,
}
impl<'a> std::ops::Deref for Prepared<'a> {
    type Target = retained_fixture::Prepared<'a>;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}
impl<'a> Prepared<'a> {
    pub(super) async fn new(f: &'a Fixture) -> Self {
        let base = retained_fixture::Prepared::new(f).await;
        // Only explicit disposable administration. The existing retained fixture removed this
        // lock-only grant; restore it without Task business UPDATE or outbox INSERT/UPDATE.
        f.sql(&format!(
            "GRANT UPDATE(task_id) ON {}.collaboration_tasks TO {}",
            f.task, f.role
        ))
        .await;
        Self { base }
    }
    pub(super) async fn connector(&self) -> PlatformTaskTextArtifactConnector {
        PlatformTaskTextArtifactConnector::new(self.storage().await)
    }
    pub(super) async fn create(
        &self,
        connector: &PlatformTaskTextArtifactConnector,
        owner: usize,
        expected: &TaskContentSnapshot,
        n: u128,
        text: &str,
    ) -> Result<ArtifactRevision, SessionTaskError> {
        connector
            .create_session_task_text_artifact(
                &self.tokens[owner],
                expected.clone(),
                artifact_id(n),
                revision_id(n),
                text.to_owned(),
            )
            .await
    }
    pub(super) async fn reject(
        &self,
        connector: &PlatformTaskTextArtifactConnector,
        owner: usize,
        expected: &TaskContentSnapshot,
        n: u128,
        error: SessionTaskError,
    ) {
        let before = snapshot(self.f).await;
        let catalog = self.f.snapshot().await;
        let files = self.files();
        assert_eq!(
            self.create(connector, owner, expected, n, "Rejected before new bytes")
                .await,
            Err(error)
        );
        assert_eq!(before, snapshot(self.f).await);
        assert!(catalog == self.f.snapshot().await);
        assert_eq!(
            files,
            self.files(),
            "pre-publication rejection may not create any file"
        );
    }
    pub(super) async fn unchanged(&self, expected: &TaskContentSnapshot) {
        schema::path(self.f, &self.f.task).await;
        let actual = TaskContentStore::new(self.f.owner().clone(), self.f.config().scope())
            .get(expected.task.reference, expected.task.owner)
            .await
            .unwrap();
        schema::path(self.f, &self.f.source).await;
        assert_eq!(actual, Some(expected.clone()));
    }
    pub(super) async fn ready(&self, old: &TaskContentSnapshot) -> TaskContentSnapshot {
        schema::path(self.f, &self.f.task).await;
        let actual = TaskContentStore::new(self.f.owner().clone(), self.f.config().scope())
            .transition(
                old.task.reference,
                old.task.owner,
                old.task.revision,
                TaskStatus::Ready,
            )
            .await
            .unwrap();
        schema::path(self.f, &self.f.source).await;
        actual
    }
    pub(super) async fn acl(&self) -> String {
        within(query("SELECT md5(coalesce(string_agg(c.oid::text||':'||coalesce(c.relacl::text,'')||':'||(SELECT coalesce(string_agg(a.attnum::text||':'||coalesce(a.attacl::text,''),'|' ORDER BY a.attnum),'') FROM pg_catalog.pg_attribute a WHERE a.attrelid=c.oid AND a.attnum>0),'|' ORDER BY c.oid),'')) AS acl FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=ANY($1)")
            .bind(vec![self.f.source.clone(),self.f.task.clone(),self.f.artifacts.clone()]).fetch_one(self.f.owner())).await.unwrap().get("acl")
    }
    pub(super) fn finish(self) -> bool {
        self.base.finish()
    }
}
