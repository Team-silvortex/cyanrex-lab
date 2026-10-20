use super::index_fixture;
pub(super) use super::index_fixture::{
    invalid_session, snapshot, task_id, token_hash, unavailable, within, Fixture,
};
use cyanrex_engine::{
    models::collaboration::TaskId,
    platform_host::PlatformTaskCreateConnector,
    services::{auth_service::durable_source::SessionTaskError, task_store::TaskContentSnapshot},
};
use sqlx_core::{query::query, row::Row};

pub(super) struct Prepared<'a> {
    base: index_fixture::Prepared<'a>,
}
impl<'a> std::ops::Deref for Prepared<'a> {
    type Target = index_fixture::Prepared<'a>;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}
impl<'a> Prepared<'a> {
    pub(super) async fn new(f: &'a Fixture) -> Self {
        let base = index_fixture::Prepared::new(f).await;
        // Explicit fixture administration only: no product privilege repair or broad UPDATE.
        for grant in [
            format!("INSERT ON {}.collaboration_tasks", f.task),
            format!("INSERT ON {}.collaboration_task_outbox", f.task),
            format!(
                "UPDATE(singleton) ON {}.collaboration_artifact_schema",
                f.artifacts
            ),
        ] {
            f.sql(&format!("GRANT {grant} TO {}", f.role)).await;
        }
        Self { base }
    }
    pub(super) async fn connector(&self) -> PlatformTaskCreateConnector {
        PlatformTaskCreateConnector::new(self.storage().await)
    }
    pub(super) async fn enable_reopen(&self) {
        // Explicitly separate read capability for flow fixtures, never required by creation.
        for grant in [
            "SELECT,UPDATE(artifact_id) ON collaboration_artifacts",
            "SELECT ON collaboration_artifact_revisions",
            "SELECT ON collaboration_artifact_outbox",
        ] {
            self.f
                .sql(&format!(
                    "GRANT {} TO {}",
                    grant.replace(" ON ", &format!(" ON {}.", self.f.artifacts)),
                    self.f.role
                ))
                .await;
        }
    }
    pub(super) async fn create(
        &self,
        connector: &PlatformTaskCreateConnector,
        owner: usize,
        id: TaskId,
        title: &str,
    ) -> Result<TaskContentSnapshot, SessionTaskError> {
        connector
            .create_session_empty_content_task(&self.tokens[owner], id, title)
            .await
    }
    pub(super) async fn count(&self, table: &str) -> i64 {
        within(
            query(&format!(
                "SELECT count(*) AS count FROM {}.{table}",
                self.f.task
            ))
            .fetch_one(self.f.owner()),
        )
        .await
        .unwrap()
        .get("count")
    }
    pub(super) async fn created(&self, expected: &TaskContentSnapshot, count: i64) {
        assert_eq!(self.count("collaboration_tasks").await, count);
        assert_eq!(self.count("collaboration_task_outbox").await, count);
        // A trusted positive control must not add the separate retained-reader business SELECT
        // profile to this creation fixture (especially the narrow-permission case).
        use cyanrex_engine::services::auth_service::durable_source::{
            DurableAuthSource, SessionTaskContentWorkspace,
        };
        let source =
            DurableAuthSource::new(self.f.owner().clone(), self.f.config().scope().authority_id);
        let artifacts = source
            .open_artifact_workspace(&self.f.artifacts, self.f.config().scope(), self.root.path())
            .unwrap();
        let workspace =
            SessionTaskContentWorkspace::new(&self.f.task, self.f.config().scope(), artifacts)
                .unwrap();
        let actual = source
            .get_session_content_task(&self.tokens[0], &workspace, expected.task.reference.task_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(actual.snapshot, *expected);
        assert!(actual.contents.is_empty());
        assert_eq!(
            std::fs::read_dir(self.root.path()).unwrap().count(),
            1,
            "only retained root marker exists"
        );
    }
    pub(super) fn finish(self) -> bool {
        self.base.finish()
    }
}
