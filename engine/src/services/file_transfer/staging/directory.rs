use super::super::{copy_chunk_checked, verify_chunk};
use super::{binding::chunk_name, filesystem::*, ChunkStagingError, FileTransferManifest};
use std::{
    ffi::OsStr,
    fs::{File, Permissions},
    io::{self, Read},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

pub(super) struct ChunkDirectory {
    path: PathBuf,
    pub(super) directory: File,
    pub(super) identity: Identity,
}

impl ChunkDirectory {
    pub(super) fn open(path: &Path) -> io::Result<Self> {
        let directory = open_root(path)?;
        let identity = Identity::of(&directory.metadata()?);
        Ok(Self {
            path: path.to_owned(),
            directory,
            identity,
        })
    }

    pub(super) fn bind_upload(&self, source_key: [u8; 32]) -> [u8; 32] {
        self.identity.bind_upload(source_key)
    }

    pub(super) fn verify_root(&self) -> io::Result<()> {
        let visible = open_root(&self.path)?;
        if Identity::of(&visible.metadata()?) != Identity::of(&self.directory.metadata()?) {
            return Err(invalid());
        }
        Ok(())
    }

    fn check_entry(
        &self,
        manifest: &FileTransferManifest,
        index: u32,
        name: &OsStr,
        expected: Option<&Snapshot>,
    ) -> io::Result<File> {
        self.verify_root()?;
        let spec = manifest.chunk(index).map_err(|_| invalid())?;
        // O_NONBLOCK prevents a substituted FIFO from waiting before its metadata is rejected.
        let mut file = open_at(&self.directory, name, libc::O_RDONLY | libc::O_NONBLOCK, 0)?;
        let before = Snapshot::private(&file.metadata()?, spec.byte_length)?;
        if expected.is_some_and(|snapshot| before != *snapshot) {
            return Err(invalid());
        }
        verify_chunk(manifest, index, &mut file).map_err(|_| invalid())?;
        let after = Snapshot::private(&file.metadata()?, spec.byte_length)?;
        let visible = open_at(&self.directory, name, libc::O_RDONLY | libc::O_NONBLOCK, 0)?;
        let visible = Snapshot::private(&visible.metadata()?, spec.byte_length)?;
        if before != after || after != visible {
            return Err(invalid());
        }
        self.verify_root()?;
        Ok(file)
    }

    pub(super) fn verify_chunk(
        &self,
        manifest: &FileTransferManifest,
        index: u32,
    ) -> io::Result<()> {
        let name = chunk_name(manifest, index)?;
        self.check_entry(manifest, index, OsStr::new(&name), None)
            .map(|_| ())
    }

    pub(super) fn stage<R: Read>(
        &self,
        manifest: &FileTransferManifest,
        index: u32,
        mut reader: R,
    ) -> Result<(), ChunkStagingError> {
        let name = chunk_name(manifest, index).map_err(|_| ChunkStagingError::Unavailable)?;
        self.stage_named(manifest, index, OsStr::new(&name), &mut reader)
            .map(|_| ())
    }

    pub(super) fn stage_named<R: Read>(
        &self,
        manifest: &FileTransferManifest,
        index: u32,
        name: &OsStr,
        mut reader: R,
    ) -> Result<(Snapshot, File), ChunkStagingError> {
        self.verify_root()
            .map_err(|_| ChunkStagingError::Unavailable)?;
        let mut file = open_at(
            &self.directory,
            name,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NONBLOCK,
            0o600,
        )
        .map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                ChunkStagingError::Conflict
            } else {
                // Failure to obtain a descriptor does not prove that creation had no side effect.
                ChunkStagingError::Unconfirmed
            }
        })?;
        let operation = || -> io::Result<(Snapshot, File)> {
            let original = created_identity(&file.metadata()?)?;
            copy_chunk_checked(manifest, index, &mut reader, &mut file).map_err(|_| invalid())?;
            file.set_permissions(Permissions::from_mode(0o400))?;
            file.sync_all()?;
            self.directory.sync_all()?;
            let spec = manifest.chunk(index).map_err(|_| invalid())?;
            let written = Snapshot::private(&file.metadata()?, spec.byte_length)?;
            if written.identity() != original {
                return Err(invalid());
            }
            // Close the writer before readback; no writable descriptor is returned to the caller.
            drop(file);
            let retained = self.check_entry(manifest, index, name, Some(&written))?;
            Ok((written, retained))
        };
        // Partial bytes, full corrupt chunks and even synchronized files all remain on failure.
        operation().map_err(|_| ChunkStagingError::Unconfirmed)
    }

    pub(super) fn verify_original(
        &self,
        manifest: &FileTransferManifest,
        index: u32,
        name: &OsStr,
        original: &Snapshot,
        retained: &File,
    ) -> io::Result<()> {
        self.verify_root()?;
        let spec = manifest.chunk(index).map_err(|_| invalid())?;
        if Snapshot::private(&retained.metadata()?, spec.byte_length)? != *original {
            return Err(invalid());
        }
        let _current = self.check_entry(manifest, index, name, Some(original))?;
        if Snapshot::private(&retained.metadata()?, spec.byte_length)? != *original {
            return Err(invalid());
        }
        self.verify_root()
    }
}
