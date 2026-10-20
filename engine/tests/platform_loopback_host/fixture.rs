pub(super) use super::{
    catalog_fixture::{within, Fixture},
    child::{failed, killed_before_ready, safe_threads, stopped, Running},
    login_fixture::{otp, password, read_only, session_count, snapshot, username, watermark},
    storage_helpers::no_connections,
};
use super::{
    expectation_helpers::Pins,
    kernel::{clear_child_capabilities, Identity},
    marker_helpers::MarkerPins,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx_core::{query::query, row::Row};
use std::{
    ffi::OsStr,
    fs::{self, OpenOptions, Permissions},
    io::{self, Write},
    net::{SocketAddr, TcpListener},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

struct Directory(PathBuf);
impl Directory {
    fn cleanup(&self) -> bool {
        match fs::remove_dir_all(&self.0) {
            Ok(()) => true,
            Err(e) if e.kind() == io::ErrorKind::NotFound => true,
            Err(_) => false,
        }
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        self.cleanup();
    }
}

pub(super) struct Prepared<'a> {
    inner: super::login_fixture::Prepared<'a>,
    directory: Directory,
    path: PathBuf,
    original: Vec<u8>,
    pub(super) document: Value,
    pub(super) bind: SocketAddr,
    pub(super) origin: String,
    pub(super) identity: Identity,
}
impl<'a> Prepared<'a> {
    pub(super) async fn new(owner: &'a Fixture) -> Self {
        let inner = super::login_fixture::Prepared::new(owner).await;
        owner
            .sql(&format!(
                "GRANT DELETE ON {}.sessions TO {}",
                owner.source, owner.role
            ))
            .await;
        let directory = std::env::temp_dir().join(format!(
            "cyanrex-loopback-input-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        fs::set_permissions(&directory, Permissions::from_mode(0o700)).unwrap();
        let directory = Directory(directory);
        let reserve = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let bind = reserve.local_addr().unwrap();
        drop(reserve);
        let origin = format!("http://{bind}");
        let catalog = Pins::read(owner).await;
        let markers = MarkerPins::read(owner).await;
        let options = owner.runtime().connect_options();
        let transport = match options.get_socket() {
            Some(path) => json!({"kind":"unix","directory":path}),
            None => json!({"kind":"loopback","address":options.get_host()}),
        };
        let c = owner.config();
        let document = json!({"format_version":3,"origin":origin,
            "authority_id":c.scope().authority_id.as_uuid(),"workspace_id":c.scope().workspace_id.as_uuid(),
            "namespaces":{"source":owner.source,"task":owner.task,"artifacts":owner.artifacts},
            "database":{"database":owner.database,"role":owner.role,"system_identifier":c.target().system_identifier(),
                "database_oid":c.target().database_oid(),"transport":transport,"port":options.get_port(),
                "password":super::storage_helpers::PASSWORD},
            "session_limits":{"per_account":c.limits().per_account(),"total":c.limits().total()},
            "artifact_root":inner.root().path(),"catalog_identity":catalog.json(),"marker_identity":markers.json()});
        let original = serde_json::to_vec(&document).unwrap();
        let path = directory.0.join("input.json");
        write(&path, &original);
        Self {
            inner,
            directory,
            path,
            original,
            document,
            bind,
            origin,
            identity: Identity::current(),
        }
    }
    pub(super) fn root(&self) -> &super::storage_helpers::Root {
        self.inner.root()
    }
    pub(super) fn variant(&self, name: &str, document: &Value) -> PathBuf {
        let path = self.directory.0.join(name);
        write(&path, &serde_json::to_vec(document).unwrap());
        path
    }
    pub(super) fn spawn(&self, path: Option<&Path>) -> Running {
        self.spawn_environment(path, None)
    }
    // This shared plain-host fixture is also compiled by the TLS notification cohort.
    #[allow(dead_code)]
    pub(super) fn spawn_notify(&self, path: Option<&Path>, socket: &OsStr) -> Running {
        self.spawn_environment(path, Some(socket))
    }
    fn spawn_environment(&self, path: Option<&Path>, socket: Option<&OsStr>) -> Running {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cyanrex-platform"));
        let groups = if self.identity.groups.is_empty() {
            "none".to_owned()
        } else {
            self.identity
                .groups
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",")
        };
        command
            .args([
                "serve-loopback",
                "--uid",
                &self.identity.uid[0].to_string(),
                "--gid",
                &self.identity.gid[0].to_string(),
                "--groups",
                &groups,
                "--config",
            ])
            .arg(path.unwrap_or(&self.path))
            .args(["--bind", &self.bind.to_string()])
            .current_dir(&self.directory.0)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (name, _) in std::env::vars_os() {
            if name.as_encoded_bytes().starts_with(b"PG") {
                command.env_remove(name);
            }
        }
        command
            .env_remove("DATABASE_URL")
            .env_remove("NOTIFY_SOCKET");
        if let Some(socket) = socket {
            command.arg("--notify-systemd").env("NOTIFY_SOCKET", socket);
        }
        clear_child_capabilities(&mut command);
        Running::spawn(&mut command)
    }
    pub(super) fn finish(self) -> bool {
        let stable = fs::read(&self.path).is_ok_and(|v| v == self.original)
            && Identity::current() == self.identity;
        let root = self.inner.finish();
        let cleaned = self.directory.cleanup();
        stable && root && cleaned
    }
}
fn write(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    fs::set_permissions(path, Permissions::from_mode(0o600)).unwrap();
}
pub(super) async fn no_listener(bind: SocketAddr) -> bool {
    tokio::time::timeout(
        std::time::Duration::from_millis(150),
        tokio::net::TcpStream::connect(bind),
    )
    .await
    .is_ok_and(|result| result.is_err())
}

pub(super) async fn stored_digest(f: &Fixture, token: &str) -> bool {
    let digest = format!("{:x}", Sha256::digest(token.as_bytes()));
    within(query(&format!("SELECT count(*) FILTER (WHERE token=$1 AND username=$2 AND account_id='00000000-0000-0000-0000-000000008211' AND expires_at>clock_timestamp() AND expires_at<clock_timestamp()+interval '13 hours')=1 AND count(*) FILTER (WHERE token=$3)=0 AS exact FROM {}.sessions",f.source))
        .bind(digest).bind(username().as_str()).bind(token).fetch_one(f.owner()))
        .await.unwrap().get("exact")
}
