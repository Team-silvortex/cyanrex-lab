//! Trusted private Unix chunk staging, not authorization, publication or a resumable transfer.
//!
//! Roots must already exist and be privately owned. Each instance has its own fail-fast worker
//! budget; clones share it. A blocking job retains its slot after a waiting future is dropped.
//! Errors may retain partial or complete unconfirmed files. Nothing is adopted, overwritten or
//! deleted. Verified existing bytes are not proof of a prior successful write or durable receipt.
use crate::models::collaboration::{FileTransferManifest, WorkspaceRef};
use std::{io::Read, path::Path, sync::Arc};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

mod assemble;
mod assembled_proof;
mod binding;
mod bound;
mod directory;
mod filesystem;
#[cfg(test)]
mod queued_tests;
mod read;
mod save;
mod save_directory;
mod save_io;
mod whole;
mod work;
mod work_assemble;
mod work_bound;
mod work_read;
mod work_save;
mod work_whole;

#[cfg(test)]
mod assembly_flow_tests;
#[cfg(test)]
mod assembly_test_support;
#[cfg(test)]
mod assembly_tests;

pub use assemble::AssembledFile;
pub(crate) use bound::BoundChunkReceipt;
pub(crate) use read::BufferedBoundChunkRead;
#[cfg(test)]
pub(crate) use save::SaveTestPhase;
pub use save::{FileSaveError, FileSaveTarget, SavedFile};
pub(crate) use whole::BoundFileObservation;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ChunkStagingError {
    #[error("invalid staging input")]
    InvalidInput,
    #[error("staging capacity exhausted")]
    CapacityExhausted,
    #[error("staging entry already exists")]
    Conflict,
    #[error("staging unavailable")]
    Unavailable,
    #[error("staging outcome unconfirmed")]
    Unconfirmed,
}

#[derive(Clone)]
pub struct ChunkStaging {
    inner: Arc<Inner>,
}

struct Inner {
    scope: WorkspaceRef,
    directory: directory::ChunkDirectory,
    workers: Arc<Semaphore>,
}

impl ChunkStaging {
    /// Opens only an existing root. Does not create, chmod or recover a storage directory.
    pub fn open(
        scope: WorkspaceRef,
        root: &Path,
        workers: usize,
    ) -> Result<Self, ChunkStagingError> {
        if !(1..=16).contains(&workers) {
            return Err(ChunkStagingError::InvalidInput);
        }
        let directory =
            directory::ChunkDirectory::open(root).map_err(|_| ChunkStagingError::Unavailable)?;
        Ok(Self {
            inner: Arc::new(Inner {
                scope,
                directory,
                workers: Arc::new(Semaphore::new(workers)),
            }),
        })
    }

    fn validate(
        &self,
        manifest: &FileTransferManifest,
        index: u32,
    ) -> Result<(), ChunkStagingError> {
        if manifest.workspace() != self.inner.scope || manifest.chunk(index).is_err() {
            return Err(ChunkStagingError::InvalidInput);
        }
        Ok(())
    }

    pub(crate) fn scope(&self) -> WorkspaceRef {
        self.inner.scope
    }

    /// Creates one verified private whole-file output from the exact legacy staged chunks.
    /// This is local acknowledgement metadata, not publication, delivery or later authority.
    pub async fn assemble_staged_file(
        &self,
        manifest: &FileTransferManifest,
    ) -> Result<AssembledFile, ChunkStagingError> {
        self.assemble_staged_file_leased(manifest, None).await
    }

    /// Copies the original acknowledged assembly into an explicitly selected new private file.
    /// The trusted host selects the target independently of any remote display name or path.
    pub async fn save_assembled_file(
        &self,
        manifest: &FileTransferManifest,
        assembled: &AssembledFile,
        target: &FileSaveTarget,
    ) -> Result<SavedFile, FileSaveError> {
        self.save_assembled_file_leased(manifest, assembled, target, None)
            .await
    }

    /// Cached identity only; does not inspect current roots or reserve a selected entry.
    pub(crate) fn validate_save_target(
        &self,
        target: &FileSaveTarget,
    ) -> Result<(), FileSaveError> {
        save::validate_target(&self.inner, target)
    }

    /// A whole-download slot follows the actual queued/running selected save, not only its waiter.
    pub(crate) async fn save_assembled_file_leased(
        &self,
        manifest: &FileTransferManifest,
        assembled: &AssembledFile,
        target: &FileSaveTarget,
        flow: Option<Arc<OwnedSemaphorePermit>>,
    ) -> Result<SavedFile, FileSaveError> {
        save::run(&self.inner, manifest, assembled, target, flow).await
    }

