//! Disposable native OpenSSL material stays below one exclusive 0700 fixture directory.
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub struct Certificates {
    pub ca: Vec<u8>,
    pub(crate) certificate: PathBuf,
    pub(crate) key: PathBuf,
}
struct CommandGuard(Child);
impl Drop for CommandGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn generate(directory: &Path, args: &[&str]) {
    let mut child = CommandGuard(
        Command::new("openssl")
            .current_dir(directory)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("explicit native TLS fixture requires installed OpenSSL"),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success(), "disposable certificate generation failed");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "bounded disposable certificate generation"
        );
        thread::sleep(Duration::from_millis(10));
    }
}
impl Certificates {
    pub fn generate(parent: &Path) -> Self {
        let directory = parent.join("tls");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        generate(
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
                "/CN=Cyanrex Disposable Delivery CA",
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
        generate(
            &directory,
            &[
                "req",
                "-new",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-subj",
                "/CN=Cyanrex Disposable Delivery Server",
                "-keyout",
                "server.key",
                "-out",
                "server.csr",
            ],
        );
        fs::write(
            directory.join("leaf.ext"),
            concat!(
                "subjectAltName=IP:127.0.0.1\n",
                "basicConstraints=critical,CA:FALSE\n",
                "keyUsage=critical,digitalSignature,keyEncipherment\n",
                "extendedKeyUsage=serverAuth\n",
            ),
        )
        .unwrap();
        generate(
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
        for leaf in ["ca.key", "server.key"] {
            fs::set_permissions(directory.join(leaf), fs::Permissions::from_mode(0o600)).unwrap();
        }
        Self {
            ca: fs::read(directory.join("ca.pem")).unwrap(),
            certificate: directory.join("server.pem"),
            key: directory.join("server.key"),
        }
    }
}
