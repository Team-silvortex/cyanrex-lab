//! Descriptor-relative traversal and checks. No supplied content filename or cleanup operation.
use std::{
    ffi::{CString, OsStr},
    fs::{File, Metadata, OpenOptions},
    io,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
        },
    },
    path::{Component, Path},
};

fn uid() -> u32 {
    // SAFETY: geteuid takes no arguments and has no memory preconditions.
    unsafe { libc::geteuid() }
}

pub(super) fn invalid() -> io::Error {
    io::ErrorKind::InvalidData.into()
}

pub(super) fn open_at(parent: &File, name: &OsStr, flags: i32, mode: u32) -> io::Result<File> {
    let name = CString::new(name.as_bytes()).map_err(|_| io::ErrorKind::InvalidInput)?;
    // SAFETY: the parent descriptor is live, name is NUL-terminated, and mode is valid for O_CREAT.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            mode,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful openat transfers a new, exclusively owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}

/// Checks the entry itself, including dangling symlinks and special files, without opening it.
pub(super) fn entry_exists(parent: &File, name: &OsStr) -> io::Result<bool> {
    let name = CString::new(name.as_bytes()).map_err(|_| io::ErrorKind::InvalidInput)?;
    let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: parent and name remain live; fstatat initializes the supplied stat on success.
    let status = unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            name.as_ptr(),
            metadata.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if status == 0 {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    if error.kind() == io::ErrorKind::NotFound {
        Ok(false)
    } else {
        Err(error)
    }
}

pub(super) fn open_root(path: &Path) -> io::Result<File> {
    let denied = || io::Error::from(io::ErrorKind::PermissionDenied);
    if !path.is_absolute() || path.as_os_str().len() > 4096 {
        return Err(denied());
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
            _ => return Err(denied()),
        };
        let meta = directory.metadata()?;
        if !meta.is_dir()
            || ![0, uid()].contains(&meta.uid())
            || (meta.mode() & 0o022 != 0 && !(meta.uid() == 0 && meta.mode() & 0o1000 != 0))
        {
            return Err(denied());
        }
        directory = open_at(&directory, name, libc::O_RDONLY | libc::O_DIRECTORY, 0)?;
        parts += 1;
    }
    let meta = directory.metadata()?;
    if parts == 0 || !meta.is_dir() || meta.uid() != uid() || meta.mode() & 0o7777 != 0o700 {
        return Err(denied());
    }
    Ok(directory)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Identity {
    device: u64,
    inode: u64,
}

impl Identity {
    pub(super) fn of(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }

    pub(super) fn bind_upload(self, source_key: [u8; 32]) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(b"cyanrex.file-upload.staging-root.v1\0");
        hash.update(source_key);
        hash.update(self.device.to_be_bytes());
        hash.update(self.inode.to_be_bytes());
        hash.finalize().into()
    }
}

pub(super) fn created_identity(metadata: &Metadata) -> io::Result<Identity> {
    if !metadata.is_file()
        || metadata.uid() != uid()
        || metadata.nlink() != 1
        || metadata.len() != 0
        || metadata.mode() & 0o7777 & !0o600 != 0
    {
        return Err(invalid());
    }
    Ok(Identity::of(metadata))
}

#[derive(PartialEq, Eq)]
pub(super) struct Snapshot {
    identity: Identity,
    byte_length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl Snapshot {
    pub(super) fn private(metadata: &Metadata, byte_length: u64) -> io::Result<Self> {
        Self::private_bounded(
            metadata,
            byte_length,
            crate::models::collaboration::FILE_TRANSFER_CHUNK_BYTES,
        )
    }

    pub(super) fn private_file(metadata: &Metadata, byte_length: u64) -> io::Result<Self> {
        Self::private_bounded(
            metadata,
            byte_length,
            crate::models::collaboration::MAX_FILE_TRANSFER_BYTES,
        )
    }

    fn private_bounded(metadata: &Metadata, byte_length: u64, maximum: u64) -> io::Result<Self> {
        if !metadata.is_file()
            || metadata.uid() != uid()
            || metadata.nlink() != 1
            || metadata.mode() & 0o7777 != 0o400
            || metadata.len() != byte_length
            || byte_length > maximum
        {
            return Err(invalid());
        }
        Ok(Self {
            identity: Identity::of(metadata),
            byte_length,
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        })
    }

    pub(super) fn identity(&self) -> Identity {
        self.identity
    }
}
