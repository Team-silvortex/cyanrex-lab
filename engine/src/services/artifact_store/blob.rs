//! Descriptor-relative immutable files. No supplied filenames, overwrites, adoption or cleanup.
use super::*;
use sha2::{Digest, Sha256};
use std::{
    ffi::{CString, OsStr},
    fs::{File, Metadata, OpenOptions, Permissions},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        },
    },
    path::{Component, PathBuf},
};

pub(super) fn digest(bytes: &[u8]) -> Sha256Digest {
    format!("{:x}", Sha256::digest(bytes))
        .parse()
        .expect("SHA-256 encoding")
}
fn uid() -> u32 {
    // SAFETY: geteuid takes no arguments and has no memory preconditions.
    unsafe { libc::geteuid() }
}
fn open_at(parent: &File, name: &OsStr, flags: i32, mode: u32) -> std::io::Result<File> {
    let name = CString::new(name.as_bytes()).map_err(|_| std::io::ErrorKind::InvalidInput)?;
    // SAFETY: descriptor is live, name is NUL-terminated, and mode is passed for O_CREAT.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            mode,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: a successful openat transfers a new owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn same(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino()
}
fn private_file(meta: &Metadata, length: u64) -> bool {
    meta.is_file()
        && meta.uid() == uid()
        && meta.nlink() == 1
        && meta.mode() & 0o7777 == 0o400
        && meta.len() == length
        && length <= MAX_CONTENT as u64
}
fn open_root(path: &Path) -> std::io::Result<File> {
    let invalid = || std::io::Error::from(std::io::ErrorKind::PermissionDenied);
    if !path.is_absolute() || path.as_os_str().len() > 4096 {
        return Err(invalid());
    }
    let mut directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open("/")?;
    let mut parts = 0;
    for part in path.components() {
        let name = match part {
            Component::RootDir => continue,
            Component::Normal(name) => name,
            _ => return Err(invalid()),
        };
        let meta = directory.metadata()?;
        if !meta.is_dir()
            || ![0, uid()].contains(&meta.uid())
            || (meta.mode() & 0o022 != 0 && !(meta.uid() == 0 && meta.mode() & 0o1000 != 0))
        {
            return Err(invalid());
        }
        directory = open_at(&directory, name, libc::O_RDONLY | libc::O_DIRECTORY, 0)?;
        parts += 1;
    }
    let meta = directory.metadata()?;
    if parts == 0 || meta.uid() != uid() || meta.mode() & 0o7777 != 0o700 {
        return Err(invalid());
    }
    Ok(directory)
}
fn name(reference: &ArtifactRef) -> String {
    format!(
        "{}-{}-{}-{}-{}",
        reference.workspace.authority_id,
        reference.workspace.workspace_id,
        reference.artifact_id,
        reference.revision_id,
        reference.sha256
    )
}

pub(super) struct BlobDirectory {
    path: PathBuf,
    directory: File,
}
impl BlobDirectory {
    pub(super) fn open(path: &Path) -> Result<Self> {
        let directory = open_root(path).map_err(|_| ArtifactStoreError::BlobUnavailable)?;
        Ok(Self {
            path: path.to_owned(),
            directory,
        })
    }
    pub(super) fn verify(&self) -> Result<()> {
        let operation = || -> std::io::Result<()> {
            let visible = open_root(&self.path)?;
            if !same(&visible.metadata()?, &self.directory.metadata()?) {
                return Err(std::io::ErrorKind::PermissionDenied.into());
            }
            Ok(())
        };
        operation().map_err(|_| ArtifactStoreError::BlobUnavailable)
    }
    pub(super) fn write_once(&self, revision: &ArtifactRevision, bytes: &[u8]) -> Result<()> {
        self.verify()?;
        if bytes.len() > MAX_CONTENT
            || bytes.len() as u64 != revision.byte_length
            || digest(bytes) != revision.reference.sha256
        {
            return Err(ArtifactStoreError::InvalidInput);
        }
        let operation = || -> std::io::Result<()> {
            let mut file = open_at(
                &self.directory,
                OsStr::new(&name(&revision.reference)),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NONBLOCK,
                0o600,
            )?;
            file.write_all(bytes)?;
            file.set_permissions(Permissions::from_mode(0o400))?;
            file.sync_all()?;
            self.directory.sync_all()?;
            Ok(())
        };
        operation().map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                ArtifactStoreError::Conflict
            } else {
                ArtifactStoreError::BlobUnavailable
            }
        })?;
        // A cancelled blocking operation may finish here and leave an unpublished file.
        // Never remove it: cancellation or a later database error does not prove non-publication.
        self.read(revision)?;
        Ok(())
    }
    pub(super) fn read(&self, revision: &ArtifactRevision) -> Result<Vec<u8>> {
        self.verify()?;
        let operation = || -> std::io::Result<Vec<u8>> {
            let name = name(&revision.reference);
            let mut file = open_at(
                &self.directory,
                OsStr::new(&name),
                libc::O_RDONLY | libc::O_NONBLOCK,
                0,
            )?;
            if !private_file(&file.metadata()?, revision.byte_length) {
                return Err(std::io::ErrorKind::InvalidData.into());
            }
            let mut bytes = Vec::new();
            (&mut file)
                .take(revision.byte_length + 1)
                .read_to_end(&mut bytes)?;
            let meta = file.metadata()?;
            let visible = open_at(
                &self.directory,
                OsStr::new(&name),
                libc::O_RDONLY | libc::O_NONBLOCK,
                0,
            )?;
            if !private_file(&meta, revision.byte_length)
                || !same(&meta, &visible.metadata()?)
                || bytes.len() as u64 != revision.byte_length
                || digest(&bytes) != revision.reference.sha256
            {
                return Err(std::io::ErrorKind::InvalidData.into());
            }
            Ok(bytes)
        };
        let bytes = operation().map_err(|_| ArtifactStoreError::BlobUnavailable)?;
        self.verify()?;
        Ok(bytes)
    }
}
