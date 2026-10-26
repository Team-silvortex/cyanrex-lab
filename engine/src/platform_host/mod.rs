//! Explicit trusted host configuration and independent preparation gates.
//! No Engine, listener, installer or public Session issuer is constructed here.

mod artifact_root;
mod catalog;
mod config;
mod database_connection;
mod login_connection;
#[cfg(target_os = "linux")]
mod process_startup;
mod session_connection;
mod session_pair;
mod storage_check;
mod storage_connection;
mod storage_markers;
mod storage_open;
mod task_content_connection;
mod task_create_connection;
mod task_index_connection;
mod task_replace_connection;
mod text_artifact_connection;

pub use artifact_root::{
    inspect_platform_artifact_root, verify_platform_artifact_root, PlatformArtifactRootObservation,
};
pub use catalog::{
    inspect_platform_database, inspect_platform_database_expected, verify_platform_database,
    PlatformCatalogExpectation, PlatformCatalogObservation,
};
pub use config::{PlatformDatabaseTarget, PlatformHostPreflightConfig, PlatformNamespaces};
pub use database_connection::{
    PlatformDatabaseConnectionError, PlatformDatabaseConnector, PlatformDatabaseEndpoint,
};
pub use login_connection::PlatformLoginConnector;
#[cfg(target_os = "linux")]
pub use process_startup::{
    prepare_platform_process, PlatformProcessIdentity, PlatformProcessStartup,
    PlatformProcessStartupError,
};
pub use session_connection::PlatformSessionConnector;
pub(crate) use session_pair::{
    split_platform_session_connectors, split_platform_task_replace_connectors,
    split_platform_text_publication_connectors,
};
pub use storage_check::check_platform_database_storage;
pub use storage_check::check_platform_database_storage_expected;
pub use storage_check::check_platform_database_storage_markers_expected;
pub use storage_connection::{PlatformStorageConnectionError, PlatformStorageConnector};
pub use storage_markers::{
    inspect_platform_storage_markers, inspect_platform_storage_markers_expected,
    verify_platform_storage_markers, PlatformStorageMarkerExpectation,
    PlatformStorageMarkerObservation,
};
pub use storage_open::open_platform_storage_expected;
pub use task_content_connection::PlatformTaskContentConnector;
pub use task_create_connection::PlatformTaskCreateConnector;
pub use task_index_connection::PlatformTaskIndexConnector;
pub use task_replace_connection::PlatformTaskReplaceConnector;
pub use text_artifact_connection::PlatformTextArtifactConnector;

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
