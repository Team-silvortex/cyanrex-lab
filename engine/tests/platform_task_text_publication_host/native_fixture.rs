use super::{
    certificates::Certificates,
    child::Running,
    expectation_helpers::Pins,
    fixture::*,
    kernel::{clear_child_capabilities, Identity},
    marker_helpers::MarkerPins,
};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client, Method,
};
use serde_json::{json, Value};
use sha1::Sha1;
use sqlx_core::{query::query, row::Row};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    net::{SocketAddr, TcpListener},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    process::{Command, Stdio},
    sync::OnceLock,
    time::Duration,
};

const PASSWORD: &str = "synthetic-task-bound-host-login-password-only";

fn otp() -> String {
    let counter = Utc::now().timestamp().div_euclid(30) as u64 + 1;
    let mut mac = Hmac::<Sha1>::new_from_slice(b"12345678901234567890").unwrap();
    mac.update(&counter.to_be_bytes());
    let bytes = mac.finalize().into_bytes();
    let offset = (bytes[19] & 15) as usize;
    let value = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) & 0x7fff_ffff;
    format!("{:06}", value % 1_000_000)
}

pub(super) fn credentials() -> Value {
    json!({"schema_version":1,"username":"index-owner-0","password":PASSWORD,"otp":otp()})
}

pub(super) async fn login_profile(p: &Prepared<'_>) {
    static HASH: OnceLock<String> = OnceLock::new();
    let hash = HASH.get_or_init(|| {
        let salt = SaltString::encode_b64(b"synthetic-task-bound-login-salt").unwrap();
        Argon2::default()
            .hash_password(PASSWORD.as_bytes(), &salt)
            .unwrap()
            .to_string()
    });
    within(query(&format!("UPDATE {}.users SET password_salt=$1,password_hash=$2,totp_secret=$3 WHERE username='index-owner-0'", p.f.source))
        .bind("synthetic-task-bound-login-salt").bind(hash).bind("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ")
        .execute(p.f.owner())).await.unwrap();
    for (table, grants) in [
        ("users", "SELECT(password_salt,password_hash,totp_secret,otp_last_counter),UPDATE(otp_last_counter)"),
        ("sessions", "INSERT(token,username,account_id,expires_at),DELETE"),
    ] {
        p.f.sql(&format!("GRANT {grants} ON {}.{table} TO {}", p.f.source, p.f.role)).await;
    }
}

pub(super) struct Hosted {
    pub(super) bind: SocketAddr,
    pub(super) origin: String,
    pub(super) identity: Identity,
    pub(super) tls: Option<Certificates>,
    path: PathBuf,
    original: Vec<u8>,
    material: Option<(Vec<u8>, Vec<u8>)>,
}

impl Hosted {
    pub(super) async fn new(p: &Prepared<'_>, tls: bool) -> Self {
        login_profile(p).await;
        let reserve = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let bind = reserve.local_addr().unwrap();
        drop(reserve);
        let origin = format!("{}://{bind}", if tls { "https" } else { "http" });
        let f = p.f;
        let c = f.config();
        let options = f.runtime().connect_options();
        let transport = match options.get_socket() {
            Some(path) => json!({"kind":"unix","directory":path}),
            None => json!({"kind":"loopback","address":options.get_host()}),
        };
        let document = json!({"format_version":3,"origin":origin,"authority_id":c.scope().authority_id.as_uuid(),"workspace_id":c.scope().workspace_id.as_uuid(),"namespaces":{"source":f.source,"task":f.task,"artifacts":f.artifacts},"database":{"database":f.database,"role":f.role,"system_identifier":c.target().system_identifier(),"database_oid":c.target().database_oid(),"transport":transport,"port":options.get_port(),"password":super::storage_helpers::PASSWORD},"session_limits":{"per_account":c.limits().per_account(),"total":c.limits().total()},"artifact_root":p.root.path(),"catalog_identity":Pins::read(f).await.json(),"marker_identity":MarkerPins::read(f).await.json()});
        let original = serde_json::to_vec(&document).unwrap();
        let path = p
            .root
            .path()
            .parent()
            .unwrap()
            .join("task-text-publication-host-input.json");
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        file.write_all(&original).unwrap();
        drop(file);
        let tls = tls.then(|| Certificates::generate(path.parent().unwrap(), bind.ip()));
        let material = tls.as_ref().map(Certificates::bytes);
        Self {
            bind,
            origin,
            identity: Identity::current(),
            tls,
            path,
            original,
            material,
        }
    }

