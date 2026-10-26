pub(super) use super::content_fixture::{task_id, token_hash, within, Fixture};
use super::{
    child::Running,
    expectation_helpers::Pins,
    kernel::{clear_child_capabilities, Identity},
    marker_helpers::MarkerPins,
};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use cyanrex_engine::models::collaboration::{TaskContentManifest, TextPayloadBinding};
use serde_json::{json, Value};
use sqlx_core::{query::query, row::Row};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    net::{SocketAddr, TcpListener},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::PathBuf,
    process::{Command, Stdio},
    sync::OnceLock,
};

pub(super) const PASSWORD: &str = "public-synthetic-task-browser-password-only";
pub(super) const USER: &str = "index-owner-0";
pub(super) const FIRST_TEXT:&str="合作教学说明，不是程序源码。\r\n<script>globalThis.__privateTaskExecuted = 1</script>\n保留原样😀";
pub(super) const SECOND_TEXT: &str = "第二份教学材料\t한국어 notes\n无需代码或 Run。\n";
pub(super) const TEXT_TITLE: &str = "Two optional teaching text payloads";
pub(super) const EMPTY_TITLE: &str = "Teaching outline without payload";
pub(super) const FOREIGN_TITLE: &str = "Foreign private title must never appear";

pub(super) struct Prepared<'a> {
    pub(super) base: super::content_fixture::Prepared<'a>,
    pub(super) bind: SocketAddr,
    pub(super) origin: String,
    pub(super) identity: Identity,
    path: PathBuf,
    original: Vec<u8>,
}
impl<'a> Prepared<'a> {
    pub(super) async fn new(f: &'a Fixture) -> Self {
        let base = super::content_fixture::Prepared::new(f).await;
        static HASH: OnceLock<String> = OnceLock::new();
        let hash = HASH.get_or_init(|| {
            let salt = SaltString::encode_b64(b"synthetic-platform-login-salt").unwrap();
            Argon2::default()
                .hash_password(PASSWORD.as_bytes(), &salt)
                .unwrap()
                .to_string()
        });
        within(query(&format!("UPDATE {}.users SET password_salt=$1,password_hash=$2,totp_secret=$3 WHERE username=$4",f.source)).bind("synthetic-platform-login-salt").bind(hash).bind("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ").bind(USER).execute(f.owner())).await.unwrap();
        // Existing Session issuance profile only. Task/Artifact storage keeps its original
        // read/row-lock policy; trusted owner seeding below grants no runtime business writes.
        f.sql(&format!("GRANT SELECT(password_salt,password_hash,totp_secret,otp_last_counter),UPDATE(otp_last_counter) ON {}.users TO {}",f.source,f.role)).await;
        f.sql(&format!(
            "GRANT INSERT(token,username,account_id,expires_at) ON {}.sessions TO {}",
            f.source, f.role
        ))
        .await;
        let first = base.artifact(0, FIRST_TEXT.as_bytes()).await;
        let second = base.artifact(0, SECOND_TEXT.as_bytes()).await;
        let payload = vec![
            TextPayloadBinding::new(first.reference, "教学说明.txt", "plaintext").unwrap(),
            TextPayloadBinding::new(second.reference, "未来领域说明.txt", "future-notes-v9")
                .unwrap(),
        ];
        base.task_manifest(
            10,
            0,
            TaskContentManifest::new(TEXT_TITLE, payload).unwrap(),
        )
        .await;
        base.task(20, 1, FOREIGN_TITLE).await;
        base.task(30, 0, EMPTY_TITLE).await;
        // Exactly 21 own heads force the fixed 20-item browser page, without a special test
        // page-size override. The foreign head is interleaved in the global ID order.
        for number in 0..19 {
            base.task(
                40 + number,
                0,
                &format!("Private collaboration outline {number:02}"),
            )
            .await;
        }
        let reserve = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let bind = reserve.local_addr().unwrap();
        drop(reserve);
        let origin = format!("http://{bind}");
        let c = f.config();
        let options = f.runtime().connect_options();
        let transport = match options.get_socket() {
            Some(path) => json!({"kind":"unix","directory":path}),
            None => json!({"kind":"loopback","address":options.get_host()}),
        };
        let document = json!({"format_version":3,"origin":origin,"authority_id":c.scope().authority_id.as_uuid(),"workspace_id":c.scope().workspace_id.as_uuid(),"namespaces":{"source":f.source,"task":f.task,"artifacts":f.artifacts},"database":{"database":f.database,"role":f.role,"system_identifier":c.target().system_identifier(),"database_oid":c.target().database_oid(),"transport":transport,"port":options.get_port(),"password":super::storage_helpers::PASSWORD},"session_limits":{"per_account":c.limits().per_account(),"total":c.limits().total()},"artifact_root":base.root.path(),"catalog_identity":Pins::read(f).await.json(),"marker_identity":MarkerPins::read(f).await.json()});
        let original = serde_json::to_vec(&document).unwrap();
        let path = base
            .root
            .path()
            .parent()
            .unwrap()
            .join("browser-task-input.json");
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        file.write_all(&original).unwrap();
        drop(file);
        Self {
            base,
            bind,
            origin,
            identity: Identity::current(),
            path,
            original,
        }
    }
    pub(super) fn spawn(&self) -> Running {
        let i = &self.identity;
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
                "serve-loopback",
                "--uid",
                &i.uid[0].to_string(),
                "--gid",
                &i.gid[0].to_string(),
                "--groups",
                &groups,
                "--config",
            ])
            .arg(&self.path)
            .args(["--bind", &self.bind.to_string(), "--session-ui"])
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
    pub(super) fn browser_input(&self) -> Value {
        let own = std::iter::once(task_id(10))
            .chain(std::iter::once(task_id(30)))
            .chain((0..19).map(|n| task_id(40 + n)))
            .map(|id| id.to_string())
            .collect::<Vec<_>>();
        json!({"origin":self.origin,"username":USER,"password":PASSWORD,"own":own,"text_id":task_id(10),"empty_id":task_id(30),"foreign_title":FOREIGN_TITLE,"text_title":TEXT_TITLE,"empty_title":EMPTY_TITLE,"first_text":FIRST_TEXT,"second_text":SECOND_TEXT})
    }
    pub(super) fn files(&self) -> Vec<(String, u64, u64, u32, u64, Vec<u8>)> {
        let mut files = fs::read_dir(self.base.root.path())
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
            && self.identity == Identity::current();
        let base = self.base.finish();
        unchanged && base
    }
}

