//! Private buffered bytes, never a publication receipt or reusable authorization token.
use super::{
    binding::bound_chunk_name,
    directory::ChunkDirectory,
    filesystem::{invalid, open_at, Identity, Snapshot},
    ChunkStagingError, FileTransferManifest,
};
use std::{ffi::OsStr, fs::File, io};
use tokio::sync::OwnedSemaphorePermit;

/// Move-only result. The response budget remains owned until these private bytes are dropped.
pub(crate) struct BufferedBoundChunkRead {
    bytes: Vec<u8>,
    // A live original descriptor prevents inode reuse, without exposing a path or descriptor.
    // The bytes remain an immutable verified read snapshot, not current path-health at delivery.
    _retained: File,
    _original: Snapshot,
    _root: Identity,
    _key: [u8; 32],
    _index: u32,
    _binding: [u8; 32],
    // Last field: dropping bytes and the retained descriptor precedes returning response capacity.
    _lease: OwnedSemaphorePermit,
}

impl BufferedBoundChunkRead {
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

pub(super) enum ReadPoint {
    Opened,
    Copied,
}

// The private observer is inert in production. Tests use exact in-job boundaries, never sleeps
// or a public file/reader injection API, to prove cancellation and observable entry/root drift.
pub(super) fn read<O: FnMut(ReadPoint)>(
    directory: &ChunkDirectory,
    key: [u8; 32],
    manifest: &FileTransferManifest,
    index: u32,
    lease: OwnedSemaphorePermit,
    observer: O,
) -> Result<BufferedBoundChunkRead, ChunkStagingError> {
    read_inner(directory, key, manifest, index, lease, observer)
        .map_err(|_| ChunkStagingError::Unavailable)
}

fn read_inner<O: FnMut(ReadPoint)>(
    directory: &ChunkDirectory,
    key: [u8; 32],
    manifest: &FileTransferManifest,
    index: u32,
    lease: OwnedSemaphorePermit,
    mut observer: O,
) -> io::Result<BufferedBoundChunkRead> {
    directory.verify_root()?;
    let spec = manifest.chunk(index).map_err(|_| invalid())?;
    let (name, binding) = bound_chunk_name(key, manifest, index)?;
    // Nonblocking open lets metadata reject a substituted FIFO before any content read.
    let mut file = open_at(
        &directory.directory,
        OsStr::new(&name),
        libc::O_RDONLY | libc::O_NONBLOCK,
        0,
    )?;
    let original = Snapshot::private(&file.metadata()?, spec.byte_length)?;
    observer(ReadPoint::Opened);
    let length = usize::try_from(spec.byte_length).map_err(|_| invalid())?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| invalid())?;
    // Partial output is strictly private. Nothing is returned on late EOF/hash failure.
    super::super::copy_chunk_checked(manifest, index, &mut file, &mut bytes)
        .map_err(|_| invalid())?;
    if bytes.len() != length {
        return Err(invalid());
    }
    observer(ReadPoint::Copied);
    let after = Snapshot::private(&file.metadata()?, spec.byte_length)?;
    let visible = open_at(
        &directory.directory,
        OsStr::new(&name),
        libc::O_RDONLY | libc::O_NONBLOCK,
        0,
    )?;
    let visible = Snapshot::private(&visible.metadata()?, spec.byte_length)?;
    if original != after || after != visible {
        return Err(invalid());
    }
    directory.verify_root()?;
    Ok(BufferedBoundChunkRead {
        bytes,
        _lease: lease,
        _retained: file,
        _original: original,
        _root: directory.identity,
        _key: key,
        _index: index,
        _binding: binding,
    })
}

#[cfg(test)]
#[path = "read_tests.rs"]
mod tests;
