pub use super::http_fixture::scope;
use super::*;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read},
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
};

pub struct Fixture {
    pub server: chunk_fixture::Fixture,
    pub client_root: PathBuf,
    pub client_staging: ChunkStaging,
}

impl Fixture {
    pub async fn ready() -> Self {
        let server = chunk_fixture::Fixture::ready(1).await;
        let client_root = server.sharing.base.directory.join("client-chunks");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&client_root)
            .unwrap();
        let client_staging = ChunkStaging::open(scope(), &client_root, 1).unwrap();
        Self {
            server,
            client_root,
            client_staging,
        }
    }

    pub async fn cleanup(self) {
        drop(self.client_staging);
        // The old fixture owns this exclusive temporary directory, including the client sibling.
        self.server.cleanup().await;
    }
}

pub fn legacy_name(manifest: &FileTransferManifest, index: u32) -> String {
    let canonical = serde_json::to_vec(manifest).unwrap();
    format!("v1-{:x}-{index}", Sha256::digest(canonical))
}

#[derive(Debug)]
pub struct ObservedChunk {
    pub mode: u32,
    pub links: u64,
    pub length: u64,
    pub same_path_inode: bool,
    pub verified: Result<(), String>,
    pub bytes: Vec<u8>,
}

pub fn observe_chunk(
    path: &Path,
    manifest: &FileTransferManifest,
    index: u32,
) -> io::Result<ObservedChunk> {
    let visible = fs::symlink_metadata(path)?;
    if !visible.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the client output is not an ordinary file",
        ));
    }
    // Open independently, rather than treating receiver metadata as proof of saved bytes.
    let mut file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    let verified = verify_chunk(manifest, index, &mut file).map_err(|error| error.to_string());
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(manifest.chunk(index).unwrap().byte_length + 1)
        .read_to_end(&mut bytes)?;
    Ok(ObservedChunk {
        mode: metadata.mode(),
        links: metadata.nlink(),
        length: metadata.len(),
        same_path_inode: metadata.dev() == visible.dev() && metadata.ino() == visible.ino(),
        verified,
        bytes,
    })
}
