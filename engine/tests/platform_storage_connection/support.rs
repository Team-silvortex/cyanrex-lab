use super::{
    catalog_fixture::{within, Fixture},
    expectation_helpers::Pins,
};
use cyanrex_engine::platform_host::{
    inspect_platform_artifact_root, inspect_platform_database, inspect_platform_storage_markers,
    PlatformArtifactRootObservation, PlatformCatalogObservation, PlatformDatabaseConnector,
    PlatformDatabaseEndpoint, PlatformHostPreflightConfig, PlatformStorageConnectionError,
    PlatformStorageConnector, PlatformStorageMarkerObservation,
};
use sqlx_core::{connection::Connection, query::query, row::Row};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::{self, Permissions},
    io,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub(super) const PASSWORD: &str = "synthetic-platform-catalog-fixture-only";
#[derive(Clone, Copy)]
pub(super) enum Mode {
    Observed,
    Expected,
}

struct Directory(PathBuf);
impl Directory {
    fn cleanup(&self) -> io::Result<()> {
        match fs::remove_dir_all(&self.0) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.cleanup().is_err() {
            eprintln!("owned storage connection root cleanup failed");
        }
    }
}

#[derive(PartialEq, Eq)]
struct RootState {
    identity: (u64, u64, u32, u32, u32),
    bytes: Vec<u8>,
}
fn root_state(path: &Path) -> io::Result<RootState> {
    let metadata = fs::symlink_metadata(path)?;
    Ok(RootState {
        identity: (
            metadata.dev(),
            metadata.ino(),
            metadata.uid(),
            metadata.gid(),
            metadata.mode(),
        ),
        bytes: fs::read(path.join("synthetic-unpublished-marker.bin"))?,
    })
}
fn parent_state() -> (Vec<String>, i32) {
    let status = fs::read_to_string("/proc/thread-self/status").unwrap();
    let names = [
        "Uid:", "Gid:", "Groups:", "CapPrm:", "CapEff:", "CapInh:", "CapAmb:", "CapBnd:",
    ];
    let identity = status
        .lines()
        .filter(|line| names.iter().any(|name| line.starts_with(name)))
        .map(str::to_owned)
        .collect();
    // SAFETY: PR_GET_NO_NEW_PRIVS reads only this calling thread's flag.
    let nnp = unsafe {
        libc::prctl(
            libc::PR_GET_NO_NEW_PRIVS,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
        )
    };
    (identity, nnp)
}
fn directory(path: &Path) -> io::Result<()> {
    fs::DirBuilder::new().mode(0o700).create(path)?;
    fs::set_permissions(path, Permissions::from_mode(0o700))
}

pub(super) struct Root {
    directory: Directory,
    root: PathBuf,
    saved: PathBuf,
    original: RootState,
    parent: (Vec<String>, i32),
    environment: BTreeMap<OsString, OsString>,
}
impl Root {
    pub(super) fn new() -> Self {
        let parent = PathBuf::from("/tmp").join(format!(
            "cyanrex-storage-connection-{}",
            Uuid::new_v4().simple()
        ));
        fs::DirBuilder::new().mode(0o700).create(&parent).unwrap();
        let guard = Directory(parent.clone());
        fs::set_permissions(&parent, Permissions::from_mode(0o700)).unwrap();
        let root = parent.join("artifacts");
        directory(&root).unwrap();
        let marker = root.join("synthetic-unpublished-marker.bin");
        fs::write(
            &marker,
            b"synthetic unpublished root; not a stored Artifact\n",
        )
        .unwrap();
        fs::set_permissions(&marker, Permissions::from_mode(0o400)).unwrap();
        Self {
            original: root_state(&root).unwrap(),
            directory: guard,
            root,
            saved: parent.join("artifacts-original"),
            parent: parent_state(),
            environment: std::env::vars_os().collect(),
        }
    }
    pub(super) fn path(&self) -> &Path {
        &self.root
    }
    pub(super) fn original(
        &self,
        config: &PlatformHostPreflightConfig,
    ) -> PlatformArtifactRootObservation {
        inspect_platform_artifact_root(config, &self.root)
            .unwrap_or_else(|_| panic!("owned original root unavailable"))
    }
    pub(super) fn swap(&self) -> bool {
        if fs::rename(&self.root, &self.saved).is_err() {
            return false;
        }
        if directory(&self.root).is_err() {
            return false;
        }
        fs::write(
            self.root.join("different-synthetic-root.bin"),
            b"not the original root\n",
        )
        .is_ok()
    }
    pub(super) fn restore(&self) -> bool {
        if !self.saved.exists() {
            return true;
        }
        if self.root.exists() && fs::remove_dir_all(&self.root).is_err() {
            return false;
        }
        fs::rename(&self.saved, &self.root).is_ok()
    }
    pub(super) fn finish(self) -> bool {
        let unchanged = root_state(&self.root).is_ok_and(|current| current == self.original)
            && parent_state() == self.parent
            && std::env::vars_os().collect::<BTreeMap<_, _>>() == self.environment;
        let cleaned = self.directory.cleanup().is_ok();
        unchanged && cleaned
    }
}

