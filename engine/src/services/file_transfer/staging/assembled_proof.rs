//! Original assembly descriptors only: never reconstructed from public metadata or current bytes.
use super::{
    binding::assembled_name,
    directory::ChunkDirectory,
    filesystem::{invalid, open_at, Identity, Snapshot},
    FileTransferManifest,
};
use std::{
    ffi::OsStr,
    fs::File,
    io::{self, Read},
    os::unix::fs::FileExt,
};

pub(super) struct AssembledProof {
    file: File,
    snapshot: Snapshot,
    binding: [u8; 32],
    root: Identity,
    directory: File,
}

impl AssembledProof {
    /// Called only after the original assembly's byte/readback/final verification succeeded.
    pub(super) fn capture(
        directory: &ChunkDirectory,
        file: File,
        snapshot: Snapshot,
        binding: [u8; 32],
    ) -> io::Result<Self> {
        let original_root = directory.directory.try_clone()?;
        Ok(Self {
            file,
            snapshot,
            binding,
            root: directory.identity,
            directory: original_root,
        })
    }

    pub(super) fn matches(&self, root: Identity, binding: [u8; 32]) -> bool {
        self.root == root && self.binding == binding
    }

    pub(super) fn identity(&self) -> Identity {
        self.snapshot.identity()
    }

    pub(super) fn verify(
        &self,
        directory: &ChunkDirectory,
        manifest: &FileTransferManifest,
    ) -> io::Result<()> {
        directory.verify_root()?;
        if directory.identity != self.root
            || Identity::of(&self.directory.metadata()?) != self.root
            || Identity::of(&directory.directory.metadata()?) != self.root
            || Snapshot::private_file(&self.file.metadata()?, manifest.byte_length())?
                != self.snapshot
        {
            return Err(invalid());
        }
        let name = assembled_name(self.binding);
        let visible = open_at(
            &self.directory,
            OsStr::new(&name),
            libc::O_RDONLY | libc::O_NONBLOCK,
            0,
        )?;
        if Snapshot::private_file(&visible.metadata()?, manifest.byte_length())? != self.snapshot
            || Snapshot::private_file(&self.file.metadata()?, manifest.byte_length())?
                != self.snapshot
        {
            return Err(invalid());
        }
        directory.verify_root()
    }

    /// Positional reads have an invocation-owned cursor. They never seek or alter the original
    /// descriptor's shared file offset, including when several jobs borrow the same proof.
    pub(super) fn reader(&self) -> ReadAt<'_> {
        ReadAt {
            file: &self.file,
            position: 0,
        }
    }
}

pub(super) struct ReadAt<'a> {
    file: &'a File,
    position: u64,
}
impl Read for ReadAt<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.file.read_at(buffer, self.position)?;
        if count > buffer.len() {
            return Err(invalid());
        }
        self.position = self
            .position
            .checked_add(count as u64)
            .ok_or_else(invalid)?;
        Ok(count)
    }
}

#[cfg(test)]
#[path = "assembled_proof_tests.rs"]
mod tests;
