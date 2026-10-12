//! Explicit private single-chunk reception, not current authority or a remote download receipt.
use crate::{
    models::collaboration::{FileTransferManifest, FileVersionRef, Sha256Digest, WorkspaceRef},
    services::file_transfer::staging::{ChunkStaging, ChunkStagingError},
};
use axum::{body::Bytes, http::Response};
use std::{sync::Arc, time::Duration};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

mod contract;
mod leased;
mod receive;

/// Trusted staging composition only. No directory or network connection is created here.
/// Clones share receive capacity; independently constructed receivers have additive budgets.
#[derive(Clone)]
pub struct FileChunkReceiver {
    staging: ChunkStaging,
    scope: WorkspaceRef,
    controls: Arc<Semaphore>,
    idle: Duration,
    total: Duration,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum FileChunkReceiveError {
    #[error("invalid chunk receive input or configuration")]
    InvalidInput,
    #[error("invalid chunk response")]
    InvalidResponse,
    #[error("chunk receive capacity exhausted")]
    CapacityExhausted,
    #[error("chunk response reception timed out")]
    TimedOut,
    #[error("private chunk receive buffer unavailable")]
    Unavailable,
    #[error(transparent)]
    Staging(#[from] ChunkStagingError),
}

/// Metadata from this invocation's local staging acknowledgement only.
/// Not current authority, a remote receipt, whole-file verification or resumable progress.
/// No public construction, cloning, serialization, byte extraction, path or hidden key exists.
///
/// ```compile_fail
/// use cyanrex_engine::{models::collaboration::{FileVersionRef, Sha256Digest},
///     services::file_chunk_receiver::ReceivedFileChunk};
/// fn fabricate(file: FileVersionRef, sha256: Sha256Digest) -> ReceivedFileChunk {
///     ReceivedFileChunk { file, index: 0, byte_length: 17, sha256 }
/// }
/// ```
/// ```compile_fail
/// use cyanrex_engine::services::file_chunk_receiver::ReceivedFileChunk;
/// fn duplicate(value: ReceivedFileChunk) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use cyanrex_engine::services::file_chunk_receiver::ReceivedFileChunk;
/// fn serialize(value: ReceivedFileChunk) { let _ = serde_json::to_vec(&value); }
/// ```
pub struct ReceivedFileChunk {
    file: FileVersionRef,
    index: u32,
    byte_length: u64,
    sha256: Sha256Digest,
}

impl ReceivedFileChunk {
    pub fn file(&self) -> &FileVersionRef {
        &self.file
    }

    pub fn chunk_index(&self) -> u32 {
        self.index
    }

    pub fn byte_length(&self) -> u64 {
        self.byte_length
    }

    pub fn chunk_sha256(&self) -> &Sha256Digest {
        &self.sha256
    }
}

impl FileChunkReceiver {
    pub fn new(staging: ChunkStaging) -> Self {
        Self::with_limits(staging, 4, Duration::from_secs(5), Duration::from_secs(60))
            .expect("fixed receiver limits are valid")
    }

    /// Idle and total receive deadlines accept fractional seconds within their fixed bounds.
    /// They do not terminate already dispatched blocking staging work.
    pub fn with_limits(
        staging: ChunkStaging,
        capacity: usize,
        idle: Duration,
        total: Duration,
    ) -> Result<Self, FileChunkReceiveError> {
        if !(1..=16).contains(&capacity)
            || !(Duration::from_secs(1)..=Duration::from_secs(60)).contains(&idle)
            || total < idle
            || total > Duration::from_secs(300)
        {
            return Err(FileChunkReceiveError::InvalidInput);
        }
        Ok(Self {
            scope: staging.scope(),
            staging,
            controls: Arc::new(Semaphore::new(capacity)),
            idle,
            total,
        })
    }

    pub async fn receive_and_stage<B>(
        &self,
        manifest: &FileTransferManifest,
        index: u32,
        response: Response<B>,
    ) -> Result<ReceivedFileChunk, FileChunkReceiveError>
    where
        B: http_body::Body<Data = Bytes> + Send + 'static,
    {
        self.receive_and_stage_leased(manifest, index, response, None)
            .await
    }

    /// An internal caller's flow lease follows the private bytes into the actual staging job.
    /// It confers no download authority and does not alter public receiver admission policy.
    pub(crate) async fn receive_and_stage_leased<B>(
        &self,
        manifest: &FileTransferManifest,
        index: u32,
        response: Response<B>,
        caller: Option<OwnedSemaphorePermit>,
    ) -> Result<ReceivedFileChunk, FileChunkReceiveError>
    where
        B: http_body::Body<Data = Bytes> + Send + 'static,
    {
        let mut reader = self.receive_checked(manifest, index, response).await?;
        reader.retain_caller(caller);
        self.stage_checked(manifest, index, reader).await
    }

    async fn receive_checked<B>(
        &self,
        manifest: &FileTransferManifest,
        index: u32,
        response: Response<B>,
    ) -> Result<leased::LeasedCursor, FileChunkReceiveError>
    where
        B: http_body::Body<Data = Bytes> + Send + 'static,
    {
        if manifest.workspace() != self.scope {
            return Err(FileChunkReceiveError::InvalidInput);
        }
        let spec = manifest
            .chunk(index)
            .map_err(|_| FileChunkReceiveError::InvalidInput)?;
        contract::validate(&response, manifest, &spec)?;
        // Cheap metadata rejects precede admission, allocation and the first body poll.
        let permit = self
            .controls
            .clone()
            .try_acquire_owned()
            .map_err(|_| FileChunkReceiveError::CapacityExhausted)?;
        let bytes = receive::collect(response.into_body(), &spec, self.idle, self.total).await?;
        Ok(leased::LeasedCursor::new(bytes, permit))
    }

    async fn stage_checked(
        &self,
        manifest: &FileTransferManifest,
        index: u32,
        reader: leased::LeasedCursor,
    ) -> Result<ReceivedFileChunk, FileChunkReceiveError> {
        // The Vec and receive permit move into the existing real blocking staging job. Dropping
        // this waiter cannot release either while that job remains queued, writing or checking.
        self.staging.stage_chunk(manifest, index, reader).await?;
        let spec = manifest
            .chunk(index)
            .map_err(|_| FileChunkReceiveError::InvalidInput)?;
        Ok(ReceivedFileChunk {
            file: FileVersionRef::from_manifest(manifest),
            index,
            byte_length: spec.byte_length,
            sha256: spec.sha256,
        })
    }
}

#[cfg(test)]
mod work_tests;
