pub(super) use super::{
    catalog_fixture::{within, Fixture},
    child::{failed, safe_threads, stopped, Running},
    login_fixture::{otp, password, read_only, session_count, snapshot, username, watermark},
    plain_fixture::{no_listener, stored_digest},
    storage_helpers::no_connections,
};
use super::{certificates::Certificates, plain_fixture};
use std::{
    ffi::OsStr,
    fs,
    net::{IpAddr, SocketAddr, TcpListener},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
pub(super) struct Prepared<'a> {
    base: plain_fixture::Prepared<'a>,
    pub(super) tls: Certificates,
    pub(super) bind: SocketAddr,
    pub(super) origin: String,
    pub(super) document: serde_json::Value,
    path: PathBuf,
    original: Vec<u8>,
    materials: (Vec<u8>, Vec<u8>),
}
impl<'a> Prepared<'a> {
    pub(super) async fn new(f: &'a Fixture, ip: IpAddr) -> Self {
        let base = plain_fixture::Prepared::new(f).await;
        let reserve = TcpListener::bind((ip, 0)).unwrap();
        let bind = reserve.local_addr().unwrap();
        drop(reserve);
        let origin = format!("https://{bind}");
        let mut document = base.document.clone();
        document["origin"] = origin.clone().into();
        let path = base.variant("https.json", &document);
        let original = fs::read(&path).unwrap();
        let tls = Certificates::generate(path.parent().unwrap(), ip);
        let materials = tls.bytes();
        Self {
            base,
            tls,
            bind,
            origin,
            document,
            path,
            original,
            materials,
        }
    }
    pub(super) fn root(&self) -> &super::storage_helpers::Root {
        self.base.root()
    }
    pub(super) fn identity(&self) -> &super::kernel::Identity {
        &self.base.identity
    }
    pub(super) fn variant(&self, name: &str, document: &serde_json::Value) -> PathBuf {
        self.base.variant(name, document)
    }
    pub(super) fn spawn(
        &self,
        config: Option<&Path>,
        cert: Option<&Path>,
        key: Option<&Path>,
    ) -> Running {
        self.spawn_environment(config, cert, key, None)
    }
    pub(super) fn spawn_notify(&self, config: Option<&Path>, socket: &OsStr) -> Running {
        self.spawn_environment(config, None, None, Some(socket))
    }
    fn spawn_environment(
        &self,
        config: Option<&Path>,
        cert: Option<&Path>,
        key: Option<&Path>,
        socket: Option<&OsStr>,
    ) -> Running {
        let identity = self.identity();
        let groups = if identity.groups.is_empty() {
            "none".to_owned()
        } else {
            identity
                .groups
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",")
        };
        let mut command = Command::new(env!("CARGO_BIN_EXE_cyanrex-platform"));
        command
            .args([
                "serve-loopback-tls",
                "--uid",
                &identity.uid[0].to_string(),
                "--gid",
                &identity.gid[0].to_string(),
                "--groups",
                &groups,
                "--config",
            ])
            .arg(config.unwrap_or(&self.path))
            .args(["--bind", &self.bind.to_string(), "--cert"])
            .arg(cert.unwrap_or(&self.tls.certificate))
            .arg("--key")
            .arg(key.unwrap_or(&self.tls.key))
            .current_dir(self.path.parent().unwrap())
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
        super::kernel::clear_child_capabilities(&mut command);
        Running::spawn(&mut command)
    }
    pub(super) fn finish(self) -> bool {
        let unchanged = fs::read(&self.path).is_ok_and(|v| v == self.original)
            && self.tls.unchanged(&self.materials);
        let base = self.base.finish();
        unchanged && base
    }
}
