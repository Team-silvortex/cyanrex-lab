//! Names bind the complete typed v1 manifest and index, never supplied display names or paths.
use crate::models::collaboration::FileTransferManifest;
use sha2::{Digest, Sha256};
use std::io::{self, Write};

struct HashWriter(Sha256);

impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn chunk_name(manifest: &FileTransferManifest, index: u32) -> io::Result<String> {
    let mut hash = HashWriter(Sha256::new());
    // Typed serialization fixes field order, UUID/digest spelling and numbers. This is a private
    // filename format, not canonicalization of arbitrary JSON or an authenticated capability.
    serde_json::to_writer(&mut hash, manifest).map_err(|_| io::ErrorKind::InvalidData)?;
    Ok(format!("v1-{:x}-{index}", hash.0.finalize()))
}

pub(super) fn bound_chunk_name(
    key: [u8; 32],
    manifest: &FileTransferManifest,
    index: u32,
) -> io::Result<(String, [u8; 32])> {
    let mut hash = HashWriter(Sha256::new());
    hash.0.update(b"cyanrex.file-upload.staged-chunk.v1\0");
    hash.0.update(key);
    hash.0.update(index.to_be_bytes());
    serde_json::to_writer(&mut hash, manifest).map_err(|_| io::ErrorKind::InvalidData)?;
    let digest = hash.0.finalize();
    Ok((format!("u1-{digest:x}-{index}"), digest.into()))
}
