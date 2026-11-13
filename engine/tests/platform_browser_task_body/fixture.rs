pub(super) use super::metadata_fixture::{
    counter, exact_session, revoke, sessions, task_id, token_hash, within, Fixture, ADMIN_TITLE,
};
use super::{
    certificates::Certificates, child::Running, expectation_helpers::Pins,
    kernel::clear_child_capabilities, marker_helpers::MarkerPins, metadata_fixture,
};
use cyanrex_engine::models::collaboration::ArtifactRef;
use serde_json::{json, Value};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    process::{Command, Stdio},
};

pub(super) const SAVED_TITLE: &str = "Revised collaborative body handouts";
pub(super) const SAVED_NAMES: [&str; 2] = ["共同教学正文.txt", "협업자료.txt"];
pub(super) const SAVED_LANGUAGES: [&str; 2] = ["teaching-notes-v3", "future-handout-v9"];
pub(super) const SAVED_TEXTS: [&str; 2] = [
    "Revised collaborative handout\r\n共同教学正文😀\r\nNot executable code.",
    "Second collaborative handout\n한국어 공동 자료😀\nNo Run is required.",
];
pub(super) const CONFLICT_TITLE: &str = "Private unsaved competing body edit";
pub(super) const CONFLICT_NAME: &str = "冲突未提交正文.txt";
pub(super) const CONFLICT_LANGUAGE: &str = "handout-conflict-v4";
pub(super) const CONFLICT_TEXT: &str = "Unsaved competing handout\r\n不得覆盖其他确认版本。";
pub(super) const REVOKED_TITLE: &str = "Private revoked-session body edit";
pub(super) const REVOKED_NAME: &str = "撤销未提交正文.txt";
pub(super) const REVOKED_LANGUAGE: &str = "handout-revoked-v5";
pub(super) const REVOKED_TEXT: &str = "Revoked private handout\r\n不得发布或重新展示。";

pub(super) struct Prepared<'a> {
    pub(super) base: metadata_fixture::Prepared<'a>,
    pub(super) initial_sessions: String,
    tls: Option<Certificates>,
    origin: String,
    path: PathBuf,
    original: Vec<u8>,
    material: Option<(Vec<u8>, Vec<u8>)>,
}
impl<'a> std::ops::Deref for Prepared<'a> {
    type Target = metadata_fixture::Prepared<'a>;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}
impl<'a> Prepared<'a> {
    pub(super) async fn new(f: &'a Fixture, tls: bool) -> Self {
        let base = metadata_fixture::Prepared::new(f).await;
        // Add only the three existing publication INSERTs to the unchanged read/row-lock,
        // login and Task manifest-replacement profile. No installer or product grant is used.
        for table in [
            "collaboration_artifacts",
            "collaboration_artifact_revisions",
            "collaboration_artifact_outbox",
        ] {
            f.sql(&format!(
                "GRANT INSERT ON {}.{table} TO {}",
                f.artifacts, f.role
            ))
            .await;
        }
        let origin = format!("{}://{}", if tls { "https" } else { "http" }, base.bind);
        let c = f.config();
        let options = f.runtime().connect_options();
        let transport = match options.get_socket() {
            Some(path) => json!({"kind":"unix","directory":path}),
            None => json!({"kind":"loopback","address":options.get_host()}),
        };
        let document = json!({"format_version":3,"origin":origin,"authority_id":c.scope().authority_id.as_uuid(),"workspace_id":c.scope().workspace_id.as_uuid(),"namespaces":{"source":f.source,"task":f.task,"artifacts":f.artifacts},"database":{"database":f.database,"role":f.role,"system_identifier":c.target().system_identifier(),"database_oid":c.target().database_oid(),"transport":transport,"port":options.get_port(),"password":super::storage_helpers::PASSWORD},"session_limits":{"per_account":c.limits().per_account(),"total":c.limits().total()},"artifact_root":base.base.base.root.path(),"catalog_identity":Pins::read(f).await.json(),"marker_identity":MarkerPins::read(f).await.json()});
        let original = serde_json::to_vec(&document).unwrap();
        let path = base
            .base
            .base
            .root
            .path()
            .parent()
            .unwrap()
            .join("browser-task-body-input.json");
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        file.write_all(&original).unwrap();
        drop(file);
        let tls = tls.then(|| Certificates::generate(path.parent().unwrap(), base.bind.ip()));
        let material = tls.as_ref().map(Certificates::bytes);
        Self {
            base,
            initial_sessions: sessions(f).await,
            tls,
            origin,
            path,
            original,
            material,
        }
    }
    pub(super) fn spawn(&self) -> Running {
        let identity = &self.identity;
        assert!(identity.safe_ids());
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
                if self.tls.is_some() {
                    "serve-loopback-tls"
                } else {
                    "serve-loopback"
                },
                "--uid",
                &identity.uid[0].to_string(),
                "--gid",
                &identity.gid[0].to_string(),
                "--groups",
                &groups,
                "--config",
            ])
            .arg(&self.path)
            .args(["--bind", &self.bind.to_string()]);
        if let Some(tls) = &self.tls {
            command
                .arg("--cert")
                .arg(&tls.certificate)
                .arg("--key")
                .arg(&tls.key);
        }
        command
            .args(["--task-text-publication", "--session-ui"])
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
    pub(super) fn input(&self) -> Value {
        let mut config = self.base.base.browser_input();
        let extra = json!({"origin":self.origin,"tls":self.tls.is_some(),
            "ca_file":self.tls.as_ref().map(|tls|tls.certificate.parent().unwrap().join("ca.pem")),
            "saved_title":SAVED_TITLE,"saved_names":SAVED_NAMES,"saved_languages":SAVED_LANGUAGES,
            "saved_texts":SAVED_TEXTS,"conflict_title":CONFLICT_TITLE,"conflict_name":CONFLICT_NAME,
            "conflict_language":CONFLICT_LANGUAGE,"conflict_text":CONFLICT_TEXT,"admin_title":ADMIN_TITLE,
            "revoked_title":REVOKED_TITLE,"revoked_name":REVOKED_NAME,
            "revoked_language":REVOKED_LANGUAGE,"revoked_text":REVOKED_TEXT});
        config
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        config
    }
    pub(super) fn blob(&self, reference: &ArtifactRef) -> PathBuf {
        self.base.base.base.blob(reference)
    }
    pub(super) fn finish(self) -> bool {
        let unchanged = fs::read(&self.path).is_ok_and(|value| value == self.original)
            && self
                .tls
                .as_ref()
                .zip(self.material.as_ref())
                .is_none_or(|(tls, bytes)| tls.unchanged(bytes));
        unchanged && self.base.finish()
    }
}