    /// An internal whole-download slot remains owned by actual queued/running assembly work.
    pub(crate) async fn assemble_staged_file_leased(
        &self,
        manifest: &FileTransferManifest,
        flow: Option<Arc<OwnedSemaphorePermit>>,
    ) -> Result<AssembledFile, ChunkStagingError> {
        if manifest.workspace() != self.inner.scope {
            return Err(ChunkStagingError::InvalidInput);
        }
        work_assemble::run(&self.inner, manifest, flow).await
    }

    /// Binds trusted source metadata to the originally opened root using cached identity only.
    pub(crate) fn bind_upload(&self, source_key: [u8; 32]) -> [u8; 32] {
        self.inner.directory.bind_upload(source_key)
    }

    /// Private verified read bytes only. Caller authority must be rechecked before disclosure.
    pub(crate) async fn read_bound_chunk(
        &self,
        key: [u8; 32],
        manifest: &FileTransferManifest,
        index: u32,
        lease: OwnedSemaphorePermit,
    ) -> Result<BufferedBoundChunkRead, ChunkStagingError> {
        self.validate(manifest, index)?;
        work_read::run(&self.inner, key, manifest, index, lease).await
    }

    /// Produces a local original-object proof, never durable database chunk progress.
    pub(crate) async fn stage_bound_chunk<R: Read + Send + 'static>(
        &self,
        key: [u8; 32],
        manifest: &FileTransferManifest,
        index: u32,
        reader: R,
    ) -> Result<BoundChunkReceipt, ChunkStagingError> {
        self.validate(manifest, index)?;
        work_bound::run(
            &self.inner,
            key,
            manifest,
            index,
            work_bound::Operation::Stage(reader),
        )
        .await
    }

    /// Rechecks the original retained descriptor and entry; matching other bytes are not adopted.
    pub(crate) async fn verify_bound_receipt(
        &self,
        key: [u8; 32],
        manifest: &FileTransferManifest,
        index: u32,
        receipt: BoundChunkReceipt,
    ) -> Result<BoundChunkReceipt, ChunkStagingError> {
        self.validate(manifest, index)?;
        receipt.require_binding(key, manifest, index)?;
        work_bound::run(
            &self.inner,
            key,
            manifest,
            index,
            work_bound::Operation::<std::io::Empty>::Verify(receipt),
        )
        .await
    }

    /// Observes current ordered chunk bytes and the whole digest, never historical write success.
    pub(crate) async fn observe_bound_file(
        &self,
        key: [u8; 32],
        manifest: &FileTransferManifest,
    ) -> Result<BoundFileObservation, ChunkStagingError> {
        if manifest.workspace() != self.inner.scope {
            return Err(ChunkStagingError::InvalidInput);
        }
        work_whole::observe(&self.inner, key, manifest).await
    }

    /// Rechecks the original observed metadata and root, not a fresh adopted byte observation.
    pub(crate) async fn recheck_bound_file(
        &self,
        key: [u8; 32],
        manifest: &FileTransferManifest,
        observation: BoundFileObservation,
    ) -> Result<(), ChunkStagingError> {
        if manifest.workspace() != self.inner.scope {
            return Err(ChunkStagingError::InvalidInput);
        }
        work_whole::recheck(&self.inner, key, manifest, observation).await
    }

    /// Retains the same original whole-byte observation across a later metadata acknowledgement.
    pub(crate) async fn recheck_bound_file_retained(
        &self,
        key: [u8; 32],
        manifest: &FileTransferManifest,
        observation: BoundFileObservation,
    ) -> Result<BoundFileObservation, ChunkStagingError> {
        if manifest.workspace() != self.inner.scope {
            return Err(ChunkStagingError::InvalidInput);
        }
        work_whole::recheck_retained(&self.inner, key, manifest, observation).await
    }

    /// Confirms one local fsync/readback sequence, never a publication or whole-file integrity.
    /// Failure after creation retains output; a repeated call always conflicts with that entry.
    pub async fn stage_chunk<R: Read + Send + 'static>(
        &self,
        manifest: &FileTransferManifest,
        index: u32,
        reader: R,
    ) -> Result<(), ChunkStagingError> {
        self.validate(manifest, index)?;
        work::run(&self.inner, manifest, index, work::Operation::Stage(reader)).await
    }

    /// Checks current staged bytes only; does not adopt an entry or confirm past durability.
    pub async fn verify_staged_chunk(
        &self,
        manifest: &FileTransferManifest,
        index: u32,
    ) -> Result<(), ChunkStagingError> {
        self.validate(manifest, index)?;
        work::run(
            &self.inner,
            manifest,
            index,
            work::Operation::<std::io::Empty>::Verify,
        )
        .await
    }
}
