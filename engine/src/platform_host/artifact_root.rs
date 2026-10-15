use super::{PlatformHostPreflightConfig, PlatformHostPreflightError, Result};
use crate::services::artifact_store::open_private_artifact_root;
use std::{ffi::OsString, fs::File, os::unix::fs::MetadataExt, path::Path};

#[derive(Clone, Copy, PartialEq, Eq)]
struct RootIdentity {
    device: u64,
    inode: u64,
    uid: u32,
    gid: u32,
    mode: u32,
}

impl RootIdentity {
    fn read(file: &File) -> std::result::Result<Self, ()> {
        let metadata = file.metadata().map_err(|_| ())?;
        // SAFETY: geteuid has no arguments or memory preconditions.
        let uid = unsafe { libc::geteuid() };
        let mode = metadata.mode();
        if !metadata.is_dir() || metadata.uid() != uid || mode & 0o7777 != 0o700 {
            return Err(());
        }
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            uid: metadata.uid(),
            gid: metadata.gid(),
            mode,
        })
    }
}

/// Opaque root observation, not public filesystem access, authority or content-health evidence.
pub struct PlatformArtifactRootObservation {
    config: PlatformHostPreflightConfig,
    path: OsString,
    file: File,
    identity: RootIdentity,
}

impl PlatformArtifactRootObservation {
    pub(super) fn binds_path(&self, config: &PlatformHostPreflightConfig, path: &Path) -> bool {
        self.config == *config && path.as_os_str() == self.path.as_os_str()
    }

    /// Passive comparison only; no path is opened or revalidated by this method.
    pub fn matches(&self, other: &Self) -> bool {
        self.config == other.config && self.path == other.path && self.identity == other.identity
    }
}

/// Open an existing owned private root with the Artifact store's original no-follow ancestor walk.
/// The original descriptor stays private and live; no content is enumerated or read.
/// Synchronous metadata I/O has no hard deadline and does not isolate the same UID or a root actor.
pub fn inspect_platform_artifact_root(
    config: &PlatformHostPreflightConfig,
    root: &Path,
) -> Result<PlatformArtifactRootObservation> {
    let unavailable = PlatformHostPreflightError::ArtifactRootUnavailable;
    let file = open_private_artifact_root(root).map_err(|_| unavailable)?;
    let identity = RootIdentity::read(&file).map_err(|_| unavailable)?;
    let observation = PlatformArtifactRootObservation {
        config: config.clone(),
        path: root.as_os_str().to_owned(),
        file,
        identity,
    };
    recheck(root, &observation).map_err(|_| unavailable)?;
    Ok(observation)
}

/// Recheck the original root; never adopt a replacement or infer current content integrity.
pub fn verify_platform_artifact_root(
    config: &PlatformHostPreflightConfig,
    root: &Path,
    expected: &PlatformArtifactRootObservation,
) -> Result<()> {
    let changed = PlatformHostPreflightError::ArtifactRootChanged;
    // Path equality is raw OsStr equality, never Path's component-normalizing comparison.
    if config != &expected.config || root.as_os_str() != expected.path.as_os_str() {
        return Err(changed);
    }
    recheck(root, expected).map_err(|_| changed)
}

fn recheck(root: &Path, expected: &PlatformArtifactRootObservation) -> std::result::Result<(), ()> {
    if RootIdentity::read(&expected.file)? != expected.identity {
        return Err(());
    }
    let visible = open_private_artifact_root(root).map_err(|_| ())?;
    if RootIdentity::read(&visible)? != expected.identity
        || RootIdentity::read(&expected.file)? != expected.identity
    {
        return Err(());
    }
    // Changes to directory contents/timestamps/link count are not security identity changes.
    Ok(())
}
