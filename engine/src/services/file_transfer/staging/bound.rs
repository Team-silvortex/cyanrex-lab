//! A local original-object proof only. One retained read-only descriptor per live receipt.
use super::{
    binding::bound_chunk_name, directory::ChunkDirectory, filesystem::Snapshot, ChunkStagingError,
    FileTransferManifest,
};
use std::{ffi::OsStr, fs::File, io::Read};

/// Opaque and move-only: observation cannot manufacture a successful stage acknowledgement.
pub(crate) struct BoundChunkReceipt {
    key: [u8; 32],
    index: u32,
    binding: [u8; 32],
    original: Snapshot,
    retained: File,
}

impl BoundChunkReceipt {
    pub(super) fn require_binding(
        &self,
        key: [u8; 32],
        manifest: &FileTransferManifest,
        index: u32,
    ) -> Result<(), ChunkStagingError> {
        let (_, binding) =
            bound_chunk_name(key, manifest, index).map_err(|_| ChunkStagingError::InvalidInput)?;
        if self.key != key || self.index != index || self.binding != binding {
            return Err(ChunkStagingError::InvalidInput);
        }
        Ok(())
    }
}

pub(super) fn stage<R: Read>(
    directory: &ChunkDirectory,
    key: [u8; 32],
    manifest: &FileTransferManifest,
    index: u32,
    reader: R,
) -> Result<BoundChunkReceipt, ChunkStagingError> {
    let (name, binding) =
        bound_chunk_name(key, manifest, index).map_err(|_| ChunkStagingError::Unavailable)?;
    let (original, retained) = directory.stage_named(manifest, index, OsStr::new(&name), reader)?;
    Ok(BoundChunkReceipt {
        key,
        index,
        binding,
        original,
        retained,
    })
}

pub(super) fn verify(
    directory: &ChunkDirectory,
    key: [u8; 32],
    manifest: &FileTransferManifest,
    index: u32,
    receipt: BoundChunkReceipt,
) -> Result<BoundChunkReceipt, ChunkStagingError> {
    receipt.require_binding(key, manifest, index)?;
    let (name, _) =
        bound_chunk_name(key, manifest, index).map_err(|_| ChunkStagingError::Unavailable)?;
    directory
        .verify_original(
            manifest,
            index,
            OsStr::new(&name),
            &receipt.original,
            &receipt.retained,
        )
        .map_err(|_| ChunkStagingError::Unavailable)?;
    Ok(receipt)
}
