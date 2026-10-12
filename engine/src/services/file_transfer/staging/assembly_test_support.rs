use super::{binding, ChunkStaging, FileTransferManifest};
use crate::{
    models::collaboration::{FileVersionRef, WorkspaceRef},
    services::file_transfer::{inspect_stream, verify_file},
};
use std::{
    fs::{self, Metadata},
    io::Cursor,
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    task::{Wake, Waker},
    time::Duration,
};

pub(super) struct Root(pub(super) PathBuf);
impl Root {
    pub(super) fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("cyanrex-assembly-private-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }

    pub(super) fn identity(&self) -> (u64, u64, u32) {
        let metadata = fs::metadata(&self.0).unwrap();
        (metadata.dev(), metadata.ino(), metadata.mode() & 0o7777)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone)]
pub(super) struct Barrier(Arc<(Mutex<bool>, Condvar)>);
impl Barrier {
    pub(super) fn new() -> Self {
        Self(Arc::new((Mutex::new(false), Condvar::new())))
    }
    pub(super) fn wait(&self) {
        let (lock, condition) = &*self.0;
        let mut released = lock.lock().unwrap();
        while !*released {
            released = condition.wait(released).unwrap();
        }
    }
    pub(super) fn release(&self) {
        let (lock, condition) = &*self.0;
        *lock.lock().unwrap() = true;
        condition.notify_all();
    }
}
pub(super) struct ReleaseOnDrop(pub(super) Barrier);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}

struct Noop;
impl Wake for Noop {
    fn wake(self: Arc<Self>) {}
}
pub(super) fn waker() -> Waker {
    Waker::from(Arc::new(Noop))
}

pub(super) fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap()
}

pub(super) fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: uuid::Uuid::from_u128(1).try_into().unwrap(),
        workspace_id: uuid::Uuid::from_u128(2).try_into().unwrap(),
    }
}

pub(super) fn staged(
    runtime: &tokio::runtime::Runtime,
    root: &Root,
    bytes: &[u8],
) -> (ChunkStaging, FileTransferManifest) {
    let staging = ChunkStaging::open(scope(), &root.0, 1).unwrap();
    let manifest = inspect_stream(
        scope(),
        uuid::Uuid::from_u128(3).try_into().unwrap(),
        uuid::Uuid::from_u128(4).try_into().unwrap(),
        "assembly.bin",
        &mut Cursor::new(bytes),
    )
    .unwrap();
    for index in 0..manifest.chunk_sha256().len() {
        let spec = manifest.chunk(index as u32).unwrap();
        let part = bytes[spec.offset as usize..(spec.offset + spec.byte_length) as usize].to_vec();
        runtime
            .block_on(staging.stage_chunk(&manifest, index as u32, Cursor::new(part)))
            .unwrap();
    }
    (staging, manifest)
}

pub(super) fn output(root: &Root, manifest: &FileTransferManifest) -> PathBuf {
    root.0.join(binding::assembled_name(
        binding::manifest_digest(manifest).unwrap(),
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Stamp {
    device: u64,
    inode: u64,
    length: u64,
    mode: u32,
    links: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}
impl Stamp {
    pub(super) fn of(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            length: metadata.len(),
            mode: metadata.mode() & 0o7777,
            links: metadata.nlink(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        }
    }
}

pub(super) fn sources(root: &Root, manifest: &FileTransferManifest) -> Vec<(Stamp, Vec<u8>)> {
    (0..manifest.chunk_sha256().len())
        .map(|index| {
            let path = root
                .0
                .join(binding::chunk_name(manifest, index as u32).unwrap());
            (
                Stamp::of(&fs::metadata(&path).unwrap()),
                fs::read(path).unwrap(),
            )
        })
        .collect()
}

pub(super) fn restored(runtime: &tokio::runtime::Runtime, staging: &ChunkStaging) {
    let permit = runtime.block_on(async {
        tokio::time::timeout(
            Duration::from_secs(10),
            staging.inner.workers.clone().acquire_owned(),
        )
        .await
        .unwrap()
        .unwrap()
    });
    drop(permit);
}

pub(super) fn verified_output(
    root: &Root,
    manifest: &FileTransferManifest,
    bytes: &[u8],
    original_inode: u64,
) {
    let path = output(root, manifest);
    let metadata = fs::metadata(&path).unwrap();
    assert_eq!(metadata.ino(), original_inode);
    assert_eq!(metadata.len(), bytes.len() as u64);
    assert_eq!(metadata.mode() & 0o7777, 0o400);
    assert_eq!(metadata.nlink(), 1);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    verify_file(manifest, &mut fs::File::open(path).unwrap()).unwrap();
}

pub(super) fn healthy(runtime: &tokio::runtime::Runtime, root: &Root, bytes: &[u8]) {
    let (staging, manifest) = staged(runtime, root, bytes);
    let before = sources(root, &manifest);
    let record = runtime
        .block_on(staging.assemble_staged_file(&manifest))
        .unwrap();
    assert_eq!(record.file(), &FileVersionRef::from_manifest(&manifest));
    let path = output(root, &manifest);
    verified_output(root, &manifest, bytes, fs::metadata(path).unwrap().ino());
    assert_eq!(sources(root, &manifest), before);
}
