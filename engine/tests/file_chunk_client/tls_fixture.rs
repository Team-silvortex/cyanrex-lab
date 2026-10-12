//! Explicit OpenSSL test process with disposable CA/key material; no production TLS bypass.
use crate::{bytes, fixture::Fixture};
use std::{
    fs,
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub struct Certificates {
    pub ca: Vec<u8>,
    certificate: PathBuf,
    key: PathBuf,
}

fn generate(directory: &Path, args: &[&str]) {
    let status = Command::new("openssl")
        .current_dir(directory)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("the explicit TLS fixture requires installed OpenSSL");
    assert!(
        status.success(),
        "disposable OpenSSL certificate generation failed"
    );
}

impl Certificates {
    pub fn generate(f: &Fixture) -> Self {
        let directory = f.directory.join("tls");
        // The parent is this fixture's exclusive 0700 root; keys never enter repository files.
        fs::create_dir(&directory).unwrap();
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
                "/CN=Cyanrex Disposable Test CA",
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
                "/CN=Cyanrex Disposable Test Server",
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
        Self {
            ca: fs::read(directory.join("ca.pem")).unwrap(),
            certificate: directory.join("server.pem"),
            key: directory.join("server.key"),
        }
    }
}

pub struct TlsServer {
    pub origin: String,
    process: Child,
    responder: Option<JoinHandle<bool>>,
    received: Arc<AtomicUsize>,
}

pub struct TlsObservation {
    pub received_bytes: usize,
    pub responded: bool,
}

impl TlsServer {
    pub fn start(certificates: &Certificates, f: &Fixture, ip: Ipv4Addr) -> Self {
        let reserved = TcpListener::bind(SocketAddr::new(IpAddr::V4(ip), 0)).unwrap();
        let address = reserved.local_addr().unwrap();
        drop(reserved);
        let mut process = Command::new("openssl")
            .args(["s_server", "-quiet", "-ign_eof", "-accept"])
            .arg(address.to_string())
            .arg("-cert")
            .arg(&certificates.certificate)
            .arg("-key")
            .arg(&certificates.key)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start explicit disposable loopback TLS server");
        let mut incoming = process.stdout.take().unwrap();
        let mut outgoing = process.stdin.take().unwrap();
        let manifest = &f.manifest;
        let mut response = String::from("HTTP/1.1 200 OK\r\n");
        for (name, value) in [
            ("content-length", "17".to_owned()),
            ("content-type", "application/octet-stream".to_owned()),
            (
                "content-disposition",
                "attachment; filename=\"chunk.bin\"".to_owned(),
            ),
            ("x-cyanrex-file-transfer-version", "1".to_owned()),
            ("x-cyanrex-file-id", manifest.file_id().to_string()),
            (
                "x-cyanrex-file-version-id",
                manifest.version_id().to_string(),
            ),
            ("x-cyanrex-chunk-index", "0".to_owned()),
            (
                "x-cyanrex-chunk-sha256",
                manifest.chunk(0).unwrap().sha256.to_string(),
            ),
            ("x-cyanrex-file-sha256", manifest.sha256().to_string()),
            ("cache-control", "no-store".to_owned()),
            ("pragma", "no-cache".to_owned()),
            ("x-content-type-options", "nosniff".to_owned()),
            ("connection", "close".to_owned()),
        ] {
            response.push_str(&format!("{name}: {value}\r\n"));
        }
        response.push_str("\r\n");
        let mut wire = response.into_bytes();
        wire.extend((0..17).map(bytes::byte));
        let received = Arc::new(AtomicUsize::new(0));
        let counter = received.clone();
        let responder = thread::spawn(move || {
            let mut seen = Vec::new();
            let mut buffer = [0; 512];
            loop {
                match incoming.read(&mut buffer) {
                    Ok(0) | Err(_) => return false,
                    Ok(length) => {
                        counter.fetch_add(length, Ordering::SeqCst);
                        seen.extend_from_slice(&buffer[..length]);
                    }
                }
                if seen.windows(4).any(|w| w == b"\r\n\r\n") {
                    return outgoing
                        .write_all(&wire)
                        .and_then(|_| outgoing.flush())
                        .is_ok();
                }
                if seen.len() > 8192 {
                    return false;
                }
            }
        });
        let mut server = Self {
            origin: format!("https://{address}"),
            process,
            responder: Some(responder),
            received,
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(
                server.process.try_wait().unwrap().is_none(),
                "TLS fixture process must remain running"
            );
            if TcpStream::connect_timeout(&address, Duration::from_millis(50)).is_ok() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the actual loopback TLS listener must start"
            );
            thread::sleep(Duration::from_millis(10));
        }
        server
    }

    pub fn finish(mut self) -> TlsObservation {
        let _ = self.process.kill();
        let _ = self.process.wait();
        let responded = self.responder.take().unwrap().join().unwrap();
        TlsObservation {
            received_bytes: self.received.load(Ordering::SeqCst),
            responded,
        }
    }
}

impl Drop for TlsServer {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
        if let Some(responder) = self.responder.take() {
            let _ = responder.join();
        }
    }
}
