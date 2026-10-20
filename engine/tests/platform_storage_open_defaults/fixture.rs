use cyanrex_engine::{
    models::collaboration::WorkspaceRef,
    platform_host::{
        inspect_platform_artifact_root, PlatformArtifactRootObservation,
        PlatformCatalogExpectation, PlatformDatabaseEndpoint, PlatformDatabaseTarget,
        PlatformHostPreflightConfig, PlatformNamespaces, PlatformStorageMarkerExpectation,
    },
    services::auth_service::durable_source::SessionStockLimits,
};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::net::TcpListener;
use uuid::Uuid;

pub(super) const PASSWORD: &str = "public-synthetic-storage-open-password-only";

pub(super) fn config(change: usize) -> PlatformHostPreflightConfig {
    PlatformHostPreflightConfig::new(
        if change == 1 {
            "https://other-platform.example"
        } else {
            "https://platform.example"
        },
        WorkspaceRef {
            authority_id: Uuid::from_u128(if change == 2 { 5201 } else { 4201 })
                .try_into()
                .unwrap(),
            workspace_id: Uuid::from_u128(if change == 3 { 5202 } else { 4202 })
                .try_into()
                .unwrap(),
        },
        PlatformNamespaces::new(
            if change == 4 {
                "other_source"
            } else {
                "source_private"
            },
            if change == 5 {
                "other_task"
            } else {
                "task_private"
            },
            if change == 6 {
                "other_artifacts"
            } else {
                "artifacts_private"
            },
        )
        .unwrap(),
        PlatformDatabaseTarget::new(
            if change == 7 {
                "other_db"
            } else {
                "platform_db"
            },
            if change == 8 {
                "other_runtime"
            } else {
                "platform_runtime"
            },
            if change == 9 { 1123 } else { 123 },
            if change == 10 { 1456 } else { 456 },
        )
        .unwrap(),
        SessionStockLimits::new(
            if change == 11 { 3 } else { 2 },
            if change == 12 { 16 } else { 8 },
        )
        .unwrap(),
    )
    .unwrap()
}
pub(super) fn expected(config: PlatformHostPreflightConfig) -> PlatformStorageMarkerExpectation {
    PlatformStorageMarkerExpectation::new(
        PlatformCatalogExpectation::new(config, 1001, 1002, [2001, 2002, 2003], [1002; 3]).unwrap(),
        [3001, 3002, 3003, 3004, 3005],
        [1002; 5],
    )
    .unwrap()
}
pub(super) async fn listener() -> TcpListener {
    TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap()
}
pub(super) fn endpoint(listener: &TcpListener) -> PlatformDatabaseEndpoint {
    PlatformDatabaseEndpoint::loopback(
        std::net::Ipv4Addr::LOCALHOST.into(),
        listener.local_addr().unwrap().port(),
    )
    .unwrap()
}
pub(super) async fn no_dial(listener: &TcpListener) -> bool {
    match tokio::time::timeout(Duration::from_millis(100), listener.accept()).await {
        Err(_) => true,
        Ok(Ok((peer, _))) => {
            drop(peer);
            false
        }
        _ => false,
    }
}
pub(super) fn environment() -> BTreeMap<OsString, OsString> {
    std::env::vars_os().collect()
}
fn process() -> Vec<String> {
    let names = [
        "Uid:",
        "Gid:",
        "Groups:",
        "CapPrm:",
        "CapEff:",
        "CapInh:",
        "CapAmb:",
        "CapBnd:",
        "NoNewPrivs:",
    ];
    fs::read_to_string("/proc/thread-self/status")
        .unwrap()
        .lines()
        .filter(|line| names.iter().any(|name| line.starts_with(name)))
        .map(str::to_owned)
        .collect()
}
fn directory(path: &Path) {
    fs::DirBuilder::new().mode(0o700).create(path).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
fn identity(path: &Path) -> (u64, u64, u32, u32, u32) {
    let metadata = fs::symlink_metadata(path).unwrap();
    (
        metadata.dev(),
        metadata.ino(),
        metadata.uid(),
        metadata.gid(),
        metadata.mode(),
    )
}

pub(super) struct Root {
    parent: PathBuf,
    root: PathBuf,
    saved: PathBuf,
    original: (u64, u64, u32, u32, u32),
    before_process: Vec<String>,
    before_environment: BTreeMap<OsString, OsString>,
}
impl Root {
    pub(super) fn new() -> Self {
        Self::new_in(Path::new("/tmp"))
    }
    pub(super) fn new_in(base: &Path) -> Self {
        let parent = base.join(format!(
            "cyanrex-storage-open-defaults-{}",
            Uuid::new_v4().simple()
        ));
        directory(&parent);
        let root = parent.join("artifacts");
        directory(&root);
        fs::write(
            root.join("synthetic-unpublished.bin"),
            b"not a stored Artifact\n",
        )
        .unwrap();
        fs::set_permissions(
            root.join("synthetic-unpublished.bin"),
            fs::Permissions::from_mode(0o400),
        )
        .unwrap();
        Self {
            original: identity(&root),
            before_process: process(),
            before_environment: environment(),
            saved: parent.join("original-artifacts"),
            parent,
            root,
        }
    }
    pub(super) fn path(&self) -> &Path {
        &self.root
    }
    pub(super) fn original(
        &self,
        config: &PlatformHostPreflightConfig,
    ) -> PlatformArtifactRootObservation {
        inspect_platform_artifact_root(config, self.path())
            .unwrap_or_else(|_| panic!("owned original root unavailable"))
    }
    pub(super) fn swap(&self) {
        fs::rename(&self.root, &self.saved).unwrap();
        directory(&self.root);
        fs::write(
            self.root.join("replacement.bin"),
            b"synthetic replacement\n",
        )
        .unwrap();
    }
    pub(super) fn restore(&self) {
        if self.saved.exists() {
            fs::remove_dir_all(&self.root).unwrap();
            fs::rename(&self.saved, &self.root).unwrap();
        }
    }
    pub(super) fn finish(self) -> bool {
        let unchanged = identity(self.path()) == self.original
            && process() == self.before_process
            && environment() == self.before_environment
            && fs::read(self.root.join("synthetic-unpublished.bin")).unwrap()
                == b"not a stored Artifact\n";
        unchanged && fs::remove_dir_all(&self.parent).is_ok()
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        if self.parent.exists() {
            let _ = fs::remove_dir_all(&self.parent);
        }
    }
}
