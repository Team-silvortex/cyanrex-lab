//! Bounded, descriptor-relative private input only. No writes, adoption or permission repair.
use std::{
    ffi::{CString, OsStr},
    fs::{File, Metadata, OpenOptions},
    io::Read,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
        },
    },
    path::Path,
};

const MAX_BYTES: u64 = 16 * 1024;

fn uid() -> u32 {
    // SAFETY: geteuid has no arguments or memory preconditions.
    unsafe { libc::geteuid() }
}

fn open_at(parent: &File, name: &OsStr, directory: bool) -> std::io::Result<File> {
    let name = CString::new(name.as_bytes()).map_err(|_| std::io::ErrorKind::InvalidInput)?;
    let flags = libc::O_RDONLY
        | libc::O_CLOEXEC
        | libc::O_NOFOLLOW
        | libc::O_NONBLOCK
        | if directory { libc::O_DIRECTORY } else { 0 };
    // SAFETY: the held parent is live and the single name is NUL-terminated. No O_CREAT.
    let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: successful openat returned one newly owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn parent(path: &Path) -> std::io::Result<(File, &OsStr)> {
    let invalid = || std::io::Error::from(std::io::ErrorKind::PermissionDenied);
    let text = path.to_str().ok_or_else(invalid)?;
    if !super::config::canonical_path(text) {
        return Err(invalid());
    }
    let mut parts = text.split('/').skip(1).collect::<Vec<_>>();
    let name = parts.pop().ok_or_else(invalid)?;
    let mut directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open("/")?;
    for part in parts {
        let meta = directory.metadata()?;
        if !meta.is_dir()
            || ![0, uid()].contains(&meta.uid())
            || (meta.mode() & 0o022 != 0 && !(meta.uid() == 0 && meta.mode() & 0o1000 != 0))
        {
            return Err(invalid());
        }
        directory = open_at(&directory, OsStr::new(part), true)?;
    }
    let meta = directory.metadata()?;
    if !meta.is_dir() || meta.uid() != uid() || meta.mode() & 0o7777 != 0o700 {
        return Err(invalid());
    }
    Ok((directory, OsStr::new(name)))
}

fn directory_identity(meta: &Metadata) -> (u64, u64, u32, u32, u32) {
    (meta.dev(), meta.ino(), meta.uid(), meta.gid(), meta.mode())
}

fn snapshot(meta: &Metadata) -> (u64, u64, u32, u32, u32, u64, u64, i64, i64, i64, i64) {
    (
        meta.dev(),
        meta.ino(),
        meta.uid(),
        meta.gid(),
        meta.mode(),
        meta.nlink(),
        meta.len(),
        meta.mtime(),
        meta.mtime_nsec(),
        meta.ctime(),
        meta.ctime_nsec(),
    )
}

pub(super) fn read(path: &Path) -> Result<Vec<u8>, ()> {
    let operation = || -> std::io::Result<Vec<u8>> {
        let invalid = || std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        let (directory, name) = parent(path)?;
        let parent_identity = directory_identity(&directory.metadata()?);
        let mut file = open_at(&directory, name, false)?;
        let before = file.metadata()?;
        if !before.is_file()
            || before.uid() != uid()
            || before.nlink() != 1
            || !matches!(before.mode() & 0o7777, 0o400 | 0o600)
            || before.len() > MAX_BYTES
        {
            return Err(invalid());
        }
        let original = snapshot(&before);
        let mut bytes = Vec::new();
        (&mut file).take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 != before.len() || snapshot(&file.metadata()?) != original {
            return Err(invalid());
        }
        let (visible_parent, visible_name) = parent(path)?;
        let visible = open_at(&visible_parent, visible_name, false)?;
        if directory_identity(&visible_parent.metadata()?) != parent_identity
            || directory_identity(&directory.metadata()?) != parent_identity
            || snapshot(&visible.metadata()?) != original
            || snapshot(&file.metadata()?) != original
        {
            return Err(invalid());
        }
        Ok(bytes)
    };
    operation().map_err(|_| ())
}
