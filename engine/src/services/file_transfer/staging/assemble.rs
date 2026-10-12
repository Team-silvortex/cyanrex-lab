use super::{
    binding::{assembled_name, manifest_digest},
    directory::ChunkDirectory,
    filesystem::{created_identity, entry_exists, invalid, open_at, Snapshot},
    whole, ChunkStagingError, FileTransferManifest,
};
use crate::{models::collaboration::FileVersionRef, services::file_transfer::verify_file};
use std::{
    ffi::OsStr,
    fs::{File, Permissions},
    io,
    os::unix::fs::PermissionsExt,
};

/// Move-only metadata acknowledging this local assembly call, not remote delivery or authority.
/// No bytes, pathname or descriptor are exposed, and supplied metadata cannot construct it.
///
/// ```compile_fail
/// use cyanrex_engine::services::file_transfer::staging::AssembledFile;
/// fn duplicate(file: &AssembledFile) -> AssembledFile { file.clone() }
/// ```
///
/// ```compile_fail
/// use cyanrex_engine::services::file_transfer::staging::AssembledFile;
/// fn serialize(file: &AssembledFile) { let _ = serde_json::to_vec(file); }
/// ```
///
/// ```compile_fail
/// use cyanrex_engine::models::collaboration::FileVersionRef;
/// use cyanrex_engine::services::file_transfer::staging::AssembledFile;
/// fn manufacture(file: FileVersionRef) -> AssembledFile { AssembledFile { file } }
/// ```
pub struct AssembledFile {
    file: FileVersionRef,
}

impl AssembledFile {
    pub fn file(&self) -> &FileVersionRef {
        &self.file
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    Created,
    Copied(u32),
    Readback,
}

pub(super) fn run(
    directory: &ChunkDirectory,
    manifest: &FileTransferManifest,
) -> Result<AssembledFile, ChunkStagingError> {
    run_inner(directory, manifest, |_| {})
}

fn run_inner<O: FnMut(Phase)>(
    directory: &ChunkDirectory,
    manifest: &FileTransferManifest,
    mut observer: O,
) -> Result<AssembledFile, ChunkStagingError> {
    directory
        .verify_root()
        .map_err(|_| ChunkStagingError::Unavailable)?;
    let binding = manifest_digest(manifest).map_err(|_| ChunkStagingError::Unavailable)?;
    let name = assembled_name(binding);
    if entry_exists(&directory.directory, OsStr::new(&name))
        .map_err(|_| ChunkStagingError::Unavailable)?
    {
        // The entry itself matters: a dangling symlink or special file must also conflict.
        return Err(ChunkStagingError::Conflict);
    }
    // Known bad or missing input bytes reject without creating even an empty output.
    let inputs = whole::observe_legacy(directory, manifest, binding)
        .map_err(|_| ChunkStagingError::Unavailable)?;
    directory
        .verify_root()
        .map_err(|_| ChunkStagingError::Unavailable)?;
    let writer = open_at(
        &directory.directory,
        OsStr::new(&name),
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NONBLOCK,
        0o600,
    )
    .map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            ChunkStagingError::Conflict
        } else {
            // An unsuccessful creation syscall cannot prove that it had no side effect.
            ChunkStagingError::Unconfirmed
        }
    })?;
    finish_created(directory, manifest, &name, &inputs, writer, &mut observer)
        .map_err(|_| ChunkStagingError::Unconfirmed)
}

fn finish_created<O: FnMut(Phase)>(
    directory: &ChunkDirectory,
    manifest: &FileTransferManifest,
    name: &str,
    inputs: &whole::LegacyFileObservation,
    mut writer: File,
    observer: &mut O,
) -> io::Result<AssembledFile> {
    let original = created_identity(&writer.metadata()?)?;
    observer(Phase::Created);
    // A second actual byte pass checks ordered chunk/whole digests while copying. The original
    // input snapshots, not just matching bytes at a replacement name, constrain every source.
    whole::copy_legacy(directory, manifest, inputs, &mut writer, |index| {
        observer(Phase::Copied(index));
    })?;
    writer.set_permissions(Permissions::from_mode(0o400))?;
    writer.sync_all()?;
    directory.directory.sync_all()?;
    let written = Snapshot::private_file(&writer.metadata()?, manifest.byte_length())?;
    if written.identity() != original {
        return Err(invalid());
    }
    drop(writer);
    let mut retained = open_at(
        &directory.directory,
        OsStr::new(name),
        libc::O_RDONLY | libc::O_NONBLOCK,
        0,
    )?;
    if Snapshot::private_file(&retained.metadata()?, manifest.byte_length())? != written {
        return Err(invalid());
    }
    // A third actual payload pass reads the output at the original inode, including exact length,
    // every chunk digest, final EOF and whole digest. No whole-file byte buffer is allocated.
    verify_file(manifest, &mut retained).map_err(|_| invalid())?;
    observer(Phase::Readback);
    whole::recheck_legacy(directory, manifest, inputs)?;
    require_output(directory, manifest, name, &written, &retained)?;
    Ok(AssembledFile {
        file: FileVersionRef::from_manifest(manifest),
    })
}

fn require_output(
    directory: &ChunkDirectory,
    manifest: &FileTransferManifest,
    name: &str,
    original: &Snapshot,
    retained: &File,
) -> io::Result<()> {
    directory.verify_root()?;
    let before = Snapshot::private_file(&retained.metadata()?, manifest.byte_length())?;
    let visible = open_at(
        &directory.directory,
        OsStr::new(name),
        libc::O_RDONLY | libc::O_NONBLOCK,
        0,
    )?;
    let visible = Snapshot::private_file(&visible.metadata()?, manifest.byte_length())?;
    let after = Snapshot::private_file(&retained.metadata()?, manifest.byte_length())?;
    if before != *original || visible != *original || after != *original {
        return Err(invalid());
    }
    directory.verify_root()
}

#[cfg(test)]
pub(super) fn run_with<O: FnMut(Phase)>(
    directory: &ChunkDirectory,
    manifest: &FileTransferManifest,
    observer: O,
) -> Result<AssembledFile, ChunkStagingError> {
    run_inner(directory, manifest, observer)
}
