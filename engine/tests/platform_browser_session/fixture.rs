pub(super) use super::{
    catalog_fixture::{within, Fixture},
    login_fixture::{otp, password, session_count, username, watermark},
};
use super::{
    certificates::Certificates,
    child::{self, Running},
    kernel::{clear_child_capabilities, Identity},
    plain_fixture,
};
use reqwest::Client;
use serde_json::json;
use sqlx_core::{query::query, row::Row};
use std::{
    fs,
    net::SocketAddr,
    os::unix::fs::MetadataExt,
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

pub(super) struct Prepared<'a> {
    pub(super) base: plain_fixture::Prepared<'a>,
    pub(super) tls: Option<Certificates>,
    pub(super) origin: String,
    path: PathBuf,
    original: Vec<u8>,
    material: Option<(Vec<u8>, Vec<u8>)>,
}
impl<'a> Prepared<'a> {
    pub(super) async fn new(f: &'a Fixture, tls: bool) -> Self {
        let base = plain_fixture::Prepared::new(f).await;
        let origin = format!("{}://{}", if tls { "https" } else { "http" }, base.bind);
        let mut document = base.document.clone();
        document["origin"] = origin.clone().into();
        let path = base.variant("browser-input.json", &document);
        let original = fs::read(&path).unwrap();
        let tls = tls.then(|| Certificates::generate(path.parent().unwrap(), base.bind.ip()));
        let material = tls.as_ref().map(Certificates::bytes);
        Self {
            base,
            tls,
            origin,
            path,
            original,
            material,
        }
    }
    pub(super) fn bind(&self) -> SocketAddr {
        self.base.bind
    }
    pub(super) fn identity(&self) -> &Identity {
        &self.base.identity
    }
    pub(super) fn spawn(&self, ui: bool) -> Running {
        let i = self.identity();
        assert!(i.safe_ids());
        let groups = if i.groups.is_empty() {
            "none".to_owned()
        } else {
            i.groups
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",")
        };
        let mut command = Command::new(env!("CARGO_BIN_EXE_cyanrex-platform"));
        command
            .args([
                if self.tls.is_some() {
                    "serve-loopback-tls"
                } else {
                    "serve-loopback"
                },
                "--uid",
                &i.uid[0].to_string(),
                "--gid",
                &i.gid[0].to_string(),
                "--groups",
                &groups,
                "--config",
            ])
            .arg(&self.path)
            .args(["--bind", &self.bind().to_string()]);
        if let Some(tls) = &self.tls {
            command
                .arg("--cert")
                .arg(&tls.certificate)
                .arg("--key")
                .arg(&tls.key);
        }
        if ui {
            command.arg("--session-ui");
        }
        command
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
        clear_child_capabilities(&mut command);
        Running::spawn(&mut command)
    }
    pub(super) fn client(&self) -> Client {
        let mut builder = Client::builder()
            .http1_only()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(5));
        if let Some(tls) = &self.tls {
            builder = builder
                .https_only(true)
                .tls_built_in_root_certs(false)
                .add_root_certificate(reqwest::Certificate::from_pem(&tls.ca).unwrap());
        }
        builder.build().unwrap()
    }
    pub(super) fn browser_input(&self) -> serde_json::Value {
        json!({"origin":self.origin,"username":username().as_str(),"password":password(),"tls":self.tls.is_some(),"ca_file":self.tls.as_ref().map(|tls|tls.certificate.parent().unwrap().join("ca.pem")),"uncertain_logout":self.tls.is_none()})
    }
    pub(super) fn files(&self) -> Vec<(String, u64, u64, u32, u64, Vec<u8>)> {
        let mut files = fs::read_dir(self.base.root().path())
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                let m = fs::symlink_metadata(e.path()).unwrap();
                (
                    e.file_name().into_string().unwrap(),
                    m.dev(),
                    m.ino(),
                    m.mode(),
                    m.nlink(),
                    fs::read(e.path()).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        files.sort_by(|a, b| a.0.cmp(&b.0));
        files
    }
    pub(super) fn finish(self) -> bool {
        let unchanged = fs::read(&self.path).is_ok_and(|v| v == self.original)
            && self
                .tls
                .as_ref()
                .zip(self.material.as_ref())
                .is_none_or(|(tls, bytes)| tls.unchanged(bytes));
        unchanged && self.base.finish()
    }
}
pub(super) async fn resources(f: &Fixture) -> Vec<(String, String)> {
    let rows=within(query("SELECT n.nspname AS schema,c.relname AS name FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=ANY($1) AND c.relkind='r' AND c.relname<>'sessions' ORDER BY n.nspname,c.relname").bind(vec![f.source.clone(),f.task.clone(),f.artifacts.clone()]).fetch_all(f.owner())).await.unwrap();
    let mut values = Vec::new();
    for row in rows {
        let schema: String = row.get("schema");
        let table: String = row.get("name");
        let name = format!("{schema}.{table}");
        let row = if schema == f.source && table == "users" {
            "to_jsonb(t)-'otp_last_counter'"
        } else {
            "to_jsonb(t)"
        };
        let digest:String=within(query(&format!("SELECT md5(coalesce(string_agg(({row})::text,'|' ORDER BY ({row})::text),'')) AS digest FROM {name} t")).fetch_one(f.owner())).await.unwrap().get("digest");
        values.push((name, digest));
    }
    values
}
pub(super) async fn exact_session(f: &Fixture, digest: &str, present: bool) {
    assert!(
        digest.len() == 64
            && digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "browser IPC must contain a digest, never a raw Cookie"
    );
    let found:bool=within(query(&format!("SELECT EXISTS(SELECT 1 FROM {0}.sessions s JOIN {0}.users u USING(username,account_id) WHERE s.token=$1 AND s.username=$2 AND s.account_id='00000000-0000-0000-0000-000000008211' AND s.expires_at>clock_timestamp()) AS found",f.source)).bind(digest).bind(username().as_str()).fetch_one(f.owner())).await.unwrap().get("found");
    assert_eq!(found, present);
    assert_eq!(session_count(f).await, i64::from(present));
}
pub(super) async fn no_ui(p: &Prepared<'_>) {
    let mut host = p.spawn(false);
    assert!(host.ready().await);
    let client = p.client();
    for path in [
        "/platform/",
        "/platform/ui/app.js",
        "/platform/ui/labels.js",
        "/platform/ui/style.css",
        "/platform/ui/tasks.js",
        "/platform/ui/task-contract.js",
        "/platform/ui/task-edit.js",
    ] {
        let response = client
            .get(format!("{}{path}", p.origin))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 404);
        assert!(!response.headers().contains_key("set-cookie"));
    }
    assert!(child::stopped(&host.stop().await));
}
