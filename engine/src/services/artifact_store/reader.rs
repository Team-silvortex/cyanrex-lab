//! Pool-free exact reads. Host worker admission never changes the original store's policy.
use super::*;
use std::fs::File;
use tokio::sync::Semaphore;

#[derive(Clone)]
enum Workers {
    Store,
    Host(Arc<Semaphore>),
}

/// Opaque original root plus scope, never authentication, a pool or a path-bearing public API.
#[derive(Clone)]
pub(crate) struct ArtifactContentReader {
    scope: WorkspaceRef,
    blobs: Arc<blob::BlobDirectory>,
    workers: Workers,
}

impl ArtifactContentReader {
    pub(crate) fn scope(&self) -> WorkspaceRef {
        self.scope
    }

    /// Explicit compatibility policy: old stores retain their prior blocking-worker behavior.
    pub(crate) fn from_store(store: &ArtifactStore) -> Self {
        Self {
            scope: store.scope,
            blobs: store.blobs.clone(),
            workers: Workers::Store,
        }
    }

    /// Duplicate only the caller's original descriptor, then verify its visible no-follow root.
    /// This never enumerates content, creates a directory or adopts a newly opened baseline.
    pub(crate) fn from_retained_root(
        scope: WorkspaceRef,
        path: &Path,
        original: &File,
        workers: Arc<Semaphore>,
    ) -> Result<Self> {
        Ok(Self {
            scope,
            blobs: Arc::new(blob::BlobDirectory::from_retained_root(path, original)?),
            workers: Workers::Host(workers),
        })
    }

    fn check_scope(&self, workspace: WorkspaceRef, owner: PrincipalRef) -> Result<()> {
        check_scope(self.scope, workspace, owner)
    }

    /// Admission is fail-fast. The blocking closure, not its cancellable waiter, owns the slot.
    pub(super) async fn blocking<T: Send + 'static>(
        &self,
        operation: impl FnOnce(Arc<blob::BlobDirectory>) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let permit = match &self.workers {
            Workers::Store => None,
            Workers::Host(workers) => Some(
                workers
                    .clone()
                    .try_acquire_owned()
                    .map_err(|_| ArtifactStoreError::StorageUnavailable)?,
            ),
        };
        let blobs = self.blobs.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            operation(blobs)
        })
        .await
        .map_err(|_| ArtifactStoreError::BlobUnavailable)?
    }

    pub(crate) async fn verify_root(&self) -> Result<()> {
        self.blocking(|blobs| blobs.verify()).await
    }

    pub(super) async fn read_blob(&self, revision: &ArtifactRevision) -> Result<Vec<u8>> {
        let revision = revision.clone();
        self.blocking(move |blobs| blobs.read(&revision)).await
    }

    pub(crate) async fn validate_namespace_in_transaction(&self, tx: &mut Tx<'_>) -> Result<()> {
        schema::check_schema(self.scope, tx, true).await
    }

    /// No acquisition or commit; the caller retains schema/head locks and final authorization.
    pub(crate) async fn read_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        reference: &ArtifactRef,
        owner: PrincipalRef,
    ) -> Result<Option<ArtifactContent>> {
        self.check_scope(reference.workspace, owner)?;
        self.read_content(tx, reference, owner, true).await
    }

    pub(super) async fn read_content(
        &self,
        tx: &mut Tx<'_>,
        reference: &ArtifactRef,
        owner: PrincipalRef,
        lock: bool,
    ) -> Result<Option<ArtifactContent>> {
        schema::check_schema(self.scope, tx, lock).await?;
        let Some(head) = records::head(self.scope, tx, reference.artifact_id, owner, lock).await?
        else {
            return Ok(None);
        };
        let found = if head.reference.revision_id == reference.revision_id {
            Some(head.clone())
        } else {
            records::revision(
                self.scope,
                tx,
                reference.artifact_id,
                reference.revision_id,
                owner,
            )
            .await?
        };
        let Some(revision) = found else {
            return Ok(None);
        };
        if revision.reference != *reference
            || u64::from(revision.sequence) > u64::from(head.sequence)
        {
            return Err(ArtifactStoreError::InvalidRecord);
        }
        let bytes = self.read_blob(&revision).await?;
        Ok(Some(ArtifactContent { revision, bytes }))
    }
}

#[cfg(test)]
#[path = "reader_tests.rs"]
mod tests;
