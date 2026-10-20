//! Runtime-generated CA and keys only. All files belong to one exclusive 0700 test directory.
use rustls::{
    pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer},
    ClientConfig, RootCertStore, ServerConfig,
};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) struct Material {
    pub(super) root: PathBuf,
    pub(super) cert: PathBuf,
    pub(super) key: PathBuf,
    pub(super) ca: Vec<u8>,
}
struct CommandGuard(Child);
impl Drop for CommandGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
impl Material {
    pub(super) fn new() -> Self {
        let root = PathBuf::from("/tmp").join(format!(
            "cyanrex-private-tls-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let mut material = Self {
            cert: root.join("server.pem"),
            key: root.join("server.key"),
            root,
            ca: Vec::new(),
        };
        material.run(&[
            "req",
            "-x509",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-nodes",
            "-days",
            "1",
            "-subj",
            "/CN=Cyanrex Disposable TLS CA",
            "-addext",
            "basicConstraints=critical,CA:TRUE",
            "-addext",
            "keyUsage=critical,keyCertSign,cRLSign",
            "-keyout",
            "ca.key",
            "-out",
            "ca.pem",
        ]);
        material.run(&[
            "req",
            "-new",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-nodes",
            "-subj",
            "/CN=Cyanrex Disposable TLS Server",
            "-keyout",
            "server.key",
            "-out",
            "server.csr",
        ]);
        material.write("leaf.ext",b"subjectAltName=IP:127.0.0.1,IP:::1,DNS:wrong.fixture.test\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=serverAuth\n");
        material.run(&[
            "x509",
            "-req",
            "-in",
            "server.csr",
            "-CA",
            "ca.pem",
            "-CAkey",
            "ca.key",
            "-CAcreateserial",
            "-out",
            "server.pem",
            "-days",
            "1",
            "-extfile",
            "leaf.ext",
        ]);
        for name in ["ca.key", "ca.pem", "server.key", "server.pem", "server.csr"] {
            fs::set_permissions(material.root.join(name), fs::Permissions::from_mode(0o600))
                .unwrap();
        }
        material.ca = fs::read(material.root.join("ca.pem")).unwrap();
        material
    }
    pub(super) fn run(&self, args: &[&str]) {
        let mut child = CommandGuard(
            Command::new("openssl")
                .current_dir(&self.root)
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("native TLS fixture requires OpenSSL"),
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                assert!(status.success(), "disposable OpenSSL operation failed");
                break;
            }
            assert!(
                Instant::now() < deadline,
                "disposable OpenSSL operation exceeded deadline"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    pub(super) fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        path
    }
    pub(super) fn client_config(&self, trusted: bool, alpn: Vec<Vec<u8>>) -> Arc<ClientConfig> {
        self.client_versions(trusted, alpn, rustls::DEFAULT_VERSIONS)
    }
    pub(super) fn client_versions(
        &self,
        trusted: bool,
        alpn: Vec<Vec<u8>>,
        versions: &[&'static rustls::SupportedProtocolVersion],
    ) -> Arc<ClientConfig> {
        let mut roots = RootCertStore::empty();
        if trusted {
            for cert in CertificateDer::pem_slice_iter(&self.ca) {
                roots.add(cert.unwrap()).unwrap();
            }
        }
        let mut config =
            ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_protocol_versions(versions)
                .unwrap()
                .with_root_certificates(roots)
                .with_no_client_auth();
        config.alpn_protocols = alpn;
        Arc::new(config)
    }
    pub(super) fn raw_server_config(&self) -> Arc<ServerConfig> {
        let cert = fs::read(&self.cert).unwrap();
        let key = fs::read(&self.key).unwrap();
        let chain = CertificateDer::pem_slice_iter(&cert)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let key = PrivateKeyDer::from_pem_slice(&key).unwrap();
        let mut config =
            ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_no_client_auth()
                .with_single_cert(chain, key)
                .unwrap();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        Arc::new(config)
    }
    pub(super) fn bytes(&self, path: &Path) -> Vec<u8> {
        fs::read(path).unwrap()
    }
}
impl Drop for Material {
    fn drop(&mut self) {
        if fs::remove_dir_all(&self.root).is_err() {
            eprintln!("owned temporary TLS material cleanup failed");
        }
    }
}
