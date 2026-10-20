pub(super) use super::index_fixture::{
    invalid_session, snapshot, task_id, token_hash, unavailable, within, Fixture,
};
use super::{index_fixture, schema};
use cyanrex_engine::{
    models::collaboration::*,
    platform_host::PlatformTaskContentConnector,
    services::{
        artifact_store::{ArtifactDraft, ArtifactStore},
        auth_service::durable_source::{
            DurableAuthSource, SessionTaskContent, SessionTaskContentWorkspace, SessionTaskError,
        },
        task_store::TaskContentSnapshot,
    },
};
use std::{fs, os::unix::fs::MetadataExt, path::PathBuf};

pub(super) fn manifest(title: &str, refs: Vec<ArtifactRef>) -> TaskContentManifest {
    TaskContentManifest::new(
        title,
        refs.into_iter()
            .enumerate()
            .map(|(index, reference)| {
                TextPayloadBinding::new(reference, format!("教学笔记-{index}.txt"), "plaintext")
                    .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
fn draft(bytes: &[u8]) -> ArtifactDraft {
    ArtifactDraft::new(
        "teaching.notes".parse().unwrap(),
        "教学内容，不是代码",
        "text/plain",
        bytes.to_vec(),
    )
    .unwrap()
}
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
        // Read-only commands still need explicit PostgreSQL row-lock column privileges.
        // These are fixture administration, never a product grant or broad table UPDATE.
        for grant in [
            "SELECT,UPDATE(singleton) ON collaboration_artifact_schema",
            "SELECT,UPDATE(artifact_id) ON collaboration_artifacts",
            "SELECT ON collaboration_artifact_revisions",
            "SELECT ON collaboration_artifact_outbox",
        ] {
            let qualified = grant.replace(" ON ", &format!(" ON {}.", f.artifacts));
            f.sql(&format!("GRANT {qualified} TO {}", f.role)).await;
        }
        Self { base }
    }
    pub(super) async fn connector(&self) -> PlatformTaskContentConnector {
        PlatformTaskContentConnector::new(self.storage().await)
    }
    pub(super) fn pooled(&self) -> (DurableAuthSource, SessionTaskContentWorkspace) {
        // Existing pooled private commands deliberately use their original full relation-lock
        // policy. This trusted positive control is separate from the narrow retained host role.
        let source =
            DurableAuthSource::new(self.f.owner().clone(), self.f.config().scope().authority_id);
        let artifacts = source
            .open_artifact_workspace(&self.f.artifacts, self.f.config().scope(), self.root.path())
            .unwrap();
        let workspace =
            SessionTaskContentWorkspace::new(&self.f.task, self.f.config().scope(), artifacts)
                .unwrap();
        (source, workspace)
    }
    pub(super) async fn artifact(&self, owner: usize, bytes: &[u8]) -> ArtifactRevision {
        schema::path(self.f, &self.f.artifacts).await;
        let result = ArtifactStore::open(
            self.f.owner().clone(),
            self.f.config().scope(),
            self.root.path(),
        )
        .unwrap()
        .create(
            uuid::Uuid::new_v4().try_into().unwrap(),
            uuid::Uuid::new_v4().try_into().unwrap(),
            self.owners[owner],
            draft(bytes),
        )
        .await
        .unwrap();
        schema::path(self.f, &self.f.source).await;
        result
    }
    pub(super) async fn revise(&self, old: &ArtifactRevision, bytes: &[u8]) -> ArtifactRevision {
        schema::path(self.f, &self.f.artifacts).await;
        let result = ArtifactStore::open(
            self.f.owner().clone(),
            self.f.config().scope(),
            self.root.path(),
        )
        .unwrap()
        .revise(
            &old.reference,
            uuid::Uuid::new_v4().try_into().unwrap(),
            old.owner,
            draft(bytes),
        )
        .await
        .unwrap();
        schema::path(self.f, &self.f.source).await;
        result
    }
    pub(super) fn blob(&self, reference: &ArtifactRef) -> PathBuf {
        self.root.path().join(format!(
            "{}-{}-{}-{}-{}",
            reference.workspace.authority_id,
            reference.workspace.workspace_id,
            reference.artifact_id,
            reference.revision_id,
            reference.sha256
        ))
    }
    pub(super) async fn read(
        &self,
        connector: &PlatformTaskContentConnector,
        owner: usize,
        task: &TaskContentSnapshot,
    ) -> Result<Option<SessionTaskContent>, SessionTaskError> {
        let before = snapshot(self.f).await;
        let catalog = self.f.snapshot().await;
        let files = self.files();
        let result = connector
            .get_session_content_task(&self.tokens[owner], task.task.reference.task_id)
            .await;
        assert!(
            before == snapshot(self.f).await,
            "reopen changed private rows"
        );
        assert!(
            catalog == self.f.snapshot().await,
            "reopen changed catalog state"
        );
        assert!(files == self.files(), "reopen changed private files");
        result
    }
    pub(super) fn files(&self) -> Vec<(String, u32, Vec<u8>)> {
        let mut result = fs::read_dir(self.root.path())
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (
                    entry.file_name().into_string().unwrap(),
                    entry.metadata().unwrap().mode(),
                    fs::read(entry.path()).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        result.sort_by(|left, right| left.0.cmp(&right.0));
        result
    }
    pub(super) fn finish(self) -> bool {
        self.base.finish()
    }
}
