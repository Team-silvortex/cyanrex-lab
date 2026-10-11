use cyanrex_engine::{
    models::collaboration::*,
    services::file_transfer::{
        staging::{ChunkStaging, ChunkStagingError},
        FILE_TRANSFER_BUFFER_BYTES,
    },
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions, Permissions},
    io::{self, Read},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};
use tokio::sync::oneshot;

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

pub fn digest(bytes: &[u8]) -> Sha256Digest {
    format!("{:x}", Sha256::digest(bytes)).parse().unwrap()
}

pub fn manifest(length: u64, variant: u128) -> FileTransferManifest {
    let mut whole = Sha256::new();
    let mut hashes = Vec::new();
    let mut offset = 0;
    let mut buffer = [0u8; FILE_TRANSFER_BUFFER_BYTES];
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
        hashes.push(format!("{:x}", chunk.finalize()).parse().unwrap());
    }
    FileTransferManifest::new(
        super::scope(),
        FileId::try_from(uuid::Uuid::from_u128(variant + 10)).unwrap(),
        FileVersionId::try_from(uuid::Uuid::from_u128(variant + 100)).unwrap(),
        "data.bin",
        length,
        hashes,
        format!("{:x}", whole.finalize()).parse().unwrap(),
    )
    .unwrap()
}

pub fn changed(
    original: &FileTransferManifest,
    key: &str,
    value: serde_json::Value,
) -> FileTransferManifest {
    let mut raw = serde_json::to_value(original).unwrap();
    raw[key] = value;
    FileTransferManifest::parse_json(&serde_json::to_vec(&raw).unwrap()).unwrap()
}

pub fn name(manifest: &FileTransferManifest, index: u32) -> String {
    let mut canonical = Vec::new();
    serde_json::to_writer(&mut canonical, manifest).unwrap();
    format!("v1-{}-{index}", digest(&canonical))
}

#[derive(Debug, PartialEq, Eq)]
pub struct EntrySnapshot {
    pub name: std::ffi::OsString,
    pub device: u64,
    pub inode: u64,
    pub mode: u32,
    pub owner: u32,
    pub links: u64,
    pub length: u64,
    pub modified: (i64, i64),
    pub changed: (i64, i64),
    pub content: Option<Sha256Digest>,
    pub symlink: Option<PathBuf>,
}

pub fn snapshot(path: &Path) -> EntrySnapshot {
    let metadata = fs::symlink_metadata(path).unwrap();
    let content = if metadata.is_file() {
        let mut file = File::open(path).unwrap();
        let mut whole = Sha256::new();
        let mut buffer = [0u8; FILE_TRANSFER_BUFFER_BYTES];
        loop {
            let amount = file.read(&mut buffer).unwrap();
            if amount == 0 {
                break;
            }
            whole.update(&buffer[..amount]);
        }
        Some(format!("{:x}", whole.finalize()).parse().unwrap())
    } else {
        None
    };
    EntrySnapshot {
        name: path.file_name().unwrap().to_owned(),
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        owner: metadata.uid(),
        links: metadata.nlink(),
        length: metadata.len(),
        modified: (metadata.mtime(), metadata.mtime_nsec()),
        changed: (metadata.ctime(), metadata.ctime_nsec()),
        content,
        symlink: metadata
            .file_type()
            .is_symlink()
            .then(|| fs::read_link(path).unwrap()),
    }
}

pub fn tree(path: &Path) -> Vec<EntrySnapshot> {
    let mut entries: Vec<_> = fs::read_dir(path)
        .unwrap()
        .map(|entry| snapshot(&entry.unwrap().path()))
        .collect();
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    entries
}

pub fn private_directory(path: &Path) {
    fs::create_dir(path).unwrap();
    fs::set_permissions(path, Permissions::from_mode(0o700)).unwrap();
}

pub struct Fixture {
    pub directory: PathBuf,
    pub root: PathBuf,
    pub store: ChunkStaging,
}

impl Fixture {
    pub fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("cyanrex-chunk-stage-{}", uuid::Uuid::new_v4()));
        private_directory(&directory);
        let root = directory.join("chunks");
        private_directory(&root);
        let store = ChunkStaging::open(super::scope(), &root, 1).unwrap();
        Self {
            directory,
            root,
            store,
        }
    }

    pub fn path(&self, manifest: &FileTransferManifest, index: u32) -> PathBuf {
        self.root.join(name(manifest, index))
    }

    pub fn real_file(&self, length: u64) -> PathBuf {
        let path = self.directory.join("source.bin");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        std::io::copy(&mut Pattern::new(0, length), &mut file).unwrap();
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).expect("remove this exclusive private fixture only");
    }
}

pub struct Release(Option<mpsc::Sender<()>>);

impl Drop for Release {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

pub struct HeldReader {
    pattern: Pattern,
    block_at: u64,
    entered: Option<oneshot::Sender<()>>,
    wait: mpsc::Receiver<()>,
    dropped: Option<oneshot::Sender<()>>,
}

impl Read for HeldReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.pattern.consumed >= self.block_at {
            if let Some(entered) = self.entered.take() {
                let _ = entered.send(());
                self.wait
                    .recv_timeout(Duration::from_secs(5))
                    .expect("held file worker not released");
            }
        }
        self.pattern.read(buffer)
    }
}

impl Drop for HeldReader {
    fn drop(&mut self) {
        if let Some(dropped) = self.dropped.take() {
            let _ = dropped.send(());
        }
    }
}

pub fn held(
    length: u64,
    block_at: u64,
) -> (
    HeldReader,
    Release,
    oneshot::Receiver<()>,
    oneshot::Receiver<()>,
) {
    let (release, wait) = mpsc::channel();
    let (entered, started) = oneshot::channel();
    let (dropped, finished) = oneshot::channel();
    (
        HeldReader {
            pattern: Pattern::new(0, length),
            block_at,
            entered: Some(entered),
            wait,
            dropped: Some(dropped),
        },
        Release(Some(release)),
        started,
        finished,
    )
}

pub async fn signal(receiver: oneshot::Receiver<()>) {
    tokio::time::timeout(Duration::from_secs(2), receiver)
        .await
        .expect("worker did not reach its explicit lifecycle barrier")
        .unwrap();
}

pub async fn fresh_stage_after_release(store: &ChunkStaging, manifest: &FileTransferManifest) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match store
                .stage_chunk(
                    manifest,
                    0,
                    Pattern::new(0, manifest.chunk(0).unwrap().byte_length),
                )
                .await
            {
                Err(ChunkStagingError::CapacityExhausted) => tokio::task::yield_now().await,
                result => {
                    assert_eq!(result, Ok(()));
                    break;
                }
            }
        }
    })
    .await
    .expect("released worker did not return staging capacity");
}
