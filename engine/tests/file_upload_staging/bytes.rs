use super::fixture::scope;
use cyanrex_engine::{
    models::collaboration::*, services::file_transfer::FILE_TRANSFER_BUFFER_BYTES,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read},
    os::unix::fs::MetadataExt,
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

pub fn byte(offset: u64) -> u8 {
    ((offset % 251 + (offset / FILE_TRANSFER_CHUNK_BYTES) * 73) % 251) as u8
}

pub struct Pattern {
    start: u64,
    length: u64,
    consumed: u64,
    pub calls: Arc<AtomicUsize>,
}

impl Pattern {
    pub fn new(start: u64, length: u64) -> Self {
        Self {
            start,
            length,
            consumed: 0,
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Read for Pattern {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        assert!(buffer.len() <= FILE_TRANSFER_BUFFER_BYTES);
        self.calls.fetch_add(1, Ordering::SeqCst);
        let amount = (self.length - self.consumed).min(buffer.len() as u64) as usize;
        for (index, value) in buffer[..amount].iter_mut().enumerate() {
            *value = byte(self.start + self.consumed + index as u64);
        }
        self.consumed += amount as u64;
        Ok(amount)
    }
}

pub fn manifest(length: u64, variant: u128) -> FileTransferManifest {
    let mut whole = Sha256::new();
    let mut chunks = Vec::new();
    let mut offset = 0;
    let mut buffer = [0; FILE_TRANSFER_BUFFER_BYTES];
    while offset < length {
        let end = (offset + FILE_TRANSFER_CHUNK_BYTES).min(length);
        let mut chunk = Sha256::new();
        while offset < end {
            let amount = (end - offset).min(buffer.len() as u64) as usize;
            for (index, value) in buffer[..amount].iter_mut().enumerate() {
                *value = byte(offset + index as u64);
            }
            whole.update(&buffer[..amount]);
            chunk.update(&buffer[..amount]);
            offset += amount as u64;
        }
        chunks.push(format!("{:x}", chunk.finalize()).parse().unwrap());
    }
    FileTransferManifest::new(
        scope(),
        uuid::Uuid::from_u128(variant + 400).try_into().unwrap(),
        uuid::Uuid::from_u128(variant + 500).try_into().unwrap(),
        "真实作业.bin",
        length,
        chunks,
        format!("{:x}", whole.finalize()).parse().unwrap(),
    )
    .unwrap()
}

#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: std::ffi::OsString,
    pub device: u64,
    pub inode: u64,
    pub mode: u32,
    pub links: u64,
    pub length: u64,
    pub modified: (i64, i64),
    pub changed: (i64, i64),
    pub digest: Option<String>,
    pub symlink: Option<std::path::PathBuf>,
}

pub fn snapshot(path: &Path) -> Entry {
    let metadata = fs::symlink_metadata(path).unwrap();
    let digest = if metadata.is_file() {
        let mut file = fs::File::open(path).unwrap();
        let mut hash = Sha256::new();
        let mut bytes = [0; FILE_TRANSFER_BUFFER_BYTES];
        loop {
            let amount = file.read(&mut bytes).unwrap();
            if amount == 0 {
                break;
            }
            hash.update(&bytes[..amount]);
        }
        Some(format!("{:x}", hash.finalize()))
    } else {
        None
    };
    Entry {
        name: path.file_name().unwrap().to_owned(),
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        links: metadata.nlink(),
        length: metadata.len(),
        modified: (metadata.mtime(), metadata.mtime_nsec()),
        changed: (metadata.ctime(), metadata.ctime_nsec()),
        digest,
        symlink: metadata
            .file_type()
            .is_symlink()
            .then(|| fs::read_link(path).unwrap()),
    }
}

pub fn tree(path: &Path) -> Vec<Entry> {
    let mut entries: Vec<_> = fs::read_dir(path)
        .unwrap()
        .map(|entry| snapshot(&entry.unwrap().path()))
        .collect();
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    entries
}
