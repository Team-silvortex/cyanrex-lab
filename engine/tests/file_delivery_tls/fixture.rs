pub use crate::legacy_fixture::{chunk_name, grant, output_name, scope, upload, TOKEN};
pub use crate::reply::{Kind, Reply};
pub use crate::server::{Hold, HoldAt, TlsObservation};
use crate::{certificates::Certificates, server::TlsServer, *};
use axum::http::{HeaderMap, Method, Uri};
use futures_util::FutureExt;
use serde::Deserialize;
use std::{
    fs,
    future::Future,
    io,
    net::Ipv4Addr,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    panic::AssertUnwindSafe,
    path::{Path, PathBuf},
    pin::Pin,
};

pub const CHOSEN_LEAF: &str = "host-chosen-tls-delivery.bin";
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManifestQuery {
    pub grant_id: FileGrantId,
    pub upload_id: FileUploadId,
    pub file_id: FileId,
    pub version_id: FileVersionId,
    pub byte_length: u64,
    pub sha256: Sha256Digest,
}
#[derive(Debug)]
pub enum Selected {
    Manifest(ManifestQuery),
    Chunk(crate::legacy_fixture::RequestQuery),
}
#[derive(Debug)]
pub struct ObservedRequest {
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
    pub selected: Result<Selected, String>,
    pub staged_before_request: Vec<bytes::Entry>,
}
struct DirectoryGuard(PathBuf);
impl Drop for DirectoryGuard {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            if error.kind() != io::ErrorKind::NotFound {
                eprintln!("exclusive native TLS fixture cleanup failed: {error}");
            }
        }
    }
}
pub struct Fixture {
    pub origin: String,
    pub root: PathBuf,
    pub selected: PathBuf,
    pub staging: ChunkStaging,
    pub manifest: FileTransferManifest,
    pub ca: Vec<u8>,
    server: TlsServer,
    cleanup: DirectoryGuard,
}
impl Fixture {
    pub async fn new(length: u64, variant: u128) -> Self {
        Self::at_ip(length, variant, Ipv4Addr::LOCALHOST).await
    }
    pub async fn at_ip(length: u64, variant: u128, ip: Ipv4Addr) -> Self {
        let directory =
            std::env::temp_dir().join(format!("cyanrex-delivery-tls-{}", Uuid::new_v4()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let cleanup = DirectoryGuard(directory.clone());
        let root = directory.join("chunks");
        let selected = directory.join("selected");
        for path in [&root, &selected] {
            fs::DirBuilder::new().mode(0o700).create(path).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        fs::set_permissions(&selected, fs::Permissions::from_mode(0o755)).unwrap();
        let staging = ChunkStaging::open(scope(), &root, 1).unwrap();
        let manifest = bytes::manifest(length, variant);
        let certificates = Certificates::generate(&directory);
        let server = TlsServer::start(&certificates, manifest.clone(), root.clone(), ip);
        Self {
            origin: server.origin.clone(),
            root,
            selected,
            staging,
            manifest,
            ca: certificates.ca,
            server,
            cleanup,
        }
    }
    pub fn client(&self, trusted: bool) -> FileChunkClient {
        FileChunkClient::with_config(
            &self.origin,
            self.staging.clone(),
            FileChunkClientConfig {
                capacity: 1,
                trusted_root_pem: trusted.then(|| self.ca.clone()),
                ..FileChunkClientConfig::default()
            },
        )
        .unwrap()
    }
    pub fn target(&self) -> Result<FileSaveTarget, FileSaveError> {
        FileSaveTarget::open_user_directory(&self.selected, CHOSEN_LEAF)
    }
    pub fn hold(&self, kind: Kind, at: HoldAt) -> Hold {
        self.server.hold(kind, at)
    }
    pub fn set_reply(&self, kind: Kind, reply: Reply) {
        self.server.set_reply(kind, reply);
    }
    pub fn take_requests(&self) -> Vec<ObservedRequest> {
        self.server.take_requests()
    }
    pub fn received_bytes(&self) -> usize {
        self.server.received_bytes()
    }
    pub fn finish_server(&self) -> TlsObservation {
        self.server.finish()
    }
    async fn cleanup(self) {
        self.finish_server();
        // No TLS process/thread or pipe is alive when certificate material is removed.
        let removed = fs::remove_dir_all(&self.cleanup.0);
        drop(self);
        removed.expect("remove only the stopped fixture's exclusive native TLS root");
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.finish();
    }
}
pub async fn checked<T, F>(f: Fixture, work: F) -> T
where
    F: for<'a> FnOnce(&'a Fixture) -> Pin<Box<dyn Future<Output = T> + 'a>>,
{
    let outcome = AssertUnwindSafe(work(&f)).catch_unwind().await;
    f.cleanup().await;
    match outcome {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

#[derive(Debug)]
pub struct Output {
    pub mode: u32,
    pub owner: u32,
    pub gid: u32,
    pub links: u64,
    pub length: u64,
    pub same_visible_inode: bool,
    pub verified: Result<(), String>,
    pub observed_length: u64,
    pub pattern_matches: bool,
    pub digest: String,
}
pub fn observe_output(path: &Path, manifest: &FileTransferManifest) -> io::Result<Output> {
    let original = crate::legacy_fixture::observe_output(path, manifest)?;
    let file = fs::File::open(path)?;
    let opened = file.metadata()?;
    let visible = fs::symlink_metadata(path)?;
    Ok(Output {
        mode: original.mode,
        owner: original.owner,
        gid: opened.gid(),
        links: original.links,
        length: original.length,
        same_visible_inode: original.same_visible_inode
            && (opened.dev(), opened.ino()) == (visible.dev(), visible.ino()),
        verified: original.verified,
        observed_length: original.observed_length,
        pattern_matches: original.pattern_matches,
        digest: original.digest,
    })
}
