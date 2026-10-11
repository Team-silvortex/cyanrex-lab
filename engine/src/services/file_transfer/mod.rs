//! Bounded stream checks and separate Unix chunk staging, not publication or authorization.
//!
//! Synchronous readers and writers are supplied by a trusted caller. Those operations neither open
//! paths nor bound blocking time. The explicit Unix staging module has its own private root and
//! worker budget; deadlines, stream framing and durable upload state still belong to the caller.
//! Failed copies may leave a partial or complete invalid chunk in the sink. Success checks supplied
//! bytes only, not authenticated provenance, durable storage, a publication or a transfer receipt.
use crate::models::collaboration::{FileId, FileTransferManifest, FileVersionId, WorkspaceRef};
use std::io::{Read, Write};

#[cfg(unix)]
mod descriptor;
#[cfg(unix)]
pub mod staging;
mod stream;

pub const FILE_TRANSFER_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum FileTransferError {
    #[error("invalid file chunk")]
    InvalidChunk,
    #[error("file length does not match")]
    LengthMismatch,
    #[error("file digest does not match")]
    DigestMismatch,
    #[error("invalid file manifest")]
    InvalidManifest,
    #[error("file source changed")]
    SourceChanged,
    #[error("file IO failed")]
    Io(#[from] std::io::Error),
}

pub fn inspect_stream<R: Read + ?Sized>(
    workspace: WorkspaceRef,
    file_id: FileId,
    version_id: FileVersionId,
    display_name: impl Into<String>,
    reader: &mut R,
) -> Result<FileTransferManifest, FileTransferError> {
    stream::inspect(workspace, file_id, version_id, display_name.into(), reader)
}

/// The reader starts at the chunk's first byte and contains that chunk only. No seek is performed.
/// EOF probing can consume one excess byte; caller-supplied framing is not proven by this function.
pub fn verify_chunk<R: Read + ?Sized>(
    manifest: &FileTransferManifest,
    index: u32,
    reader: &mut R,
) -> Result<(), FileTransferError> {
    stream::verify_chunk(manifest, index, reader)
}

/// Copies into caller-owned, unconfirmed output without flushing, seeking or removing any bytes.
/// The complete chunk may already have been written when a digest or EOF check subsequently fails.
pub fn copy_chunk_checked<R: Read + ?Sized, W: Write + ?Sized>(
    manifest: &FileTransferManifest,
    index: u32,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), FileTransferError> {
    stream::copy_chunk(manifest, index, reader, writer)
}

/// Checks every ordered chunk and the whole stream, with one EOF probe after all expected bytes.
pub fn verify_stream<R: Read + ?Sized>(
    manifest: &FileTransferManifest,
    reader: &mut R,
) -> Result<(), FileTransferError> {
    stream::verify(manifest, reader)
}

/// Reads a preopened single-link regular Unix file at offset zero, then checks observed metadata.
/// This does not verify a pathname, private root, trusted ownership or permanent immutability.
#[cfg(unix)]
pub fn inspect_file(
    workspace: WorkspaceRef,
    file_id: FileId,
    version_id: FileVersionId,
    display_name: impl Into<String>,
    file: &mut std::fs::File,
) -> Result<FileTransferManifest, FileTransferError> {
    descriptor::inspect(workspace, file_id, version_id, display_name.into(), file)
}

/// An observation of this descriptor's supplied bytes, never a reusable access or storage grant.
#[cfg(unix)]
pub fn verify_file(
    manifest: &FileTransferManifest,
    file: &mut std::fs::File,
) -> Result<(), FileTransferError> {
    descriptor::verify(manifest, file)
}
