//! Explicit metadata/chunk requests and private assembly, never mounting or download authority.
use crate::{
    models::collaboration::{
        FileGrantId, FileTransferManifest, FileUploadId, FileVersionRef, WorkspaceRef,
    },
    services::{
        file_chunk_receiver::{FileChunkReceiveError, FileChunkReceiver, ReceivedFileChunk},
        file_transfer::staging::{
            AssembledFile, ChunkStaging, ChunkStagingError, FileSaveError, FileSaveTarget,
            SavedFile,
        },
    },
};
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;

mod cancel;
mod controlled;
mod delivery;
mod download;
mod manifest;
mod manifest_contract;
mod manifest_receive;
mod policy;
mod progress;
mod request;

pub use cancel::{FileDeliveryCancelHandle, FileDeliveryCancellation};
pub use controlled::FileControlledSaveError;
pub use progress::{
    FileDeliveryObserver, FileDeliveryPhase, FileDeliveryProgress, FileDeliverySnapshot,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChunkTransport {
    Https,
    LoopbackHttp,
}

/// Host-supplied configuration only, never remote metadata or a public request payload.
#[derive(Clone)]
pub struct FileChunkClientConfig {
    pub transport: FileChunkTransport,
    pub capacity: usize,
    pub header_timeout: Duration,
    pub idle_timeout: Duration,
    pub total_timeout: Duration,
    pub trusted_root_pem: Option<Vec<u8>>,
}

impl Default for FileChunkClientConfig {
    fn default() -> Self {
        Self {
            transport: FileChunkTransport::Https,
            capacity: 4,
            header_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(5),
            total_timeout: Duration::from_secs(60),
            trusted_root_pem: None,
        }
    }
}

/// Clones retain the same private trusted composition. No public Client or arbitrary URL exists.
#[derive(Clone)]
pub struct FileChunkClient {
    inner: Arc<Inner>,
}

struct Inner {
    endpoint: reqwest::Url,
    manifest_endpoint: reqwest::Url,
    scope: WorkspaceRef,
    staging: ChunkStaging,
    receiver: FileChunkReceiver,
    http: reqwest::Client,
    controls: Arc<Semaphore>,
    downloads: Arc<Semaphore>,
    manifests: Arc<Semaphore>,
    header_timeout: Duration,
    idle_timeout: Duration,
    total_timeout: Duration,
    #[cfg(test)]
    eof_observer: Option<Arc<std::sync::atomic::AtomicBool>>,
    #[cfg(test)]
    manifest_eof_observer: Option<Arc<std::sync::atomic::AtomicBool>>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum FileChunkClientError {
    #[error("invalid chunk client configuration")]
    InvalidConfiguration,
    #[error("invalid chunk request input")]
    InvalidInput,
    #[error("chunk request capacity exhausted")]
    CapacityExhausted,
    #[error("chunk request response headers timed out")]
    TimedOut,
    #[error("chunk request unavailable")]
    Unavailable,
    #[error(transparent)]
    Receive(#[from] FileChunkReceiveError),
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum FileDownloadError {
    #[error("invalid file download input")]
    InvalidInput,
    #[error("file download capacity exhausted")]
    CapacityExhausted,
    #[error("file download chunk {index} failed: {error}")]
    Chunk {
        index: u32,
        error: FileChunkClientError,
    },
    #[error(transparent)]
    Assembly(ChunkStagingError),
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum FileManifestClientError {
    #[error("invalid file manifest request input")]
    InvalidInput,
    #[error("file manifest request capacity exhausted")]
    CapacityExhausted,
    #[error("invalid file manifest response")]
    InvalidResponse,
    #[error("file manifest response timed out")]
    TimedOut,
    #[error("file manifest request unavailable")]
    Unavailable,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum FileGrantedDownloadError {
    #[error("invalid granted file download input")]
    InvalidInput,
    #[error("granted file download capacity exhausted")]
    CapacityExhausted,
    #[error(transparent)]
    Manifest(FileManifestClientError),
    #[error(transparent)]
    Download(FileDownloadError),
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum FileGrantedSaveError {
    #[error("invalid granted file save input")]
    InvalidInput,
    #[error("granted file save capacity exhausted")]
    CapacityExhausted,
    #[error(transparent)]
    Download(FileGrantedDownloadError),
    #[error(transparent)]
    Save(FileSaveError),
}

impl FileChunkClient {
    pub fn new_https(origin: &str, staging: ChunkStaging) -> Result<Self, FileChunkClientError> {
        Self::with_config(origin, staging, FileChunkClientConfig::default())
    }

    /// Explicit host opt-in only; this constructor does not discover or contact a peer.
    pub fn new_loopback(origin: &str, staging: ChunkStaging) -> Result<Self, FileChunkClientError> {
        Self::with_config(
            origin,
            staging,
            FileChunkClientConfig {
                transport: FileChunkTransport::LoopbackHttp,
                ..FileChunkClientConfig::default()
            },
        )
    }

    pub fn with_config(
        origin: &str,
        staging: ChunkStaging,
        config: FileChunkClientConfig,
    ) -> Result<Self, FileChunkClientError> {
        let (endpoint, http) = policy::configure(origin, &config)?;
        let mut manifest_endpoint = endpoint.clone();
        manifest_endpoint.set_path("/platform/v1/file-manifests");
        let scope = staging.scope();
        let original_staging = staging.clone();
        let receiver = FileChunkReceiver::with_limits(
            staging,
            config.capacity,
            config.idle_timeout,
            config.total_timeout,
        )
        .map_err(|_| FileChunkClientError::InvalidConfiguration)?;
        Ok(Self {
            inner: Arc::new(Inner {
                endpoint,
                manifest_endpoint,
                scope,
                staging: original_staging,
                receiver,
                http,
                controls: Arc::new(Semaphore::new(config.capacity)),
                downloads: Arc::new(Semaphore::new(config.capacity)),
                manifests: Arc::new(Semaphore::new(config.capacity)),
                header_timeout: config.header_timeout,
                idle_timeout: config.idle_timeout,
                total_timeout: config.total_timeout,
                #[cfg(test)]
                eof_observer: None,
                #[cfg(test)]
                manifest_eof_observer: None,
            }),
        })
    }

    pub async fn fetch_and_stage(
        &self,
        token: &str,
        grant_id: FileGrantId,
        upload_id: FileUploadId,
        manifest: &FileTransferManifest,
        index: u32,
    ) -> Result<ReceivedFileChunk, FileChunkClientError> {
        request::fetch(&self.inner, token, grant_id, upload_id, manifest, index).await
    }

    /// Explicit ordered chunk download followed by one private local whole-file assembly.
    /// The supplied manifest is not fetched or authenticated here; even empty input establishes
    /// no remote authority. Existing files are never adopted as progress or permission to retry.
    pub async fn fetch_and_assemble(
        &self,
        token: &str,
        grant_id: FileGrantId,
        upload_id: FileUploadId,
        manifest: &FileTransferManifest,
    ) -> Result<AssembledFile, FileDownloadError> {
        if manifest.workspace() != self.inner.scope
            || token.len() != 36
            || !uuid::Uuid::parse_str(token).is_ok_and(|id| !id.is_nil() && id.to_string() == token)
        {
            return Err(FileDownloadError::InvalidInput);
        }
        download::run(&self.inner, token, grant_id, upload_id, manifest).await
    }

    /// Fetch one exact version's complete metadata through the fixed manifest endpoint.
    /// A matching response is not reusable byte authority, a receipt or continuous authorization.
    pub async fn fetch_manifest(
        &self,
        token: &str,
        grant_id: FileGrantId,
        upload_id: FileUploadId,
        expected_file: &FileVersionRef,
    ) -> Result<FileTransferManifest, FileManifestClientError> {
        manifest::fetch(&self.inner, token, grant_id, upload_id, expected_file).await
    }

    /// Observe the exact remote manifest before ordered private download and assembly.
    /// Unlike the supplied-manifest entry, even empty files require that metadata GET.
    /// Success does not establish continuing grant validity or current remote principal provenance.
    pub async fn fetch_granted_and_assemble(
        &self,
        token: &str,
        grant_id: FileGrantId,
        upload_id: FileUploadId,
        expected_file: &FileVersionRef,
    ) -> Result<AssembledFile, FileGrantedDownloadError> {
        if !manifest_inputs_valid(&self.inner, token, expected_file) {
            return Err(FileGrantedDownloadError::InvalidInput);
        }
        let flow = Arc::new(
            self.inner
                .downloads
                .clone()
                .try_acquire_owned()
                .map_err(|_| FileGrantedDownloadError::CapacityExhausted)?,
        );
        let manifest = manifest::fetch(&self.inner, token, grant_id, upload_id, expected_file)
            .await
            .map_err(FileGrantedDownloadError::Manifest)?;
        download::run_with_flow(&self.inner, token, grant_id, upload_id, &manifest, flow)
            .await
            .map_err(FileGrantedDownloadError::Download)
    }

    /// Observe the exact remote manifest, assemble privately and copy to a host-selected new file.
    /// The target is explicit trusted local input, never a remote display name or continuing grant.
    /// One shared whole-download slot follows actual file jobs, including saving after cancellation.
    /// The first failure stops this live flow without retrying, adopting or removing retained files.
    pub async fn fetch_granted_and_save(
        &self,
        token: &str,
        grant_id: FileGrantId,
        upload_id: FileUploadId,
        expected_file: &FileVersionRef,
        target: &FileSaveTarget,
    ) -> Result<SavedFile, FileGrantedSaveError> {
        delivery::run(
            &self.inner,
            token,
            grant_id,
            upload_id,
            expected_file,
            target,
        )
        .await
    }
    /// Observe only this live call's bounded ACK-derived counters, never a receipt or resume state.
    /// Closing the observation does not stop actual already-dispatched file work.
    pub async fn fetch_granted_and_save_observed(
        &self,
        token: &str,
        grant_id: FileGrantId,
        upload_id: FileUploadId,
        expected_file: &FileVersionRef,
        target: &FileSaveTarget,
        observer: FileDeliveryObserver,
    ) -> Result<SavedFile, FileGrantedSaveError> {
        delivery::run_observed(
            &self.inner,
            token,
            grant_id,
            upload_id,
            expected_file,
            target,
            observer,
        )
        .await
    }

    /// Explicit cooperative cancellation of this call's waiting and later scheduling only.
    /// Already-dispatched file jobs keep their original leases and can finish after cancellation.
    /// Invalid input takes priority; cancellation wins when both branches are ready at a poll.
    /// A request cannot preempt synchronous polling or revoke a result already returned.
    pub async fn fetch_granted_and_save_controlled(
        &self,
        token: &str,
        grant_id: FileGrantId,
        upload_id: FileUploadId,
        expected_file: &FileVersionRef,
        target: &FileSaveTarget,
        observer: FileDeliveryObserver,
        cancellation: FileDeliveryCancellation,
    ) -> Result<SavedFile, FileControlledSaveError> {
        controlled::run(
            &self.inner,
            token,
            grant_id,
            upload_id,
            expected_file,
            target,
            observer,
            cancellation,
        )
        .await
    }
}

fn manifest_inputs_valid(inner: &Inner, token: &str, expected_file: &FileVersionRef) -> bool {
    expected_file.workspace() == inner.scope
        && token.len() == 36
        && uuid::Uuid::parse_str(token).is_ok_and(|id| !id.is_nil() && id.to_string() == token)
}

#[cfg(test)]
mod download_test_support;
#[cfg(test)]
mod download_work_tests;
#[cfg(test)]
mod manifest_work_support;
#[cfg(test)]
mod manifest_work_tests;
#[cfg(test)]
mod work_tests;
