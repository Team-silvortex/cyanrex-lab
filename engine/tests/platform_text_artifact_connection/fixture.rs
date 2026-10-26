pub(super) use super::content_fixture::{snapshot, token_hash, within, Fixture};
use super::{content_fixture, schema};
use cyanrex_engine::{
    models::collaboration::*,
    platform_host::PlatformTextArtifactConnector,
    services::{
        artifact_store::ArtifactStore,
        auth_service::durable_source::{
            DurableAuthError, SessionArtifactError, SessionCommandError,
        },
    },
};
use sha2::{Digest, Sha256};
use sqlx_core::{query::query, row::Row};
use std::{fs, os::unix::fs::MetadataExt, path::PathBuf};

pub(super) type Files = Vec<(String, u64, u64, u32, u64, Vec<u8>)>;
pub(super) fn artifact_id(n: u128) -> ArtifactId {
    uuid::Uuid::from_u128(9500 + n).try_into().unwrap()
}
pub(super) fn revision_id(n: u128) -> ArtifactRevisionId {
    uuid::Uuid::from_u128(9600 + n).try_into().unwrap()
}
pub(super) fn invalid_session() -> SessionArtifactError {
    SessionCommandError::InvalidSession.into()
}
pub(super) fn unavailable() -> SessionArtifactError {
    DurableAuthError::StorageUnavailable.into()
}
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
        // Only explicit fixture provisioning: three INSERT grants, no Task or Source writes.
        for table in [
            "collaboration_artifacts",
            "collaboration_artifact_revisions",
            "collaboration_artifact_outbox",
        ] {
            f.sql(&format!(
                "GRANT INSERT ON {}.{table} TO {}",
                f.artifacts, f.role
            ))
            .await;
        }
        // Create-only validation owns its inserted row; no Artifact business UPDATE is needed.
        f.sql(&format!(
            "REVOKE UPDATE(artifact_id) ON {}.collaboration_artifacts FROM {}",
            f.artifacts, f.role
        ))
        .await;
        for (table, column) in [
            ("collaboration_tasks", "task_id"),
            ("collaboration_task_outbox", "event_id"),
        ] {
            f.sql(&format!(
                "REVOKE UPDATE({column}) ON {}.{table} FROM {}",
                f.task, f.role
            ))
            .await;
        }
        Self { base }
    }
    pub(super) async fn connector(&self) -> PlatformTextArtifactConnector {
        PlatformTextArtifactConnector::new(self.storage().await)
    }
    pub(super) async fn create(
        &self,
        connector: &PlatformTextArtifactConnector,
        owner: usize,
        n: u128,
        text: &str,
    ) -> Result<ArtifactRevision, SessionArtifactError> {
        connector
            .create_session_text_artifact(
                &self.tokens[owner],
                artifact_id(n),
                revision_id(n),
                "协作讲义，不要求代码或执行".to_owned(),
                text.to_owned(),
            )
            .await
    }
    pub(super) fn reference(&self, n: u128, bytes: &[u8]) -> ArtifactRef {
        ArtifactRef {
            workspace: self.f.config().scope(),
            artifact_id: artifact_id(n),
            revision_id: revision_id(n),
            sha256: format!("{:x}", Sha256::digest(bytes)).parse().unwrap(),
        }
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
    pub(super) fn files(&self) -> Files {
        let mut files = fs::read_dir(self.root.path())
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                let metadata = fs::symlink_metadata(entry.path()).unwrap();
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
        files.sort_by(|a, b| a.0.cmp(&b.0));
        files
    }
    pub(super) fn old_files_unchanged(&self, original: &Files, additions: usize) {
        let actual = self.files();
        assert_eq!(actual.len(), original.len() + additions);
        for expected in original {
            assert_eq!(
                actual.iter().find(|file| file.0 == expected.0),
                Some(expected)
            );
        }
    }
    pub(super) async fn counts(&self, expected: i64) {
        for table in [
            "collaboration_artifacts",
            "collaboration_artifact_revisions",
            "collaboration_artifact_outbox",
        ] {
            let count: i64 = within(
                query(&format!(
                    "SELECT count(*) AS count FROM {}.{table}",
                    self.f.artifacts
                ))
                .fetch_one(self.f.owner()),
            )
            .await
            .unwrap()
            .get("count");
            assert_eq!(count, expected, "{table}");
        }
    }
    pub(super) async fn non_artifacts(&self) -> Vec<(String, String)> {
        snapshot(self.f)
            .await
            .into_iter()
            .filter(|(table, _)| !table.starts_with(&format!("{}.", self.f.artifacts)))
            .collect()
    }
    pub(super) async fn read_committed(&self, expected: &ArtifactRevision, bytes: &[u8]) {
        // Trusted independent oracle, never evidence that the retained role has extra privileges.
        schema::path(self.f, &self.f.artifacts).await;
        let store = ArtifactStore::open(
            self.f.owner().clone(),
            self.f.config().scope(),
            self.root.path(),
        )
        .unwrap();
        let content = store
            .read(&expected.reference, expected.owner)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(content.revision, *expected);
        assert_eq!(content.bytes, bytes);
        schema::path(self.f, &self.f.source).await;
    }
    pub(super) fn finish(self) -> bool {
        self.base.finish()
    }
}
