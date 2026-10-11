//! Explicit trusted chunk journal preparation. Metadata reads are not receipts or recovery grants.
//!
//! Unknown and Confirmed are retained historical metadata, not current file health or authority.
//! Source/filesystem observations remain separate transactions; no SQL lock spans file work.
//! Every occupied upload/index conflicts, even with matching bytes, bindings or reported state.
use super::prepared_resource_sql;
use crate::{
    models::collaboration::*,
    services::file_upload_store::UploadReservationPin,
    sqlx_compat::{self as sqlx, PgPool, Postgres, Row},
};
use sqlx_core::transaction::Transaction;
use uuid::Uuid;

mod commands;
mod ledger;
mod namespace;
mod publication_pin;
mod records;
mod schema;
pub(crate) use publication_pin::ConfirmedFilePin;
pub(crate) use records::UnknownChunk;
pub(crate) use schema::JournalSchemaPin;

pub const MAX_FILE_UPLOAD_CHUNK_ENTRIES: u32 = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistentChunkState {
    Unknown,
    Confirmed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadChunkRecord {
    upload_id: FileUploadId,
    owner: PrincipalRef,
    account_id: LegacyAccountId,
    index: u32,
    byte_length: u64,
    sha256: Sha256Digest,
    state: PersistentChunkState,
}

impl UploadChunkRecord {
    pub fn upload_id(&self) -> FileUploadId {
        self.upload_id
    }
    pub fn owner(&self) -> PrincipalRef {
        self.owner
    }
    pub fn account_id(&self) -> LegacyAccountId {
        self.account_id
    }
    pub fn chunk_index(&self) -> u32 {
        self.index
    }
    pub fn byte_length(&self) -> u64 {
        self.byte_length
    }
    pub fn sha256(&self) -> &Sha256Digest {
        &self.sha256
    }
    pub fn state(&self) -> PersistentChunkState {
        self.state
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FileUploadChunkJournalError {
    #[error("chunk journal did not confirm the operation")]
    StorageUnavailable,
    #[error("unsupported chunk journal schema")]
    UnsupportedSchema,
    #[error("chunk journal namespace is not empty")]
    NamespaceNotEmpty,
    #[error("chunk journal scope does not match")]
    ScopeMismatch,
    #[error("chunk journal installation does not match")]
    IncarnationMismatch,
    #[error("invalid chunk journal input")]
    InvalidInput,
    #[error("invalid persisted chunk journal record")]
    InvalidRecord,
    #[error("chunk coordinate already exists")]
    Conflict,
    #[error("chunk journal capacity exhausted")]
    CapacityExhausted,
}

type Result<T> = std::result::Result<T, FileUploadChunkJournalError>;
type Tx<'a> = Transaction<'a, Postgres>;

impl From<sqlx::Error> for FileUploadChunkJournalError {
    fn from(error: sqlx::Error) -> Self {
        if error
            .as_database_error()
            .is_some_and(|error| error.is_unique_violation())
        {
            Self::Conflict
        } else {
            Self::StorageUnavailable
        }
    }
}

#[derive(Clone)]
pub struct FileUploadChunkJournal {
    pool: PgPool,
    scope: WorkspaceRef,
    installation_id: Uuid,
    namespace: String,
    max_entries: u32,
}

/// Inert original-source coordinates. Not a reservation acknowledgement or a retry permission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChunkCoordinate {
    scope: WorkspaceRef,
    record: UploadChunkRecord,
    source_key: [u8; 32],
    root_key: [u8; 32],
    total_length: u64,
}

impl ChunkCoordinate {
    pub(crate) fn new(pin: &UploadReservationPin, root_key: [u8; 32], index: u32) -> Result<Self> {
        let reservation = pin.reservation();
        let manifest = reservation.manifest();
        let chunk = manifest
            .chunk(index)
            .map_err(|_| FileUploadChunkJournalError::InvalidInput)?;
        Ok(Self {
            scope: manifest.workspace(),
            record: UploadChunkRecord {
                upload_id: reservation.upload_id(),
                owner: reservation.owner(),
                account_id: reservation.account_id(),
                index,
                byte_length: chunk.byte_length,
                sha256: chunk.sha256,
                state: PersistentChunkState::Unknown,
            },
            source_key: pin.source_key(),
            root_key,
            total_length: manifest.byte_length(),
        })
    }
}

impl FileUploadChunkJournal {
    pub fn new(
        pool: PgPool,
        scope: WorkspaceRef,
        installation_id: Uuid,
        namespace: impl Into<String>,
        max_entries: u32,
    ) -> Result<Self> {
        let namespace = namespace.into();
        if !(1..=MAX_FILE_UPLOAD_CHUNK_ENTRIES).contains(&max_entries)
            || installation_id.get_version() != Some(uuid::Version::Random)
            || installation_id.get_variant() != uuid::Variant::RFC4122
            || !namespace::valid_name(&namespace)
        {
            return Err(FileUploadChunkJournalError::InvalidInput);
        }
        Ok(Self {
            pool,
            scope,
            installation_id,
            namespace,
            max_entries,
        })
    }

    pub(crate) fn scope(&self) -> WorkspaceRef {
        self.scope
    }
    pub(crate) fn namespace(&self) -> &str {
        &self.namespace
    }

    fn check_input(&self, owner: PrincipalRef, index: u32) -> Result<()> {
        if owner.authority_id != self.scope.authority_id {
            return Err(FileUploadChunkJournalError::ScopeMismatch);
        }
        if index as usize >= MAX_FILE_TRANSFER_CHUNKS {
            return Err(FileUploadChunkJournalError::InvalidInput);
        }
        Ok(())
    }
}

async fn bounded<T>(operation: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    tokio::time::timeout(std::time::Duration::from_secs(10), operation)
        .await
        .map_err(|_| FileUploadChunkJournalError::StorageUnavailable)?
}

fn confirmed_one(rows: u64) -> Result<()> {
    if rows != 1 {
        return Err(FileUploadChunkJournalError::StorageUnavailable);
    }
    Ok(())
}
