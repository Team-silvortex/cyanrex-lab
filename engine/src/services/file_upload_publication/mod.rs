//! Explicit trusted immutable publication metadata, never download authority or recovered proof.
//!
//! Construction installs nothing. Metadata reads cannot recreate a publication acknowledgement,
//! and uncertainty never permits overwrite, retry, adoption, migration or automatic cleanup.
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
mod records;
mod schema;
mod transaction;
pub(crate) use schema::PublicationSchemaPin;
pub(crate) use transaction::PendingFileUploadPublicationRead;

pub const MAX_FILE_UPLOAD_PUBLICATIONS: u32 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FileUploadPublicationStoreError {
    #[error("publication storage did not confirm the operation")]
    StorageUnavailable,
    #[error("unsupported publication schema")]
    UnsupportedSchema,
    #[error("publication namespace is not empty")]
    NamespaceNotEmpty,
    #[error("publication scope does not match")]
    ScopeMismatch,
    #[error("publication installation does not match")]
    IncarnationMismatch,
    #[error("invalid publication input")]
    InvalidInput,
    #[error("invalid persisted publication record")]
    InvalidRecord,
    #[error("publication coordinate already exists")]
    Conflict,
    #[error("publication capacity exhausted")]
    CapacityExhausted,
}

type Result<T> = std::result::Result<T, FileUploadPublicationStoreError>;
type Tx<'a> = Transaction<'a, Postgres>;

impl From<sqlx::Error> for FileUploadPublicationStoreError {
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
pub struct FileUploadPublicationStore {
    pool: PgPool,
    scope: WorkspaceRef,
    installation_id: Uuid,
    namespace: String,
    max_publications: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileUploadPublicationRecord {
    upload_id: FileUploadId,
    owner: PrincipalRef,
    account_id: LegacyAccountId,
    manifest: FileTransferManifest,
}

impl FileUploadPublicationRecord {
    pub fn upload_id(&self) -> FileUploadId {
        self.upload_id
    }
    pub fn owner(&self) -> PrincipalRef {
        self.owner
    }
    pub fn account_id(&self) -> LegacyAccountId {
        self.account_id
    }
    pub fn manifest(&self) -> &FileTransferManifest {
        &self.manifest
    }
}

/// Inert original source/root/journal binding, not a commit acknowledgement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PublicationCoordinate {
    record: FileUploadPublicationRecord,
    source_key: [u8; 32],
    root_key: [u8; 32],
    journal_key: [u8; 32],
}

impl PublicationCoordinate {
    pub(crate) fn new(
        pin: &UploadReservationPin,
        root_key: [u8; 32],
        journal_key: [u8; 32],
    ) -> Result<Self> {
        let reservation = pin.reservation();
        Ok(Self {
            record: FileUploadPublicationRecord {
                upload_id: reservation.upload_id(),
                owner: reservation.owner(),
                account_id: reservation.account_id(),
                manifest: reservation.manifest().clone(),
            },
            source_key: pin.source_key(),
            root_key,
            journal_key,
        })
    }
}

/// Move-only acknowledgement. A matching metadata read cannot manufacture this private token.
pub(crate) struct PublicationAcknowledgement {
    schema: PublicationSchemaPin,
    expected: records::PublishedRow,
    fingerprint: [u8; 32],
}

impl PublicationAcknowledgement {
    pub(crate) fn record(&self) -> &FileUploadPublicationRecord {
        &self.expected.coordinate.record
    }
}

impl FileUploadPublicationStore {
    pub fn new(
        pool: PgPool,
        scope: WorkspaceRef,
        installation_id: Uuid,
        namespace: impl Into<String>,
        max_publications: u32,
    ) -> Result<Self> {
        let namespace = namespace.into();
        if !(1..=MAX_FILE_UPLOAD_PUBLICATIONS).contains(&max_publications)
            || installation_id.get_version() != Some(uuid::Version::Random)
            || installation_id.get_variant() != uuid::Variant::RFC4122
            || !namespace::valid_name(&namespace)
        {
            return Err(FileUploadPublicationStoreError::InvalidInput);
        }
        Ok(Self {
            pool,
            scope,
            installation_id,
            namespace,
            max_publications,
        })
    }

    pub(crate) fn scope(&self) -> WorkspaceRef {
        self.scope
    }
    pub(crate) fn namespace(&self) -> &str {
        &self.namespace
    }
}

async fn bounded<T>(operation: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    tokio::time::timeout(std::time::Duration::from_secs(10), operation)
        .await
        .map_err(|_| FileUploadPublicationStoreError::StorageUnavailable)?
}

fn confirmed_one(rows: u64) -> Result<()> {
    if rows != 1 {
        return Err(FileUploadPublicationStoreError::StorageUnavailable);
    }
    Ok(())
}
