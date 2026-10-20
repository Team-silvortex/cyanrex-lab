pub(super) use super::content_fixture::{
    invalid_session, manifest, snapshot, task_id, token_hash, unavailable, within, Fixture,
};
use super::{content_fixture, schema};
use cyanrex_engine::{
    models::collaboration::*,
    platform_host::PlatformTaskReplaceConnector,
    services::{
        auth_service::durable_source::SessionTaskError,
        task_store::{TaskContentSnapshot, TaskContentStore},
    },
};
use sqlx_core::{query::query, row::Row};
use std::{fs, os::unix::fs::MetadataExt};

pub(super) type Files = Vec<(String, u64, u64, u32, u64, Vec<u8>)>;
pub(super) struct Prepared<'a> {
    base: content_fixture::Prepared<'a>,
}
impl<'a> std::ops::Deref for Prepared<'a> {
    type Target = content_fixture::Prepared<'a>;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}
impl<'a> Prepared<'a> {
    pub(super) async fn new(f: &'a Fixture) -> Self {
        let base = content_fixture::Prepared::new(f).await;
        // Explicit disposable administration only. No Task INSERT or Artifact business writes.
        for grant in [
            "UPDATE(revision,snapshot,manifest) ON collaboration_tasks",
            "INSERT ON collaboration_task_outbox",
        ] {
            f.sql(&format!(
                "GRANT {} TO {}",
                grant.replace(" ON ", &format!(" ON {}.", f.task)),
                f.role
            ))
            .await;
        }
        Self { base }
    }
    pub(super) async fn connector(&self) -> PlatformTaskReplaceConnector {
        PlatformTaskReplaceConnector::new(self.storage().await)
    }
    pub(super) async fn replace(
        &self,
        c: &PlatformTaskReplaceConnector,
        owner: usize,
        old: &TaskContentSnapshot,
        manifest: TaskContentManifest,
    ) -> Result<TaskContentSnapshot, SessionTaskError> {
        c.replace_session_content_task(
            &self.tokens[owner],
            old.task.reference.task_id,
            old.task.revision,
            manifest,
        )
        .await
    }
    pub(super) fn files(&self) -> Files {
        let mut rows = fs::read_dir(self.root.path())
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                let metadata = entry.metadata().unwrap();
                (
                    entry.file_name().into_string().unwrap(),
                    metadata.dev(),
                    metadata.ino(),
                    metadata.mode(),
                    metadata.nlink(),
                    fs::read(entry.path()).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        rows
    }
    pub(super) async fn immutable(&self) -> Vec<(String, String)> {
        snapshot(self.f)
            .await
            .into_iter()
            .filter(|(table, _)| {
                ![
                    format!("{}.collaboration_tasks", self.f.task),
                    format!("{}.collaboration_task_outbox", self.f.task),
                ]
                .contains(table)
            })
            .collect()
    }
    pub(super) async fn unchanged(&self, expected: &TaskContentSnapshot, events: i64) {
        schema::path(self.f, &self.f.task).await;
        let actual = TaskContentStore::new(self.f.owner().clone(), self.f.config().scope())
            .get(expected.task.reference, expected.task.owner)
            .await
            .unwrap();
        schema::path(self.f, &self.f.source).await;
        assert_eq!(actual, Some(expected.clone()));
        assert_eq!(self.count("collaboration_task_outbox").await, events);
    }
    pub(super) async fn count(&self, table: &str) -> i64 {
        assert!(["collaboration_tasks", "collaboration_task_outbox"].contains(&table));
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
    pub(super) async fn reopened(&self, expected: &TaskContentSnapshot) {
        let reader =
            cyanrex_engine::platform_host::PlatformTaskContentConnector::new(self.storage().await);
        let read = reader
            .get_session_content_task(&self.tokens[0], expected.task.reference.task_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(read.snapshot, *expected);
        assert_eq!(read.contents.len(), expected.manifest.payload().len());
    }
    pub(super) fn finish(self) -> bool {
        self.base.finish()
    }
}
