use super::{
    validation,
    AiAgentSettingsError::{Conflict, InvalidInput, StorageUnavailable},
    Result, MAX_AI_AGENT_SETTINGS_BYTES,
};
use crate::models::ai_agent::{AiAgentSettings, UpdateAiAgentSettingsRequest};
use std::{
    ffi::{CStr, CString},
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::{Component, Path},
};

const SNAPSHOT: &CStr = c"settings.json";

pub(super) fn execute(
    path: &Path,
    private_parent: Option<&Path>,
    update: Option<UpdateAiAgentSettingsRequest>,
) -> Result<AiAgentSettings> {
    if let Some(parent) = private_parent {
        open_directory(parent, false)?;
    }
    let directory = open_directory(path, false)?;
    let current = match directory.as_ref() {
        Some(directory) => read(directory)?,
        None => AiAgentSettings::default(),
    };
    let Some(update) = update else {
        return Ok(current);
    };
    if current.revision != update.expected_revision {
        return Err(Conflict);
    }
    let next = AiAgentSettings {
        revision: current.revision.checked_add(1).ok_or(InvalidInput)?,
        default_profile_id: update.default_profile_id,
        profiles: update.profiles,
    };
    let bytes = serde_json::to_vec(&next).map_err(|_| InvalidInput)?;
    if bytes.len() > MAX_AI_AGENT_SETTINGS_BYTES {
        return Err(InvalidInput);
    }
    let directory = match directory {
        Some(directory) => directory,
        None => {
            if let Some(parent) = private_parent {
                open_directory(parent, true)?;
            }
            open_directory(path, true)?.ok_or(StorageUnavailable)?
        }
    };
    // Re-read after directory creation rather than overwrite an unexpected existing file.
    if read(&directory)? != current {
        return Err(StorageUnavailable);
    }
    persist(&directory, &bytes)?;
    if read(&directory)? != next {
        return Err(StorageUnavailable);
    }
    Ok(next)
}

fn open_directory(path: &Path, create: bool) -> Result<Option<File>> {
    // Walk with O_NOFOLLOW, including existing ancestors, and never chmod a shared parent.
    // The configured root is trusted configuration, not a path supplied by the HTTP client.
    let root = if path.is_absolute() { c"/" } else { c"." };
    let mut directory = open_at(libc::AT_FDCWD, root, libc::O_RDONLY | libc::O_DIRECTORY, 0)?;
    let mut normals = 0;
    for component in path.components() {
        let component = match component {
            Component::RootDir | Component::CurDir => continue,
            Component::Normal(component) => component,
            _ => return Err(StorageUnavailable),
        };
        normals += 1;
        let name = CString::new(component.as_bytes()).map_err(|_| StorageUnavailable)?;
        match try_open_at(
            directory.as_raw_fd(),
            &name,
            libc::O_RDONLY | libc::O_DIRECTORY,
            0,
        ) {
            Ok(next) => directory = next,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if !create {
                    return Ok(None);
                }
                // Exclusive creation is relative to an already opened, non-symlink directory.
                let created = unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), 0o700) };
                if created != 0
                    && std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists
                {
                    return Err(StorageUnavailable);
                }
                directory = open_at(
                    directory.as_raw_fd(),
                    &name,
                    libc::O_RDONLY | libc::O_DIRECTORY,
                    0,
                )?;
            }
            Err(_) => return Err(StorageUnavailable),
        }
    }
    if normals == 0 {
        return Err(StorageUnavailable);
    }
    let metadata = directory.metadata().map_err(|_| StorageUnavailable)?;
    if !metadata.is_dir()
        || metadata.mode() & 0o7777 != 0o700
        || metadata.uid() != unsafe { libc::geteuid() }
    {
        return Err(StorageUnavailable);
    }
    Ok(Some(directory))
}

fn read(directory: &File) -> Result<AiAgentSettings> {
    let file = match try_open_at(
        directory.as_raw_fd(),
        SNAPSHOT,
        libc::O_RDONLY | libc::O_NONBLOCK,
        0,
    ) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AiAgentSettings::default())
        }
        Err(_) => return Err(StorageUnavailable),
    };
    let metadata = file.metadata().map_err(|_| StorageUnavailable)?;
    if !metadata.is_file()
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.len() > MAX_AI_AGENT_SETTINGS_BYTES as u64
    {
        return Err(StorageUnavailable);
    }
    let mut bytes = Vec::new();
    file.take((MAX_AI_AGENT_SETTINGS_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| StorageUnavailable)?;
    if bytes.len() > MAX_AI_AGENT_SETTINGS_BYTES {
        return Err(StorageUnavailable);
    }
    let snapshot: AiAgentSettings =
        serde_json::from_slice(&bytes).map_err(|_| StorageUnavailable)?;
    validation::validate(&snapshot.default_profile_id, &snapshot.profiles)
        .map_err(|_| StorageUnavailable)?;
    Ok(snapshot)
}

fn persist(directory: &File, bytes: &[u8]) -> Result<()> {
    let name = CString::new(format!(".ai-agents-{}.tmp", uuid::Uuid::new_v4()))
        .map_err(|_| StorageUnavailable)?;
    let mut file = open_at(
        directory.as_raw_fd(),
        &name,
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        0o600,
    )?;
    let _cleanup = TemporarySnapshot {
        directory,
        name: &name,
    };
    file.write_all(bytes).map_err(|_| StorageUnavailable)?;
    file.sync_all().map_err(|_| StorageUnavailable)?;
    let renamed = unsafe {
        libc::renameat(
            directory.as_raw_fd(),
            name.as_ptr(),
            directory.as_raw_fd(),
            SNAPSHOT.as_ptr(),
        )
    };
    if renamed != 0 {
        return Err(StorageUnavailable);
    }
    directory.sync_all().map_err(|_| StorageUnavailable)
}

fn open_at(parent: i32, name: &CStr, flags: i32, mode: libc::mode_t) -> Result<File> {
    try_open_at(parent, name, flags, mode).map_err(|_| StorageUnavailable)
}

fn try_open_at(parent: i32, name: &CStr, flags: i32, mode: libc::mode_t) -> std::io::Result<File> {
    let fd = unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            flags | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            mode,
        )
    };
    if fd < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

struct TemporarySnapshot<'a> {
    directory: &'a File,
    name: &'a CStr,
}
impl Drop for TemporarySnapshot<'_> {
    fn drop(&mut self) {
        // This unique name belongs only to this operation. After rename it no longer exists.
        unsafe {
            libc::unlinkat(self.directory.as_raw_fd(), self.name.as_ptr(), 0);
        }
    }
}
