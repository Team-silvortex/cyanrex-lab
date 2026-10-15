//! Explicit trusted host configuration and independent read-only preparation gates.
//! No Engine, listener, installer, credential, Session or runtime authority is constructed here.

mod artifact_root;
mod catalog;
mod config;
mod storage_markers;

pub use artifact_root::{
    inspect_platform_artifact_root, verify_platform_artifact_root, PlatformArtifactRootObservation,
};
pub use catalog::{
    inspect_platform_database, verify_platform_database, PlatformCatalogObservation,
};
pub use config::{PlatformDatabaseTarget, PlatformHostPreflightConfig, PlatformNamespaces};
pub use storage_markers::{
    inspect_platform_storage_markers, verify_platform_storage_markers,
    PlatformStorageMarkerObservation,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PlatformHostPreflightError {
    #[error("invalid platform host configuration")]
    InvalidConfig,
    #[error("invalid platform host origin")]
    InvalidOrigin,
    #[error("invalid platform host namespace")]
    InvalidNamespace,
    #[error("platform database target does not match")]
    TargetMismatch,
    #[error("platform database role is unsafe")]
    UnsafeRole,
    #[error("platform database namespace is unsafe")]
    UnsafeNamespace,
    #[error("platform database catalog changed")]
    CatalogChanged,
    #[error("platform database observation is unavailable")]
    StorageUnavailable,
    #[error("platform artifact root observation is unavailable")]
    ArtifactRootUnavailable,
    #[error("platform artifact root changed")]
    ArtifactRootChanged,
    #[error("unsupported platform storage marker")]
    UnsupportedStorageMarker,
    #[error("platform storage marker scope does not match")]
    StorageScopeMismatch,
    #[error("platform storage markers changed")]
    StorageMarkersChanged,
}

type Result<T> = std::result::Result<T, PlatformHostPreflightError>;

impl From<crate::sqlx_compat::Error> for PlatformHostPreflightError {
    fn from(_: crate::sqlx_compat::Error) -> Self {
        Self::StorageUnavailable
    }
}