pub(super) async fn resources(f: &Fixture) -> Vec<(String, String)> {
    let rows=within(query("SELECT n.nspname AS schema,c.relname AS name FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=ANY($1) AND c.relkind='r' AND c.relname<>'sessions' ORDER BY n.nspname,c.relname").bind(vec![f.source.clone(),f.task.clone(),f.artifacts.clone()]).fetch_all(f.owner())).await.unwrap();
    let mut result = Vec::new();
    for row in rows {
        let schema: String = row.get("schema");
        let table: String = row.get("name");
        let name = format!("{schema}.{table}");
        let value = if schema == f.source && table == "users" {
            "to_jsonb(t)-'otp_last_counter'"
        } else {
            "to_jsonb(t)"
        };
        let digest:String=within(query(&format!("SELECT md5(coalesce(string_agg(({value})::text,'|' ORDER BY ({value})::text),'')) AS digest FROM {name} t")).fetch_one(f.owner())).await.unwrap().get("digest");
        result.push((name, digest));
    }
    result
}
pub(super) async fn counter(f: &Fixture) -> i64 {
    within(
        query(&format!(
            "SELECT otp_last_counter AS counter FROM {}.users WHERE username=$1",
            f.source
        ))
        .bind(USER)
        .fetch_one(f.owner()),
    )
    .await
    .unwrap()
    .get("counter")
}
pub(super) async fn exact_session(f: &Fixture, digest: &str) {
    assert!(
        digest.len() == 64
            && digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    let count:i64=within(query(&format!("SELECT count(*) AS count FROM {0}.sessions s JOIN {0}.users u USING(username,account_id) WHERE s.token=$1 AND s.username=$2 AND s.account_id=$3 AND s.expires_at>clock_timestamp()",f.source)).bind(digest).bind(USER).bind(uuid::Uuid::from_u128(8300)).fetch_one(f.owner())).await.unwrap().get("count");
    assert_eq!(count, 1);
}
pub(super) async fn no_resource_writes(f: &Fixture) {
    let clean:bool=within(query("SELECT NOT EXISTS(SELECT 1 FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=ANY($1) AND c.relkind='r' AND (has_table_privilege($2,c.oid,'INSERT') OR has_table_privilege($2,c.oid,'DELETE') OR has_table_privilege($2,c.oid,'UPDATE') OR has_table_privilege($2,c.oid,'TRIGGER'))) AS clean").bind(vec![f.task.clone(),f.artifacts.clone()]).bind(&f.role).fetch_one(f.owner())).await.unwrap().get("clean");
    assert!(
        clean,
        "UI must not gain Task/Artifact business write grants"
    );
}