pub(super) fn endpoint(fixture: &Fixture) -> PlatformDatabaseEndpoint {
    let options = fixture.runtime().connect_options();
    match options.get_socket() {
        Some(path) => PlatformDatabaseEndpoint::unix(path, options.get_port()),
        None => PlatformDatabaseEndpoint::loopback(
            options.get_host().parse().unwrap(),
            options.get_port(),
        ),
    }
    .unwrap_or_else(|_| panic!("owned explicit local endpoint unavailable"))
}

pub(super) async fn catalog_and_markers(
    fixture: &Fixture,
) -> (PlatformCatalogObservation, PlatformStorageMarkerObservation) {
    let mut connection = within(fixture.runtime().acquire()).await.unwrap();
    let catalog = within(inspect_platform_database(fixture.config(), &mut connection))
        .await
        .unwrap();
    let markers = within(inspect_platform_storage_markers(
        fixture.config(),
        &mut connection,
        &catalog,
    ))
    .await
    .unwrap();
    drop(connection);
    (catalog, markers)
}

pub(super) async fn binding(
    fixture: &Fixture,
    root: &Root,
    mode: Mode,
) -> Result<PlatformStorageConnector, PlatformStorageConnectionError> {
    let (catalog, markers) = catalog_and_markers(fixture).await;
    let database = match mode {
        Mode::Observed => PlatformDatabaseConnector::new(
            fixture.config().clone(),
            catalog,
            endpoint(fixture),
            PASSWORD,
        ),
        Mode::Expected => PlatformDatabaseConnector::with_catalog_expectation(
            fixture.config().clone(),
            Pins::read(fixture).await.expectation(fixture).unwrap(),
            endpoint(fixture),
            PASSWORD,
        ),
    }
    .unwrap_or_else(|_| panic!("owned original database connector unavailable"));
    PlatformStorageConnector::new(
        fixture.config().clone(),
        database,
        markers,
        root.path(),
        root.original(fixture.config()),
    )
}

pub(super) async fn attempt(
    binding: &PlatformStorageConnector,
) -> Result<(), PlatformStorageConnectionError> {
    match within(binding.connect()).await {
        Ok(connection) => {
            within(connection.close()).await.unwrap();
            Ok(())
        }
        Err(error) => Err(error),
    }
}

pub(super) async fn no_connections(fixture: &Fixture) -> bool {
    super::expectation_helpers::no_connections(fixture).await
}

pub(super) async fn pid(
    fixture: &Fixture,
    connection: &mut sqlx_postgres::PgConnection,
) -> (i32, bool) {
    let row = within(query("SELECT pg_catalog.pg_backend_pid() AS pid,
        current_user::text=$1 AND session_user::text=$1 AS role, current_database()::text=$2 AS database")
        .bind(&fixture.role).bind(&fixture.database).fetch_one(connection)).await.unwrap();
    (
        row.get("pid"),
        row.get::<bool, _>("role") && row.get::<bool, _>("database"),
    )
}

pub(super) async fn marker_state(fixture: &Fixture) -> String {
    within(
        query(
            "SELECT md5(concat_ws('|', c.oid::text, c.relowner::text,
        coalesce(c.relacl::text,''), c.relrowsecurity::text, c.relforcerowsecurity::text,
        (SELECT string_agg(a.attname::text || ':' || a.atttypid::text || ':' || a.attnotnull::text,
            '|' ORDER BY a.attnum) FROM pg_catalog.pg_attribute a
            WHERE a.attrelid=c.oid AND a.attnum>0 AND NOT a.attisdropped))) AS digest
        FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
        WHERE n.nspname=$1 AND c.relname='collaboration_auth_source_schema'",
        )
        .bind(&fixture.source)
        .fetch_one(fixture.owner()),
    )
    .await
    .unwrap()
    .get("digest")
}
