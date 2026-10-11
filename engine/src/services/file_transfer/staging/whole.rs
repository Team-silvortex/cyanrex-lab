//! Current whole-file observations only, never original write receipts or recovery grants.
use super::{
    binding::bound_chunk_name,
    directory::ChunkDirectory,
    filesystem::{invalid, open_at, Identity, Snapshot},
    ChunkStagingError, FileTransferManifest,
};
use std::{
    ffi::OsStr,
    fs::File,
    io::{self, Read},
};

/// Opaque and move-only. Metadata equality cannot construct a successful byte observation.
pub(crate) struct BoundFileObservation {
    key: [u8; 32],
    binding: [u8; 32],
    root: Identity,
    chunks: Vec<ObservedChunk>,
}

struct ObservedChunk {
    name: String,
    original: Snapshot,
}

struct OpenChunk {
    file: File,
    name: String,
    original: Snapshot,
    byte_length: u64,
    remaining: u64,
    index: u32,
}

// The existing stream verifier owns the sole 64 KiB byte buffer. This reader exposes each
// complete file in manifest order, and checks its exact EOF and original metadata at boundaries.
struct OrderedChunks<'a, O> {
    directory: &'a ChunkDirectory,
    key: [u8; 32],
    manifest: &'a FileTransferManifest,
    next: usize,
    current: Option<OpenChunk>,
    observed: Vec<ObservedChunk>,
    observer: O,
}

impl<O: FnMut(u32)> OrderedChunks<'_, O> {
    fn open_next(&mut self) -> io::Result<()> {
        if self.next == self.manifest.chunk_sha256().len() {
            return Ok(());
        }
        let index = self.next as u32;
        let spec = self.manifest.chunk(index).map_err(|_| invalid())?;
        let (name, _) = bound_chunk_name(self.key, self.manifest, index)?;
        let file = open_readonly(self.directory, &name)?;
        let original = Snapshot::private(&file.metadata()?, spec.byte_length)?;
        self.current = Some(OpenChunk {
            file,
            name,
            original,
            byte_length: spec.byte_length,
            remaining: spec.byte_length,
            index,
        });
        self.next += 1;
        Ok(())
    }

    fn finish_current(&mut self) -> io::Result<()> {
        let Some(mut current) = self.current.take() else {
            return Ok(());
        };
        if current.remaining != 0 {
            return Err(invalid());
        }
        let mut sentinel = [0];
        // One exact per-file EOF probe, never silently skipping appended bytes at boundaries.
        if current.file.read(&mut sentinel)? != 0 {
            return Err(invalid());
        }
        let after = Snapshot::private(&current.file.metadata()?, current.byte_length)?;
        let visible = open_readonly(self.directory, &current.name)?;
        let visible = Snapshot::private(&visible.metadata()?, current.byte_length)?;
        if current.original != after || after != visible {
            return Err(invalid());
        }
        self.observed.push(ObservedChunk {
            name: current.name,
            original: current.original,
        });
        (self.observer)(current.index);
        Ok(())
    }
}

impl<O: FnMut(u32)> Read for OrderedChunks<'_, O> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        if self
            .current
            .as_ref()
            .is_some_and(|current| current.remaining == 0)
        {
            self.finish_current()?;
        }
        if self.current.is_none() {
            self.open_next()?;
        }
        let Some(current) = self.current.as_mut() else {
            return Ok(0);
        };
        let requested = (bytes.len() as u64).min(current.remaining) as usize;
        let count = current.file.read(&mut bytes[..requested])?;
        if count == 0 || count > requested {
            return Err(invalid());
        }
        current.remaining -= count as u64;
        Ok(count)
    }
}

fn open_readonly(directory: &ChunkDirectory, name: &str) -> io::Result<File> {
    // Nonblocking open makes a substituted FIFO rejectable without waiting for a writer.
    open_at(
        &directory.directory,
        OsStr::new(name),
        libc::O_RDONLY | libc::O_NONBLOCK,
        0,
    )
}

