//! Explicit single-chunk request preparation, never application mounting or download authority.
use crate::{
    models::collaboration::{FileGrantId, FileTransferManifest, FileUploadId, WorkspaceRef},
    services::{
        file_chunk_receiver::{FileChunkReceiveError, FileChunkReceiver, ReceivedFileChunk},
        file_transfer::staging::ChunkStaging,
    },
};
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;

mod policy;
mod request;

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
    scope: WorkspaceRef,
    receiver: FileChunkReceiver,
    http: reqwest::Client,
    controls: Arc<Semaphore>,
    header_timeout: Duration,
    #[cfg(test)]
    eof_observer: Option<Arc<std::sync::atomic::AtomicBool>>,
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
        let scope = staging.scope();
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
                scope,
                receiver,
                http,
                controls: Arc::new(Semaphore::new(config.capacity)),
                header_timeout: config.header_timeout,
                #[cfg(test)]
                eof_observer: None,
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
}

#[cfg(test)]
mod work_tests;
