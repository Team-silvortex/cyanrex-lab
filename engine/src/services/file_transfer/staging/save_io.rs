use super::super::{stream, verify_file, verify_stream};
use super::{
    assembled_proof::AssembledProof,
    directory::ChunkDirectory,
    filesystem::{created_identity, entry_exists, invalid, open_at, Snapshot},
    save::Target,
    FileSaveError, FileTransferManifest, SavedFile,
};
use std::{
    ffi::OsStr,
    fs::{File, Permissions},
    io,
    os::unix::fs::PermissionsExt,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    Created,
    Copied,
    Readback,
}

pub(super) fn run(
    source: &ChunkDirectory,
    manifest: &FileTransferManifest,
    proof: &AssembledProof,
    target: &Target,
) -> Result<SavedFile, FileSaveError> {
    run_inner(source, manifest, proof, target, |_| {})
}

fn run_inner<O: FnMut(Phase)>(
    source: &ChunkDirectory,
    manifest: &FileTransferManifest,
    proof: &AssembledProof,
    target: &Target,
    mut observer: O,
) -> Result<SavedFile, FileSaveError> {
    proof
        .verify(source, manifest)
        .map_err(|_| FileSaveError::Unavailable)?;
    target
        .directory
        .verify_root()
        .map_err(|_| FileSaveError::Unavailable)?;
    let name = OsStr::new(&target.leaf);
    if entry_exists(target.directory.fd(), name).map_err(|_| FileSaveError::Unavailable)? {
        return Err(FileSaveError::Conflict);
    }
    // A complete first payload pass catches bad original bytes without creating an output.
    verify_stream(manifest, &mut proof.reader()).map_err(|_| FileSaveError::Unavailable)?;
    proof
        .verify(source, manifest)
        .map_err(|_| FileSaveError::Unavailable)?;
    target
        .directory
        .verify_root()
        .map_err(|_| FileSaveError::Unavailable)?;
    let writer = open_at(
        target.directory.fd(),
        name,
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NONBLOCK,
        0o600,
    )
    .map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            FileSaveError::Conflict
        } else {
            FileSaveError::Unconfirmed
        }
    })?;
    finish_created(source, manifest, proof, target, writer, &mut observer)
        .map_err(|_| FileSaveError::Unconfirmed)
}

fn finish_created<O: FnMut(Phase)>(
    source: &ChunkDirectory,
    manifest: &FileTransferManifest,
    proof: &AssembledProof,
    target: &Target,
    mut writer: File,
    observer: &mut O,
) -> io::Result<SavedFile> {
    let original = created_identity(&writer.metadata()?)?;
    if original == proof.identity() {
        return Err(invalid());
    }
    observer(Phase::Created);
    // This second source pass independently checks every ordered digest and whole EOF while
    // copying. The original retained source descriptor never changes its shared offset.
    stream::copy_verify(manifest, &mut proof.reader(), &mut writer).map_err(|_| invalid())?;
    proof.verify(source, manifest)?;
    observer(Phase::Copied);
    writer.set_permissions(Permissions::from_mode(0o400))?;
    writer.sync_all()?;
    target.directory.fd().sync_all()?;
    let written = Snapshot::private_file(&writer.metadata()?, manifest.byte_length())?;
    if written.identity() != original {
        return Err(invalid());
    }
    let mut retained = open_at(
        target.directory.fd(),
        OsStr::new(&target.leaf),
        libc::O_RDONLY | libc::O_NONBLOCK,
        0,
    )?;
    if Snapshot::private_file(&retained.metadata()?, manifest.byte_length())? != written {
        return Err(invalid());
    }
    // Pin the original inode read-only before closing the writer. Reopening a reused inode cannot
    // substitute for the originally created object, and no writer remains through actual readback.
    drop(writer);
    verify_file(manifest, &mut retained).map_err(|_| invalid())?;
    observer(Phase::Readback);
    proof.verify(source, manifest)?;
    require_output(target, manifest, &written, &retained)?;
    proof.verify(source, manifest)?;
    Ok(SavedFile::confirmed(manifest))
}

fn require_output(
    target: &Target,
    manifest: &FileTransferManifest,
    original: &Snapshot,
    retained: &File,
) -> io::Result<()> {
    target.directory.verify_root()?;
    let before = Snapshot::private_file(&retained.metadata()?, manifest.byte_length())?;
    let visible = open_at(
        target.directory.fd(),
        OsStr::new(&target.leaf),
        libc::O_RDONLY | libc::O_NONBLOCK,
        0,
    )?;
    let visible = Snapshot::private_file(&visible.metadata()?, manifest.byte_length())?;
    let after = Snapshot::private_file(&retained.metadata()?, manifest.byte_length())?;
    if before != *original || visible != *original || after != *original {
        return Err(invalid());
    }
    target.directory.verify_root()
}

#[cfg(test)]
pub(super) fn run_with<O: FnMut(Phase)>(
    source: &ChunkDirectory,
    manifest: &FileTransferManifest,
    proof: &AssembledProof,
    target: &Target,
    observer: O,
) -> Result<SavedFile, FileSaveError> {
    run_inner(source, manifest, proof, target, observer)
}
