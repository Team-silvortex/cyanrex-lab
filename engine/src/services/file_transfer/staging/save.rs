//! Explicit trusted-host destination selection, not remote permission or a UI save dialog.
use super::{
    binding::manifest_digest, save_directory::SaveDirectory, work_save, AssembledFile,
    FileTransferManifest, Inner,
};
use crate::models::collaboration::FileVersionRef;
use std::{path::Path, sync::Arc};
use tokio::sync::OwnedSemaphorePermit;

/// One original owned parent and explicitly selected basename. Opens no output file.
/// Clones share the original directory descriptor; no path or mode comes from remote metadata.
#[derive(Clone)]
pub struct FileSaveTarget {
    inner: Arc<Target>,
}

pub(super) struct Target {
    pub(super) directory: SaveDirectory,
    pub(super) leaf: String,
    #[cfg(test)]
    pub(super) observer: Option<Arc<dyn Fn(SaveTestPhase) + Send + Sync>>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum FileSaveError {
    #[error("invalid file save input")]
    InvalidInput,
    #[error("file save capacity exhausted")]
    CapacityExhausted,
    #[error("file save target already exists")]
    Conflict,
    #[error("file save unavailable")]
    Unavailable,
    #[error("file save outcome unconfirmed")]
    Unconfirmed,
}

impl FileSaveTarget {
    /// Opens an existing owned 0700 directory through the no-follow trusted ancestor walk.
    /// Neither creates/chmods a parent nor reserves, adopts or overwrites an output entry.
    pub fn open(parent: &Path, leaf: &str) -> Result<Self, FileSaveError> {
        validate_leaf(leaf)?;
        let directory =
            SaveDirectory::open_private(parent).map_err(|_| FileSaveError::Unavailable)?;
        Ok(Self {
            inner: Arc::new(Target {
                directory,
                leaf: leaf.to_owned(),
                #[cfg(test)]
                observer: None,
            }),
        })
    }

    /// Explicit host opt-in for an owned ordinary 0700/0750/0755 parent, never remote authority.
    /// Pins its original identity/UID/GID/exact mode, not directory entry timestamps or ACL semantics.
    pub fn open_user_directory(parent: &Path, leaf: &str) -> Result<Self, FileSaveError> {
        validate_leaf(leaf)?;
        let directory = SaveDirectory::open_user(parent).map_err(|_| FileSaveError::Unavailable)?;
        Ok(Self {
            inner: Arc::new(Target {
                directory,
                leaf: leaf.to_owned(),
                #[cfg(test)]
                observer: None,
            }),
        })
    }
}

/// Move-only metadata acknowledging this local selected-file save, not durable remote authority.
/// No pathname, descriptor, byte extraction or supplied-metadata construction exists.
///
/// ```compile_fail
/// use cyanrex_engine::services::file_transfer::staging::SavedFile;
/// fn duplicate(file: &SavedFile) -> SavedFile { file.clone() }
/// ```
///
/// ```compile_fail
/// use cyanrex_engine::services::file_transfer::staging::SavedFile;
/// fn serialize(file: &SavedFile) { let _ = serde_json::to_vec(file); }
/// ```
///
/// ```compile_fail
/// use cyanrex_engine::models::collaboration::FileVersionRef;
/// use cyanrex_engine::services::file_transfer::staging::SavedFile;
/// fn manufacture(file: FileVersionRef) -> SavedFile { SavedFile { file } }
/// ```
pub struct SavedFile {
    file: FileVersionRef,
}

impl SavedFile {
    pub fn file(&self) -> &FileVersionRef {
        &self.file
    }

    pub(super) fn confirmed(manifest: &FileTransferManifest) -> Self {
        Self {
            file: FileVersionRef::from_manifest(manifest),
        }
    }
}

pub(super) async fn run(
    inner: &Arc<Inner>,
    manifest: &FileTransferManifest,
    assembled: &AssembledFile,
    target: &FileSaveTarget,
    flow: Option<Arc<OwnedSemaphorePermit>>,
) -> Result<SavedFile, FileSaveError> {
    if manifest.workspace() != inner.scope
        || assembled.file() != &FileVersionRef::from_manifest(manifest)
    {
        return Err(FileSaveError::InvalidInput);
    }
    validate_target(inner, target)?;
    let binding = manifest_digest(manifest).map_err(|_| FileSaveError::InvalidInput)?;
    if !assembled.proof().matches(inner.directory.identity, binding) {
        return Err(FileSaveError::InvalidInput);
    }
    work_save::run(inner, manifest, assembled.proof(), &target.inner, flow).await
}

pub(super) fn validate_target(inner: &Inner, target: &FileSaveTarget) -> Result<(), FileSaveError> {
    if target.inner.directory.identity() == inner.directory.identity {
        return Err(FileSaveError::InvalidInput);
    }
    Ok(())
}

fn validate_leaf(leaf: &str) -> Result<(), FileSaveError> {
    if leaf.is_empty()
        || leaf.len() > 255
        || matches!(leaf, "." | "..")
        || leaf
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\'))
    {
        return Err(FileSaveError::InvalidInput);
    }
    Ok(())
}

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SaveTestPhase {
    BeforeDispatch,
    Created,
    Copied,
    Readback,
}

#[cfg(test)]
impl FileSaveTarget {
    pub(crate) fn set_test_observer(&mut self, observer: Arc<dyn Fn(SaveTestPhase) + Send + Sync>) {
        Arc::get_mut(&mut self.inner).unwrap().observer = Some(observer);
    }
}

#[cfg(test)]
#[path = "save_drift_tests.rs"]
mod drift_tests;
#[cfg(test)]
#[path = "save_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "save_user_tests.rs"]
mod user_tests;
#[cfg(test)]
#[path = "save_work_tests.rs"]
mod work_tests;
