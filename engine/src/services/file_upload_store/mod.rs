//! Trusted upload-intent reservation only, never authentication or confirmed chunk progress.
//!
//! A trusted caller supplies the Principal and exact account incarnation. Retained intents occupy
//! quotas indefinitely; reads never release, adopt, resume or authorize a transfer. A missing commit
//! acknowledgement may race a committed reservation. No failure proves rollback or permits retry.
use super::prepared_resource_sql;
use crate::{
    models::collaboration::*,
    sqlx_compat::{self as sqlx, PgPool, Postgres, Row},
};
use sqlx_core::transaction::Transaction;
use uuid::Uuid;

mod commands;
mod ledger;
mod namespace;
#[cfg(unix)]
mod pins;
mod records;
mod schema;
mod transaction;

#[cfg(unix)]
pub(crate) use pins::UploadReservationPin;
#[cfg(unix)]
pub(crate) use transaction::PendingFileUpload;

pub const MAX_FILE_UPLOAD_INTENTS: u32 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FileUploadStoreError {
    #[error("upload storage did not confirm the operation")]
    StorageUnavailable,
    #[error("unsupported upload schema")]
    UnsupportedSchema,
    #[error("upload namespace is not empty")]
    NamespaceNotEmpty,
    #[error("upload scope does not match")]
    ScopeMismatch,
    #[error("upload installation does not match")]
    IncarnationMismatch,
    #[error("invalid upload input")]
    InvalidInput,
    #[error("invalid persisted upload")]
    InvalidRecord,
    #[error("upload already exists")]
    Conflict,
    #[error("upload quota exceeded")]
    QuotaExceeded,
}
type Result<T> = std::result::Result<T, FileUploadStoreError>;
type Tx<'a> = Transaction<'a, Postgres>;

impl From<sqlx::Error> for FileUploadStoreError {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileUploadLimits {
    total_bytes: u64,
    owner_bytes: u64,
    total_uploads: u32,
    owner_uploads: u32,
}
impl FileUploadLimits {
    pub fn new(
        total_bytes: u64,
        owner_bytes: u64,
        total_uploads: u32,
        owner_uploads: u32,
    ) -> Result<Self> {
        if owner_bytes > total_bytes
            || total_bytes > MAX_FILE_TRANSFER_BYTES * u64::from(MAX_FILE_UPLOAD_INTENTS)
            || owner_uploads == 0
            || owner_uploads > total_uploads
            || total_uploads > MAX_FILE_UPLOAD_INTENTS
        {
            return Err(FileUploadStoreError::InvalidInput);
        }
        Ok(Self {
            total_bytes,
            owner_bytes,
            total_uploads,
            owner_uploads,
        })
    }
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }
    pub fn owner_bytes(&self) -> u64 {
        self.owner_bytes
    }
    pub fn total_uploads(&self) -> u32 {
        self.total_uploads
    }
    pub fn owner_uploads(&self) -> u32 {
        self.owner_uploads
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileUploadReservation {
    upload_id: FileUploadId,
    owner: PrincipalRef,
    account_id: LegacyAccountId,
    manifest: FileTransferManifest,
}
impl FileUploadReservation {
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

#[derive(Clone)]
pub struct FileUploadStore {
    pool: PgPool,
    scope: WorkspaceRef,
    installation_id: Uuid,
    namespace: String,
    limits: FileUploadLimits,
}

impl FileUploadStore {
    #[cfg(unix)]
    pub(crate) fn namespace(&self) -> &str {
        &self.namespace
    }

    #[cfg(unix)]
    pub(crate) fn scope(&self) -> WorkspaceRef {
        self.scope
    }

    /// Freezes trusted configuration without connection, installation or ambient state adoption.
    pub fn new(
        pool: PgPool,
        scope: WorkspaceRef,
        installation_id: Uuid,
        namespace: impl Into<String>,
        limits: FileUploadLimits,
    ) -> Result<Self> {
        let namespace = namespace.into();
        if installation_id.get_version() != Some(uuid::Version::Random)
            || installation_id.get_variant() != uuid::Variant::RFC4122
            || !namespace::valid_name(&namespace)
        {
            return Err(FileUploadStoreError::InvalidInput);
        }
        Ok(Self {
            pool,
            scope,
            installation_id,
            namespace,
            limits,
        })
    }

    fn check_owner(&self, owner: PrincipalRef) -> Result<()> {
        if owner.authority_id != self.scope.authority_id {
            return Err(FileUploadStoreError::ScopeMismatch);
        }
        Ok(())
    }
}

async fn bounded<T>(operation: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    tokio::time::timeout(std::time::Duration::from_secs(10), operation)
        .await
        .map_err(|_| FileUploadStoreError::StorageUnavailable)?
}

fn confirmed_one(rows: u64) -> Result<()> {
    if rows != 1 {
        return Err(FileUploadStoreError::StorageUnavailable);
    }
    Ok(())
}
