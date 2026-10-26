//! Pool-free create capability; the original root and actual-worker policy remain caller-owned.
use super::*;

pub(crate) struct ArtifactContentWriter {
    reader: ArtifactContentReader,
    lock_head: bool,
}

impl ArtifactContentWriter {
    pub(crate) fn from_reader(reader: ArtifactContentReader) -> Self {
        // A retained create owns its inserted head row; no extra UPDATE privilege is needed.
        Self {
            reader,
            lock_head: false,
        }
    }

    /// Existing stores retain their original head row locks and blocking-worker policy.
    pub(super) fn from_store(store: &ArtifactStore) -> Self {
        Self {
            reader: ArtifactContentReader::from_store(store),
            lock_head: true,
        }
    }

    pub(crate) fn scope(&self) -> WorkspaceRef {
        self.reader.scope()
    }

    pub(crate) async fn create_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        artifact: ArtifactId,
        revision: ArtifactRevisionId,
        owner: PrincipalRef,
        draft: ArtifactDraft,
    ) -> Result<ArtifactRevision> {
        commands::create_in_transaction(self, tx, artifact, revision, owner, draft).await
    }

    pub(crate) async fn verify_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        expected: &ArtifactRevision,
    ) -> Result<()> {
        check_scope(self.scope(), expected.reference.workspace, expected.owner)?;
        self.reader.validate_namespace_in_transaction(tx).await?;
        // A legal historical revision read is insufficient: publication must retain this head.
        if records::head(
            self.scope(),
            tx,
            expected.reference.artifact_id,
            expected.owner,
            self.lock_head,
        )
        .await?
        .as_ref()
            != Some(expected)
        {
            return Err(ArtifactStoreError::InvalidRecord);
        }
        self.reader.read_blob(expected).await?;
        Ok(())
    }

    pub(crate) async fn verify_root(&self) -> Result<()> {
        self.reader.verify_root().await
    }

    pub(super) async fn write_blob(
        &self,
        revision: &ArtifactRevision,
        bytes: Vec<u8>,
    ) -> Result<()> {
        let revision = revision.clone();
        // The same closure retains the original root, bytes and permit after waiter drop/timeout.
        self.reader
            .blocking(move |blobs| blobs.write_once(&revision, &bytes))
            .await
    }
}

#[cfg(test)]
#[path = "writer_tests.rs"]
mod tests;
