use cyanrex_engine::{
    models::collaboration::WorkspaceRef,
    platform_host::{PlatformDatabaseTarget, PlatformHostPreflightConfig, PlatformNamespaces},
    services::auth_service::durable_source::SessionStockLimits,
};
use std::{
    ffi::OsString,
    fs::{self, Metadata, Permissions},
    io,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const SENTINEL: &[u8] = b"synthetic artifact root marker; not a stored Artifact or credential\n";

struct DirectoryGuard(PathBuf);

impl DirectoryGuard {
    fn cleanup(&self) -> io::Result<()> {
        fn restore(path: &Path) -> io::Result<()> {
            let metadata = match fs::symlink_metadata(path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(error),
            };
            // Never follow a substituted symlink, including while restoring test permissions.
            if metadata.is_dir() {
                fs::set_permissions(path, Permissions::from_mode(0o700))?;
                for entry in fs::read_dir(path)? {
                    restore(&entry?.path())?;
                }
            }
            Ok(())
        }
        restore(&self.0)?;
        match fs::remove_dir_all(&self.0) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }
}

impl Drop for DirectoryGuard {
    fn drop(&mut self) {
        if self.cleanup().is_err() {
            eprintln!("owned platform Artifact root fixture cleanup failed");
        }
    }
}

#[derive(PartialEq, Eq)]
pub(super) struct MetadataSnapshot {
    identity: (u64, u64),
    ownership: (u32, u32),
    mode: u32,
    links: u64,
    length: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

impl From<Metadata> for MetadataSnapshot {
    fn from(metadata: Metadata) -> Self {
        Self {
            identity: (metadata.dev(), metadata.ino()),
            ownership: (metadata.uid(), metadata.gid()),
            mode: metadata.mode(),
            links: metadata.nlink(),
            length: metadata.len(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        }
    }
}

#[derive(PartialEq, Eq)]
pub(super) struct Snapshot {
    parent: MetadataSnapshot,
    root: MetadataSnapshot,
    sentinel: MetadataSnapshot,
    names: Vec<OsString>,
    bytes: Vec<u8>,
}

pub(super) struct Fixture {
    pub(super) config: PlatformHostPreflightConfig,
    pub(super) root: PathBuf,
    sentinel: PathBuf,
    guard: DirectoryGuard,
}

impl Fixture {
    fn new() -> Self {
        // A literal /tmp root, not an environment-selected or actual user directory.
        let directory = PathBuf::from("/tmp").join(format!(
            "cyanrex-platform-artifact-root-{}",
            Uuid::new_v4().simple()
        ));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap_or_else(|_| panic!("exclusive synthetic fixture parent creation failed"));
        let guard = DirectoryGuard(directory.clone());
        fs::set_permissions(&directory, Permissions::from_mode(0o700))
            .unwrap_or_else(|_| panic!("synthetic parent mode setup failed"));
        let root = directory.join("private-artifacts");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap_or_else(|_| panic!("synthetic Artifact root creation failed"));
        fs::set_permissions(&root, Permissions::from_mode(0o700))
            .unwrap_or_else(|_| panic!("synthetic Artifact root mode setup failed"));
        let sentinel = root.join("synthetic-unpublished-marker.bin");
        fs::write(&sentinel, SENTINEL)
            .unwrap_or_else(|_| panic!("synthetic nonempty root setup failed"));
        fs::set_permissions(&sentinel, Permissions::from_mode(0o400))
            .unwrap_or_else(|_| panic!("synthetic marker mode setup failed"));
        let scope = WorkspaceRef {
            authority_id: Uuid::from_u128(6401).try_into().unwrap(),
            workspace_id: Uuid::from_u128(6402).try_into().unwrap(),
        };
        let config = PlatformHostPreflightConfig::new(
            "https://platform.example",
            scope,
            PlatformNamespaces::new("source_private", "task_private", "artifacts_private").unwrap(),
            PlatformDatabaseTarget::new("platform_db", "platform_runtime", 123, 456).unwrap(),
            SessionStockLimits::new(2, 8).unwrap(),
        )
        .unwrap();
        Self {
            config,
            root,
            sentinel,
            guard,
        }
    }

    pub(super) fn snapshot(&self) -> Snapshot {
        let metadata = |path: &Path| {
            MetadataSnapshot::from(
                fs::symlink_metadata(path)
                    .unwrap_or_else(|_| panic!("independent synthetic filesystem snapshot failed")),
            )
        };
        let mut names: Vec<_> = fs::read_dir(&self.root)
            .unwrap_or_else(|_| panic!("synthetic root entries unavailable"))
            .map(|entry| {
                entry
                    .unwrap_or_else(|_| panic!("synthetic entry snapshot failed"))
                    .file_name()
            })
            .collect();
        names.sort();
        Snapshot {
            parent: metadata(&self.guard.0),
            root: metadata(&self.root),
            sentinel: metadata(&self.sentinel),
            names,
            bytes: fs::read(&self.sentinel)
                .unwrap_or_else(|_| panic!("independent synthetic marker read failed")),
        }
    }

    pub(super) fn directory(&self) -> &Path {
        &self.guard.0
    }

    pub(super) fn marker(&self) -> &Path {
        &self.sentinel
    }

    pub(super) fn checked<T>(case: impl FnOnce(&Self) -> T) -> T {
        let fixture = Self::new();
        let result = catch_unwind(AssertUnwindSafe(|| case(&fixture)));
        let cleaned = fixture.guard.cleanup().is_ok();
        drop(fixture);
        assert!(
            cleaned,
            "owned platform Artifact root fixture must clean up before assertions"
        );
        match result {
            Ok(value) => value,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }
}

pub(super) fn fingerprint(path: &Path) -> MetadataSnapshot {
    fs::symlink_metadata(path)
        .unwrap_or_else(|_| panic!("synthetic entry metadata unavailable"))
        .into()
}

pub(super) fn mode(path: &Path, bits: u32) {
    fs::set_permissions(path, Permissions::from_mode(bits))
        .unwrap_or_else(|_| panic!("synthetic entry mode setup failed"));
}

pub(super) fn directory(path: &Path) {
    fs::DirBuilder::new()
        .mode(0o700)
        .create(path)
        .unwrap_or_else(|_| panic!("owned synthetic child directory creation failed"));
    mode(path, 0o700);
}