fn recheck_metadata(
    directory: &ChunkDirectory,
    manifest: &FileTransferManifest,
    observation: &BoundFileObservation,
) -> io::Result<()> {
    if directory.identity != observation.root
        || observation.chunks.len() != manifest.chunk_sha256().len()
    {
        return Err(invalid());
    }
    directory.verify_root()?;
    for (index, chunk) in observation.chunks.iter().enumerate() {
        let length = manifest
            .chunk(index as u32)
            .map_err(|_| invalid())?
            .byte_length;
        let file = open_readonly(directory, &chunk.name)?;
        let before = Snapshot::private(&file.metadata()?, length)?;
        let visible = open_readonly(directory, &chunk.name)?;
        let visible = Snapshot::private(&visible.metadata()?, length)?;
        let after = Snapshot::private(&file.metadata()?, length)?;
        if before != chunk.original || before != visible || before != after {
            return Err(invalid());
        }
    }
    directory.verify_root()
}

pub(super) fn observe(
    directory: &ChunkDirectory,
    key: [u8; 32],
    manifest: &FileTransferManifest,
) -> Result<BoundFileObservation, ChunkStagingError> {
    observe_inner(directory, key, manifest, |_| {})
}

// The generic private callback has no public injection surface. Production supplies an inert
// callback; unit tests use it only to prove real in-job dispatch and deterministic scan drift.
fn observe_inner<O: FnMut(u32)>(
    directory: &ChunkDirectory,
    key: [u8; 32],
    manifest: &FileTransferManifest,
    observer: O,
) -> Result<BoundFileObservation, ChunkStagingError> {
    let operation = || -> io::Result<BoundFileObservation> {
        directory.verify_root()?;
        // Index zero is only a fixed digest domain here, including for an empty manifest.
        let (_, binding) = bound_chunk_name(key, manifest, 0)?;
        let mut reader = OrderedChunks {
            directory,
            key,
            manifest,
            next: 0,
            current: None,
            observed: Vec::with_capacity(manifest.chunk_sha256().len()),
            observer,
        };
        super::super::verify_stream(manifest, &mut reader).map_err(|_| invalid())?;
        let observation = BoundFileObservation {
            key,
            binding,
            root: directory.identity,
            chunks: reader.observed,
        };
        // Earlier entries can change while later chunks are read. Recheck all original snapshots
        // after the complete chunk/whole digest pass, without keeping one descriptor per chunk.
        recheck_metadata(directory, manifest, &observation)?;
        Ok(observation)
    };
    operation().map_err(|_| ChunkStagingError::Unavailable)
}

pub(super) fn recheck(
    directory: &ChunkDirectory,
    key: [u8; 32],
    manifest: &FileTransferManifest,
    observation: BoundFileObservation,
) -> Result<(), ChunkStagingError> {
    let (_, binding) =
        bound_chunk_name(key, manifest, 0).map_err(|_| ChunkStagingError::InvalidInput)?;
    if observation.key != key || observation.binding != binding {
        return Err(ChunkStagingError::InvalidInput);
    }
    recheck_metadata(directory, manifest, &observation).map_err(|_| ChunkStagingError::Unavailable)
}

/// Returns the same original observation after a metadata check, never a replacement observation.
pub(super) fn recheck_retained(
    directory: &ChunkDirectory,
    key: [u8; 32],
    manifest: &FileTransferManifest,
    observation: BoundFileObservation,
) -> Result<BoundFileObservation, ChunkStagingError> {
    let (_, binding) =
        bound_chunk_name(key, manifest, 0).map_err(|_| ChunkStagingError::InvalidInput)?;
    if observation.key != key || observation.binding != binding {
        return Err(ChunkStagingError::InvalidInput);
    }
    recheck_metadata(directory, manifest, &observation)
        .map_err(|_| ChunkStagingError::Unavailable)?;
    Ok(observation)
}

#[cfg(test)]
pub(super) fn observe_with<O: FnMut(u32)>(
    directory: &ChunkDirectory,
    key: [u8; 32],
    manifest: &FileTransferManifest,
    observer: O,
) -> Result<BoundFileObservation, ChunkStagingError> {
    observe_inner(directory, key, manifest, observer)
}

#[cfg(test)]
#[path = "whole_tests.rs"]
mod tests;
