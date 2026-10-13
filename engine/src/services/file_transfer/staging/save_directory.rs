//! Output-only policy. Private staging and the original private save terminal remain unchanged.
use super::{
    directory::ChunkDirectory,
    filesystem::{invalid, open_directory_path, uid, Identity},
};
use std::{
    fs::{File, Metadata},
    io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

pub(super) enum SaveDirectory {
    Private(ChunkDirectory),
    User(UserSaveDirectory),
}

impl SaveDirectory {
    pub(super) fn open_private(path: &Path) -> io::Result<Self> {
        ChunkDirectory::open(path).map(Self::Private)
    }

    pub(super) fn open_user(path: &Path) -> io::Result<Self> {
        let directory = open_directory_path(path)?;
        let original = SecuritySnapshot::of(&directory.metadata()?)?;
        Ok(Self::User(UserSaveDirectory {
            path: path.to_owned(),
            directory,
            original,
        }))
    }

    pub(super) fn fd(&self) -> &File {
        match self {
            Self::Private(directory) => &directory.directory,
            Self::User(directory) => &directory.directory,
        }
    }

    pub(super) fn identity(&self) -> Identity {
        match self {
            Self::Private(directory) => directory.identity,
            Self::User(directory) => directory.original.identity,
        }
    }

    pub(super) fn verify_root(&self) -> io::Result<()> {
        match self {
            Self::Private(directory) => directory.verify_root(),
            Self::User(directory) => directory.verify_root(),
        }
    }
}

pub(super) struct UserSaveDirectory {
    path: PathBuf,
    directory: File,
    original: SecuritySnapshot,
}

impl UserSaveDirectory {
    fn verify_root(&self) -> io::Result<()> {
        let before = SecuritySnapshot::of(&self.directory.metadata()?)?;
        let current = open_directory_path(&self.path)?;
        let visible = SecuritySnapshot::of(&current.metadata()?)?;
        let after = SecuritySnapshot::of(&self.directory.metadata()?)?;
        if before != self.original || visible != self.original || after != self.original {
            return Err(invalid());
        }
        Ok(())
    }
}

/// Only fixed security fields: creating our regular output changes directory entry timestamps.
#[derive(Clone, Copy, PartialEq, Eq)]
struct SecuritySnapshot {
    identity: Identity,
    uid: u32,
    gid: u32,
    mode: u32,
}

impl SecuritySnapshot {
    fn of(metadata: &Metadata) -> io::Result<Self> {
        let mode = metadata.mode() & 0o7777;
        if !metadata.is_dir() || metadata.uid() != uid() || !matches!(mode, 0o700 | 0o750 | 0o755) {
            return Err(invalid());
        }
        Ok(Self {
            identity: Identity::of(metadata),
            uid: metadata.uid(),
            gid: metadata.gid(),
            mode,
        })
    }
}

#[cfg(test)]
#[path = "save_directory_tests.rs"]
mod tests;
