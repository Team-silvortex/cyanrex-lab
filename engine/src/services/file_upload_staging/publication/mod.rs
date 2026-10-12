//! Create-only publication coordination. Trusted metadata and current byte checks are not authority.
//! Attempts are caller-owned and cannot be restored, adopted or retried after admission.
use super::*;
use crate::services::{
    file_upload_chunk_journal::{
        ConfirmedFilePin, FileUploadChunkJournal, FileUploadChunkJournalError,
    },
    file_upload_publication::{
        FileUploadPublicationRecord, FileUploadPublicationStore, FileUploadPublicationStoreError,
        PublicationCoordinate, PublicationSchemaPin,
    },
};
mod phase;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileUploadPublicationState {
    Ready,
    Unknown,
    Confirmed,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FileUploadPublicationError {
    #[error("invalid publication input")]
    InvalidInput,
    #[error("publication control capacity exhausted")]
    CapacityExhausted,
    #[error("upload reservation not visible")]
    NotVisible,
    #[error("upload chunks are not fully confirmed")]
    NotReady,
    #[error("upload reservation storage rejected the operation: {0}")]
    Store(FileUploadStoreError),
    #[error("chunk journal rejected the operation: {0}")]
    Journal(FileUploadChunkJournalError),
    #[error("publication storage rejected the operation: {0}")]
    Publication(FileUploadPublicationStoreError),
    #[error("publication bytes unavailable")]
    Unavailable,
    #[error("publication outcome unconfirmed")]
    Unconfirmed,
}

struct PublisherInner {
    local: FileUploadStaging,
    journal: FileUploadChunkJournal,
    publications: FileUploadPublicationStore,
}

#[derive(Clone)]
pub struct FileUploadPublisher {
    inner: Arc<PublisherInner>,
}

pub struct FileUploadPublicationAttempt {
    issuer: Arc<PublisherInner>,
    source: UploadReservationPin,
    key: [u8; 32],
    journal: ConfirmedFilePin,
    publication: PublicationSchemaPin,
    coordinate: PublicationCoordinate,
    state: FileUploadPublicationState,
}

impl FileUploadPublicationAttempt {
    pub fn upload_id(&self) -> FileUploadId {
        self.source.reservation().upload_id()
    }
    pub fn owner(&self) -> PrincipalRef {
        self.source.reservation().owner()
    }
    pub fn account_id(&self) -> LegacyAccountId {
        self.source.reservation().account_id()
    }
    pub fn manifest(&self) -> &FileTransferManifest {
        self.source.reservation().manifest()
    }
    pub fn state(&self) -> FileUploadPublicationState {
        self.state
    }
}

/// An in-process publication protocol result, never a download grant or permanent health claim.
pub struct FileUploadPublicationReceipt {
    record: FileUploadPublicationRecord,
}

impl FileUploadPublicationReceipt {
    pub fn upload_id(&self) -> FileUploadId {
        self.record.upload_id()
    }
    pub fn owner(&self) -> PrincipalRef {
        self.record.owner()
    }
    pub fn account_id(&self) -> LegacyAccountId {
        self.record.account_id()
    }
    pub fn manifest(&self) -> &FileTransferManifest {
        self.record.manifest()
    }
}

impl fmt::Debug for FileUploadPublicationReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileUploadPublicationReceipt")
            .field("record", &self.record)
            .finish()
    }
}

impl FileUploadPublisher {
    /// Freezes three trusted handles. Namespaces must be distinct; construction performs no I/O.
    pub fn new(
        local: FileUploadStaging,
        journal: FileUploadChunkJournal,
        publications: FileUploadPublicationStore,
    ) -> Result<Self, FileUploadPublicationError> {
        let store = &local.inner.store;
        if store.scope() != journal.scope()
            || store.scope() != publications.scope()
            || store.namespace() == journal.namespace()
            || store.namespace() == publications.namespace()
            || journal.namespace() == publications.namespace()
        {
            return Err(FileUploadPublicationError::InvalidInput);
        }
        Ok(Self {
            inner: Arc::new(PublisherInner {
                local,
                journal,
                publications,
            }),
        })
    }

    /// Captures original metadata identities only; reads no file and writes no publication.
    pub async fn prepare(
        &self,
        upload_id: FileUploadId,
        owner: PrincipalRef,
        account_id: LegacyAccountId,
    ) -> Result<FileUploadPublicationAttempt, FileUploadPublicationError> {
        let _permit = self
            .inner
            .local
            .admit()
            .map_err(|_| FileUploadPublicationError::CapacityExhausted)?;
        let source = self
            .inner
            .local
            .inner
            .store
            .capture_pin(upload_id, owner, account_id)
            .await
            .map_err(FileUploadPublicationError::Store)?
            .ok_or(FileUploadPublicationError::NotVisible)?;
        let key = self
            .inner
            .local
            .inner
            .staging
            .bind_upload(source.source_key());
        let journal = self
            .inner
            .journal
            .capture_confirmed_file(&source, key)
            .await
            .map_err(FileUploadPublicationError::Journal)?
            .ok_or(FileUploadPublicationError::NotReady)?;
        let publication = self
            .inner
            .publications
            .capture_pin()
            .await
            .map_err(FileUploadPublicationError::Publication)?;
        let coordinate = PublicationCoordinate::new(&source, key, journal.journal_key())
            .map_err(FileUploadPublicationError::Publication)?;
        Ok(FileUploadPublicationAttempt {
            issuer: Arc::clone(&self.inner),
            source,
            key,
            journal,
            publication,
            coordinate,
            state: FileUploadPublicationState::Ready,
        })
    }

    /// Reads fresh whole bytes, then acknowledges a create-only metadata publication. No SQL lock
    /// spans file work, and a committed row may remain when a later observation leaves Unknown.
    /// This result is historical protocol confirmation, never current health or download authority.
    pub async fn publish(
        &self,
        attempt: &mut FileUploadPublicationAttempt,
    ) -> Result<FileUploadPublicationReceipt, FileUploadPublicationError> {
        let run = self.start(attempt)?;
        self.publish_trusted(run).await
    }
}

fn initial_filesystem_error(error: ChunkStagingError) -> FileUploadPublicationError {
    match error {
        ChunkStagingError::CapacityExhausted => FileUploadPublicationError::CapacityExhausted,
        ChunkStagingError::Unconfirmed => FileUploadPublicationError::Unconfirmed,
        _ => FileUploadPublicationError::Unavailable,
    }
}

fn publication_error(error: FileUploadPublicationStoreError) -> FileUploadPublicationError {
    match error {
        FileUploadPublicationStoreError::StorageUnavailable => {
            FileUploadPublicationError::Unconfirmed
        }
        error => FileUploadPublicationError::Publication(error),
    }
}
