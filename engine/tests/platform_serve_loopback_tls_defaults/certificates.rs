//! Fixture-only native certificate generation. Keys never enter repository files or logs.
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
pub(crate) struct Certificates {
    pub(crate) ca: Vec<u8>,
    pub(crate) certificate: PathBuf,
    pub(crate) key: PathBuf,
}
struct Job(Child);
impl Drop for Job {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn command(directory: &Path, args: &[&str]) {
    let mut job = Job(Command::new("openssl")
        .current_dir(directory)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("explicit native TLS fixture requires OpenSSL"));
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = job.0.try_wait().unwrap() {
            assert!(status.success(), "disposable TLS generation failed");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "disposable TLS generation exceeded its deadline"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
impl Certificates {
    pub(crate) fn generate(parent: &Path, ip: std::net::IpAddr) -> Self {
        // Fixture callers may pass their owned directory as `path(".")`; never send that
        // noncanonical spelling to the production private reader.
        let directory = fs::canonicalize(parent)
            .unwrap()
            .join(format!("tls-{}", uuid::Uuid::new_v4().simple()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        command(
            &directory,
            &[
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-days",
                "1",
                "-subj",
                "/CN=Cyanrex Disposable Loopback CA",
                "-addext",
                "basicConstraints=critical,CA:TRUE",
                "-addext",
                "keyUsage=critical,keyCertSign,cRLSign",
                "-keyout",
                "ca.key",
                "-out",
                "ca.pem",
            ],
        );
        command(
            &directory,
            &[
                "req",
                "-new",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-subj",
                "/CN=Cyanrex Disposable Loopback Server",
                "-keyout",
                "server.key",
                "-out",
                "server.csr",
            ],
        );
        fs::write(directory.join("leaf.ext"),format!("subjectAltName=IP:{ip}\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n")).unwrap();
        command(
            &directory,
            &[
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
            ],
        );
        for leaf in ["ca.key", "server.key", "server.pem"] {
            fs::set_permissions(directory.join(leaf), fs::Permissions::from_mode(0o600)).unwrap();
        }
        Self {
            ca: fs::read(directory.join("ca.pem")).unwrap(),
            certificate: directory.join("server.pem"),
            key: directory.join("server.key"),
        }
    }
    pub(crate) fn bytes(&self) -> (Vec<u8>, Vec<u8>) {
        (
            fs::read(&self.certificate).unwrap(),
            fs::read(&self.key).unwrap(),
        )
    }
    pub(crate) fn unchanged(&self, before: &(Vec<u8>, Vec<u8>)) -> bool {
        self.bytes() == *before
    }
}