    pub(super) fn spawn(&self, task_text_publication: bool) -> Running {
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
        if task_text_publication {
            command.arg("--task-text-publication");
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

    pub(super) fn unchanged(&self) -> bool {
        fs::read(&self.path).is_ok_and(|value| value == self.original)
            && Identity::current() == self.identity
            && self
                .tls
                .as_ref()
                .zip(self.material.as_ref())
                .is_none_or(|(tls, bytes)| tls.unchanged(bytes))
    }

    pub(super) fn client(&self) -> Client {
        let mut builder = Client::builder()
            .http1_only()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(10));
        if let Some(tls) = &self.tls {
            builder = builder
                .https_only(true)
                .tls_built_in_root_certs(false)
                .add_root_certificate(reqwest::Certificate::from_pem(&tls.ca).unwrap());
        }
        builder.build().unwrap()
    }
}

pub(super) fn req(
    client: &Client,
    origin: &str,
    method: Method,
    path: &str,
    token: Option<&str>,
) -> reqwest::RequestBuilder {
    let mut request = client
        .request(method, format!("{origin}{path}"))
        .header("origin", origin)
        .header("x-cyanrex-platform-request", "1");
    if let Some(token) = token {
        let mut cookie = HeaderValue::from_str(&format!(
            "cyanrex_platform_session={token}; cyanrex_session=unrelated-legacy-cookie"
        ))
        .unwrap();
        cookie.set_sensitive(true);
        request = request.header("cookie", cookie);
    }
    request
}

pub(super) async fn capture(request: reqwest::RequestBuilder, status: u16) -> (HeaderMap, Value) {
    let response = request
        .send()
        .await
        .unwrap_or_else(|_| panic!("owned Task-publication host request failed"));
    assert_eq!(response.status().as_u16(), status);
    let headers = response.headers().clone();
    for (name, value) in [
        ("cache-control", "no-store"),
        ("pragma", "no-cache"),
        ("x-content-type-options", "nosniff"),
        ("content-type", "application/json"),
    ] {
        assert_eq!(headers[name], value);
    }
    for name in ["retry-after", "location", "access-control-allow-origin"] {
        assert!(!headers.contains_key(name));
    }
    let bytes = response
        .bytes()
        .await
        .unwrap_or_else(|_| panic!("owned Task-publication host body failed"));
    assert!(bytes.len() <= 1024 * 1024);
    (headers, serde_json::from_slice(&bytes).unwrap())
}

pub(super) async fn call(
    client: &Client,
    origin: &str,
    method: Method,
    path: &str,
    token: &str,
    body: Option<Value>,
    status: u16,
) -> Value {
    let mut request = req(client, origin, method, path, Some(token));
    if let Some(body) = body {
        request = request.json(&body);
    }
    let (headers, value) = capture(request, status).await;
    assert!(!headers.contains_key("set-cookie"));
    value
}

pub(super) async fn login(host: &Hosted, client: &Client) -> String {
    let (headers, value) = capture(
        req(
            client,
            &host.origin,
            Method::POST,
            "/platform/v1/session/login",
            None,
        )
        .json(&credentials()),
        200,
    )
    .await;
    assert_eq!(value, json!({"schema_version":1,"authenticated":true}));
    assert_eq!(headers.get_all("set-cookie").iter().count(), 1);
    let (pair, attributes) = headers["set-cookie"]
        .to_str()
        .unwrap()
        .split_once(';')
        .unwrap();
    assert_eq!(
        attributes,
        if host.tls.is_some() {
            " Path=/platform/v1; HttpOnly; SameSite=Strict; Secure"
        } else {
            " Path=/platform/v1; HttpOnly; SameSite=Strict"
        }
    );
    let (name, token) = pair.split_once('=').unwrap();
    assert_eq!(name, "cyanrex_platform_session");
    let id = uuid::Uuid::parse_str(token).unwrap();
    assert!(!id.is_nil());
    assert_eq!(id.to_string(), token);
    token.to_owned()
}

pub(super) async fn committed_token(p: &Prepared<'_>, token: &str) {
    let exact: bool = within(query(&format!("SELECT count(*) FILTER(WHERE token=$1 AND username='index-owner-0' AND account_id=(SELECT account_id FROM {0}.users WHERE username='index-owner-0') AND expires_at>clock_timestamp())=1 AND count(*) FILTER(WHERE token=$2)=0 AS exact FROM {0}.sessions", p.f.source))
        .bind(token_hash(token)).bind(token).fetch_one(p.f.owner())).await.unwrap().get("exact");
    assert!(exact);
}

pub(super) async fn unknown_ca_refused(host: &Hosted) -> bool {
    matches!(
        certificate_refusal(host, rustls::RootCertStore::empty(), host.bind.ip()).await,
        Some(rustls::Error::InvalidCertificate(
            rustls::CertificateError::UnknownIssuer
        ))
    )
}

pub(super) async fn wrong_ip_refused(host: &Hosted) -> bool {
    use rustls::pki_types::{pem::PemObject, CertificateDer};
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(CertificateDer::from_pem_slice(&host.tls.as_ref().unwrap().ca).unwrap())
        .unwrap();
    matches!(
        certificate_refusal(host, roots, "127.0.0.2".parse().unwrap()).await,
        Some(rustls::Error::InvalidCertificate(_))
    )
}

async fn certificate_refusal(
    host: &Hosted,
    roots: rustls::RootCertStore,
    ip: std::net::IpAddr,
) -> Option<rustls::Error> {
    use rustls::{pki_types::ServerName, ClientConfig};
    use std::sync::Arc;
    let config =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth();
    let socket = within(tokio::net::TcpStream::connect(host.bind))
        .await
        .unwrap();
    // No HTTP/credentials are sent. Verify the actual listener with independent trust/SAN policy.
    within(
        tokio_rustls::TlsConnector::from(Arc::new(config))
            .connect(ServerName::IpAddress(ip.into()), socket),
    )
    .await
    .err()
    .and_then(|error| error.into_inner())
    .and_then(|error| error.downcast::<rustls::Error>().ok())
    .map(|error| *error)
}
