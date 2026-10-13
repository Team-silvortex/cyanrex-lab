use crate::*;
use futures_util::FutureExt;
use sha2::{Digest, Sha256};
use std::{
    fs,
    future::Future,
    io::{self, Read},
    os::unix::fs::{DirBuilderExt, MetadataExt},
    panic::AssertUnwindSafe,
    path::{Path, PathBuf},
    pin::Pin,
    time::Duration,
};

pub const CHOSEN_LEAF: &str = "chosen-user.bin";
pub fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: Uuid::from_u128(1201).try_into().unwrap(),
        workspace_id: Uuid::from_u128(1202).try_into().unwrap(),
    }
}
pub fn output_name(manifest: &FileTransferManifest) -> String {
    format!(
        "f1-{:x}",
        Sha256::digest(serde_json::to_vec(manifest).unwrap())
    )
}

struct DirectoryGuard(PathBuf);
impl Drop for DirectoryGuard {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            if error.kind() != io::ErrorKind::NotFound {
                eprintln!("exclusive local save fixture cleanup failed: {error}");
            }
        }
    }
}
pub struct Fixture {
    pub source_root: PathBuf,
    pub target_root: PathBuf,
    pub store: ChunkStaging,
    pub target: FileSaveTarget,
    pub manifest: FileTransferManifest,
    cleanup: DirectoryGuard,
}
impl Fixture {
    pub fn new() -> Self {
        Self::with_workers(FILE_TRANSFER_CHUNK_BYTES + 17, 1)
    }
    pub fn with_length(length: u64) -> Self {
        Self::with_workers(length, 1)
    }
    pub fn with_workers(length: u64, workers: usize) -> Self {
        let directory = std::env::temp_dir().join(format!("cyanrex-file-save-{}", Uuid::new_v4()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let cleanup = DirectoryGuard(directory.clone());
        let source_root = directory.join("source");
        let target_root = directory.join("selected-host-parent");
        for root in [&source_root, &target_root] {
            fs::DirBuilder::new().mode(0o700).create(root).unwrap();
        }
        let store = ChunkStaging::open(scope(), &source_root, workers).unwrap();
        let target = FileSaveTarget::open(&target_root, CHOSEN_LEAF).unwrap();
        let manifest = bytes::manifest(length, 1);
        Self {
            source_root,
            target_root,
            store,
            target,
            manifest,
            cleanup,
        }
    }
    pub fn source_path(&self) -> PathBuf {
        self.source_root.join(output_name(&self.manifest))
    }
    pub fn target_path(&self) -> PathBuf {
        self.target_root.join(CHOSEN_LEAF)
    }
    pub fn directory(&self) -> &Path {
        &self.cleanup.0
    }
    pub fn target_for(&self, leaf: &str) -> FileSaveTarget {
        FileSaveTarget::open(&self.target_root, leaf).unwrap()
    }
    pub async fn assemble(&self) -> AssembledFile {
        self.assemble_manifest(&self.manifest).await
    }
    pub async fn assemble_manifest(&self, manifest: &FileTransferManifest) -> AssembledFile {
        for index in 0..manifest.chunk_sha256().len() as u32 {
            let spec = manifest.chunk(index).unwrap();
            within(self.store.stage_chunk(
                manifest,
                index,
                bytes::Pattern::new(spec.offset, spec.byte_length),
            ))
            .await
            .expect("fixture must actually stage every independently patterned source chunk");
        }
        within(self.store.assemble_staged_file(manifest))
            .await
            .expect("fixture must obtain this invocation's actual original private whole-file ACK")
    }
    fn cleanup(self) {
        drop(self.store);
        drop(self.target);
        let removed = fs::remove_dir_all(&self.cleanup.0);
        drop(self.cleanup);
        removed.expect("remove only this exclusive local save fixture directory");
    }
}
pub async fn within<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(20), future)
        .await
        .expect("bounded actual local file job must finish")
}
pub async fn checked<T, F>(fixture: Fixture, work: F) -> T
where
    F: for<'a> FnOnce(&'a Fixture) -> Pin<Box<dyn Future<Output = T> + 'a>>,
{
    let outcome = AssertUnwindSafe(work(&fixture)).catch_unwind().await;
    fixture.cleanup();
    match outcome {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
pub fn root_identity(path: &Path) -> (u64, u64, u32, u32) {
    let metadata = fs::metadata(path).unwrap();
    (
        metadata.dev(),
        metadata.ino(),
        metadata.uid(),
        metadata.mode(),
    )
}
pub struct Observed {
    pub identity: (u64, u64),
    pub mode: u32,
    pub owner: u32,
    pub links: u64,
    pub length: u64,
    pub same_visible_inode: bool,
    pub verified: Result<(), String>,
    pub observed_length: u64,
    pub pattern_matches: bool,
    pub digest: String,
}
pub fn observe(path: &Path, manifest: &FileTransferManifest) -> io::Result<Observed> {
    let mut file = fs::File::open(path)?;
    let opened = file.metadata()?;
    let visible = fs::symlink_metadata(path)?;
    let verified = verify_file(manifest, &mut file).map_err(|error| error.to_string());
    let mut file = fs::File::open(path)?;
    let mut buffer = [0; FILE_TRANSFER_BUFFER_BYTES];
    let mut hash = Sha256::new();
    let mut offset = 0;
    let mut pattern_matches = true;
    loop {
        let amount = file.read(&mut buffer)?;
        if amount == 0 {
            break;
        }
        for (index, byte) in buffer[..amount].iter().enumerate() {
            pattern_matches &= *byte == bytes::byte(offset + index as u64);
        }
        hash.update(&buffer[..amount]);
        offset += amount as u64;
    }
    Ok(Observed {
        identity: (opened.dev(), opened.ino()),
        mode: opened.mode(),
        owner: opened.uid(),
        links: opened.nlink(),
        length: opened.len(),
        same_visible_inode: opened.is_file()
            && visible.is_file()
            && (opened.dev(), opened.ino()) == (visible.dev(), visible.ino()),
        verified,
        observed_length: offset,
        pattern_matches,
        digest: format!("{:x}", hash.finalize()),
    })
}
