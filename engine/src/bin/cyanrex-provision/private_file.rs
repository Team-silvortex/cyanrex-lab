//! Descriptor-relative no-follow traversal; never chmod/adopt an existing path or remove evidence.
use super::Result;
use std::{
    ffi::{CString, OsStr},
    fs::{File, Metadata, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
        },
    },
    path::{Component, Path, PathBuf},
};

fn uid() -> u32 {
    // SAFETY: geteuid has no arguments or memory preconditions.
    unsafe { libc::geteuid() }
}
fn open_at(parent: &File, name: &OsStr, flags: i32, mode: u32) -> std::io::Result<File> {
    let name = CString::new(name.as_bytes()).map_err(|_| std::io::ErrorKind::InvalidInput)?;
    // SAFETY: parent is live, name is NUL-terminated, and mode is supplied for O_CREAT.
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
    // SAFETY: a successful openat returns a newly owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn trusted_directory(meta: &Metadata, last: bool) -> bool {
    meta.is_dir()
        && [0, uid()].contains(&meta.uid())
        && (meta.mode() & 0o022 == 0 || (!last && meta.uid() == 0 && meta.mode() & 0o1000 != 0))
}
fn parent(path: &Path) -> std::io::Result<(File, &OsStr)> {
    let invalid = || std::io::Error::from(std::io::ErrorKind::InvalidInput);
    if !path.is_absolute()
        || path.as_os_str().len() > 4096
        || path.as_os_str().as_bytes().ends_with(b"/")
    {
        return Err(invalid());
    }
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => parts.push(name),
            _ => return Err(invalid()),
        }
    }
    let name = parts.pop().ok_or_else(invalid)?;
    let mut directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open("/")?;
    for part in parts {
        if !trusted_directory(&directory.metadata()?, false) {
            return Err(invalid());
        }
        directory = open_at(&directory, part, libc::O_RDONLY | libc::O_DIRECTORY, 0)?;
    }
    if !trusted_directory(&directory.metadata()?, true) {
        return Err(invalid());
    }
    Ok((directory, name))
}
fn private(meta: &Metadata) -> bool {
    meta.is_file() && meta.uid() == uid() && meta.nlink() == 1 && meta.mode() & 0o7077 == 0
}
fn same(first: &Metadata, second: &Metadata) -> bool {
    first.dev() == second.dev() && first.ino() == second.ino()
}

pub fn read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let operation = || -> std::io::Result<Vec<u8>> {
        let (directory, name) = parent(path)?;
        let file = open_at(&directory, name, libc::O_RDONLY | libc::O_NONBLOCK, 0)?;
        let meta = file.metadata()?;
        if !private(&meta) || meta.len() > limit {
            return Err(std::io::ErrorKind::InvalidInput.into());
        }
        let mut bytes = Vec::new();
        file.take(limit + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limit {
            return Err(std::io::ErrorKind::InvalidInput.into());
        }
        Ok(bytes)
    };
    operation().map_err(|_| "unsafe_input_file")
}

pub struct Reserved {
    path: PathBuf,
    directory: File,
    file: File,
}
impl Reserved {
    pub fn create(path: &Path, marker: &[u8]) -> Result<Self> {
        let operation = || -> std::io::Result<Self> {
            let (directory, name) = parent(path)?;
            let meta = directory.metadata()?;
            if meta.uid() != uid() || meta.mode() & 0o7777 != 0o700 {
                return Err(std::io::ErrorKind::PermissionDenied.into());
            }
            let file = open_at(
                &directory,
                name,
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NONBLOCK,
                0o600,
            )?;
            let mut result = Self {
                path: path.to_owned(),
                directory,
                file,
            };
            result.write(marker)?;
            Ok(result)
        };
        operation().map_err(|_| "enrollment_unavailable")
    }
    fn verify(&self) -> std::io::Result<()> {
        let (directory, name) = parent(&self.path)?;
        let visible = open_at(&directory, name, libc::O_RDONLY | libc::O_NONBLOCK, 0)?;
        let meta = self.file.metadata()?;
        let parent_meta = directory.metadata()?;
        if !private(&meta)
            || meta.mode() & 0o777 != 0o600
            || !same(&meta, &visible.metadata()?)
            || !same(&parent_meta, &self.directory.metadata()?)
            || parent_meta.uid() != uid()
            || parent_meta.mode() & 0o7777 != 0o700
        {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }
        Ok(())
    }
    pub fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.verify()?;
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(bytes)?;
        self.file.set_len(bytes.len() as u64)?;
        self.file.sync_all()?;
        self.verify()?;
        self.directory.sync_all()
    }
}
