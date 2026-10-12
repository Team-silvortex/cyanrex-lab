use super::*;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read, Seek, SeekFrom},
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
};

pub fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: "cb4eb60e-80d7-49b6-a1ae-2a4518c9c998".parse().unwrap(),
        workspace_id: "f21b4b85-c3c3-40c7-a7b2-04732e80bda9".parse().unwrap(),
    }
}
pub fn chunk_name(manifest: &FileTransferManifest, index: u32) -> String {
    format!(
        "v1-{:x}-{index}",
        Sha256::digest(serde_json::to_vec(manifest).unwrap())
    )
}
pub fn output_name(manifest: &FileTransferManifest) -> String {
    format!(
        "f1-{:x}",
        Sha256::digest(serde_json::to_vec(manifest).unwrap())
    )
}
pub struct Fixture {
    pub directory: PathBuf,
    pub root: PathBuf,
    pub source: PathBuf,
    pub staging: ChunkStaging,
    pub manifest: FileTransferManifest,
    cleanup: DirectoryGuard,
}

struct DirectoryGuard(PathBuf);

impl Drop for DirectoryGuard {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            if error.kind() != io::ErrorKind::NotFound {
                eprintln!("exclusive assembly fixture cleanup failed: {error}");
            }
        }
    }
}
impl Fixture {
    pub async fn staged(length: u64) -> Self {
        let directory = std::env::temp_dir().join(format!("cyanrex-assembly-{}", Uuid::new_v4()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let cleanup = DirectoryGuard(directory.clone());
        let root = directory.join("chunks");
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let staging = ChunkStaging::open(scope(), &root, 1).unwrap();
        let source = directory.join("original-source.bin");
        let manifest = bytes::manifest(length, 1);
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&source)
            .unwrap();
        io::copy(&mut bytes::Pattern::new(0, length), &mut file).unwrap();
        drop(file);
        for index in 0..manifest.chunk_sha256().len() as u32 {
            let spec = manifest.chunk(index).unwrap();
            let mut reader = fs::File::open(&source).unwrap();
            reader.seek(SeekFrom::Start(spec.offset)).unwrap();
            staging
                .stage_chunk(&manifest, index, reader.take(spec.byte_length))
                .await
                .unwrap();
        }
        Self {
            directory,
            root,
            source,
            staging,
            manifest,
            cleanup,
        }
    }
    pub fn cleanup(self) {
        drop(self.staging);
        fs::remove_dir_all(&self.directory)
            .expect("remove only this exclusive synthetic assembly root");
        drop(self.cleanup);
    }

    pub async fn stage_manifest(&self, manifest: &FileTransferManifest) {
        for index in 0..manifest.chunk_sha256().len() as u32 {
            let spec = manifest.chunk(index).unwrap();
            self.staging
                .stage_chunk(
                    manifest,
                    index,
                    bytes::Pattern::new(spec.offset, spec.byte_length),
                )
                .await
                .unwrap();
        }
    }
}

pub fn changed(
    manifest: &FileTransferManifest,
    field: &str,
    value: serde_json::Value,
) -> FileTransferManifest {
    let mut raw = serde_json::to_value(manifest).unwrap();
    raw[field] = value;
    FileTransferManifest::parse_json(&serde_json::to_vec(&raw).unwrap()).unwrap()
}

pub fn fifo(path: &Path) {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let path = CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: path is a NUL-terminated name in this unique synthetic fixture; mode is valid.
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
}

pub async fn healthy(f: &Fixture) {
    let result = f.staging.assemble_staged_file(&f.manifest).await.unwrap();
    assert_eq!(result.file(), &FileVersionRef::from_manifest(&f.manifest));
    let output = observe_output(&f.root.join(output_name(&f.manifest)), &f.manifest).unwrap();
    assert_eq!(output.length, f.manifest.byte_length());
    assert_eq!(output.mode & 0o7777, 0o400);
    assert_eq!(output.links, 1);
    assert!(output.same_visible_inode);
    assert!(output.pattern_matches);
    assert_eq!(output.observed_length, f.manifest.byte_length());
    assert_eq!(output.digest, f.manifest.sha256().to_string());
    output.verified.unwrap();
}

#[derive(Debug)]
pub struct Output {
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
pub fn observe_output(path: &Path, manifest: &FileTransferManifest) -> io::Result<Output> {
    let visible = fs::symlink_metadata(path)?;
    if !visible.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "assembly output is not an ordinary file",
        ));
    }
    let mut file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    let verified = verify_file(manifest, &mut file).map_err(|error| error.to_string());
    // Independent streamed byte oracle and digest, without trusting assembly metadata or
    // reusing an assembler-provided descriptor. At most one overflow byte is inspected.
    let mut reader = fs::File::open(path)?.take(manifest.byte_length() + 1);
    let mut buffer = [0; FILE_TRANSFER_BUFFER_BYTES];
    let mut hash = Sha256::new();
    let mut offset = 0;
    let mut pattern_matches = true;
    loop {
        let amount = reader.read(&mut buffer)?;
        if amount == 0 {
            break;
        }
        for (position, value) in buffer[..amount].iter().enumerate() {
            pattern_matches &= *value == bytes::byte(offset + position as u64);
        }
        hash.update(&buffer[..amount]);
        offset += amount as u64;
    }
    Ok(Output {
        mode: metadata.mode(),
        owner: metadata.uid(),
        links: metadata.nlink(),
        length: metadata.len(),
        same_visible_inode: metadata.dev() == visible.dev() && metadata.ino() == visible.ino(),
        verified,
        observed_length: offset,
        pattern_matches,
        digest: format!("{:x}", hash.finalize()),
    })
}
