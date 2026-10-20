use super::{
    fixture::{Fixture, Run, CANARY},
    kernel::clear_child_capabilities,
};
use serde_json::{json, Value};
use std::{
    ffi::OsString,
    fs,
    io::{self, Read},
    net::{SocketAddr, TcpListener, TcpStream},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Output, Stdio},
    thread::JoinHandle,
    time::{Duration, Instant},
};

pub(super) fn listener() -> TcpListener {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    listener
}
pub(super) fn address() -> SocketAddr {
    listener().local_addr().unwrap()
}
pub(super) fn no_dial(listener: &TcpListener) -> bool {
    match listener.accept() {
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => true,
        Ok((peer, _)) => {
            drop(peer);
            false
        }
        _ => false,
    }
}
pub(super) fn no_host(address: SocketAddr) -> bool {
    TcpStream::connect_timeout(&address, Duration::from_millis(100)).is_err()
        && TcpListener::bind(address).is_ok()
}
pub(super) fn root(fixture: &Fixture, name: &str) -> PathBuf {
    let path = fixture.path(name);
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    path
}
pub(super) fn document(root: &Path, db_port: u16, bind: SocketAddr) -> Value {
    json!({
        "format_version":3,"origin":format!("http://{bind}"),
        "authority_id":"00000000-0000-0000-0000-000000001069",
        "workspace_id":"00000000-0000-0000-0000-00000000106a",
        "namespaces":{"source":"source_private","task":"task_private","artifacts":"artifacts_private"},
        "database":{"database":"platform_db","role":"platform_runtime","system_identifier":123,
            "database_oid":456,"transport":{"kind":"loopback","address":"127.0.0.1"},
            "port":db_port,"password":CANARY},
        "session_limits":{"per_account":4,"total":16},"artifact_root":root,
        "catalog_identity":{"role_oid":101,"database_owner_oid":102,
            "source":{"oid":201,"owner_oid":102},"task":{"oid":202,"owner_oid":102},
            "artifacts":{"oid":203,"owner_oid":102}},
        "marker_identity":{"source":{"oid":301,"owner_oid":102},"identity":{"oid":302,"owner_oid":102},
            "access":{"oid":303,"owner_oid":102},"task":{"oid":304,"owner_oid":102},
            "artifacts":{"oid":305,"owner_oid":102}}
    })
}
pub(super) fn input(fixture: &Fixture, name: &str, document: &Value) -> PathBuf {
    fixture.write(name, &serde_json::to_vec(document).unwrap(), 0o600)
}
pub(super) fn arguments(fixture: &Fixture, input: &Path, bind: SocketAddr) -> Vec<OsString> {
    let mut args = fixture.arguments(input);
    args[0] = "serve-loopback".into();
    args.extend([OsString::from("--bind"), bind.to_string().into()]);
    args
}

// Pipe readers drain continuously with bounded retention. There is intentionally no automatic
// lifetime timeout: a ready long-lived host must not be killed by a fixture's startup timer.
struct Pipe {
    task: JoinHandle<(Vec<u8>, bool)>,
}
impl Pipe {
    fn new(mut reader: impl Read + Send + 'static) -> Self {
        Self {
            task: std::thread::spawn(move || {
                let mut saved = Vec::new();
                let mut overflow = false;
                let mut chunk = [0u8; 1024];
                loop {
                    match reader.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(count) => {
                            let keep = (4096usize.saturating_sub(saved.len())).min(count);
                            saved.extend_from_slice(&chunk[..keep]);
                            overflow |= keep != count;
                        }
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                        Err(_) => {
                            overflow = true;
                            break;
                        }
                    }
                }
                (saved, overflow)
            }),
        }
    }
    fn finish(self) -> (Vec<u8>, bool) {
        self.task.join().unwrap()
    }
}
pub(super) struct ChildGuard {
    child: Option<Child>,
    stdout: Option<Pipe>,
    stderr: Option<Pipe>,
}
impl ChildGuard {
    pub(super) fn spawn(fixture: &Fixture, args: &[OsString], pgoptions: Option<&str>) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cyanrex-platform"));
        command
            .args(args)
            .current_dir(fixture.path("."))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (name, _) in std::env::vars_os() {
            if name.as_encoded_bytes().starts_with(b"PG") {
                command.env_remove(name);
            }
        }
        command.env_remove("DATABASE_URL");
        for (name, value) in [
            ("PGHOST", "untrusted.invalid"),
            ("PGHOSTADDR", "192.0.2.99"),
            ("PGPORT", "1"),
            ("PGUSER", CANARY),
            ("PGDATABASE", CANARY),
            ("PGPASSWORD", CANARY),
            ("PGAPPNAME", CANARY),
            ("PGSSLMODE", "verify-full"),
            ("PGSSLROOTCERT", CANARY),
            ("PGSSLCERT", CANARY),
            ("PGSSLKEY", CANARY),
            ("DATABASE_URL", "postgresql://untrusted.invalid/never"),
        ] {
            command.env(name, value);
        }
        command.env("PGPASSFILE", fixture.path("ambient.pgpass"));
        command.env("PGSERVICEFILE", fixture.path("ambient.service"));
        if let Some(value) = pgoptions {
            command.env("PGOPTIONS", value);
        }
        clear_child_capabilities(&mut command);
        let mut child = command.spawn().unwrap();
        let stdout = Some(Pipe::new(child.stdout.take().unwrap()));
        let stderr = Some(Pipe::new(child.stderr.take().unwrap()));
        Self {
            child: Some(child),
            stdout,
            stderr,
        }
    }
    pub(super) fn finished(&mut self) -> bool {
        self.child.as_mut().unwrap().try_wait().unwrap().is_some()
    }
    pub(super) fn wait(mut self, deadline: Instant) -> Run {
        let timed_out = loop {
            if self.finished() {
                break false;
            }
            if Instant::now() >= deadline {
                let _ = self.child.as_mut().unwrap().kill();
                break true;
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        let status = self.child.take().unwrap().wait().unwrap();
        self.collect(status, timed_out)
    }
    fn collect(&mut self, status: ExitStatus, timed_out: bool) -> Run {
        let (stdout, out_overflow) = self.stdout.take().unwrap().finish();
        let (stderr, err_overflow) = self.stderr.take().unwrap().finish();
        assert!(
            !out_overflow && !err_overflow,
            "child output exceeded its retained budget"
        );
        Run {
            output: Output {
                status,
                stdout,
                stderr,
            },
            timed_out,
        }
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(pipe) = self.stdout.take() {
            let _ = pipe.finish();
        }
        if let Some(pipe) = self.stderr.take() {
            let _ = pipe.finish();
        }
    }
}
pub(super) fn run(fixture: &Fixture, args: &[OsString], pgoptions: Option<&str>) -> Run {
    ChildGuard::spawn(fixture, args, pgoptions).wait(Instant::now() + Duration::from_secs(5))
}
pub(super) fn rejected(run: &Run, code: i32) {
    assert!(!run.timed_out, "owned child deadline expired");
    assert_eq!(run.output.status.code(), Some(code));
    assert!(
        run.output.stdout.is_empty(),
        "rejected startup must never announce ready"
    );
    assert!(!run.output.stderr.is_empty() && run.output.stderr.len() <= 256);
    let error = String::from_utf8_lossy(&run.output.stderr);
    for secret in [
        CANARY,
        "/tmp/",
        "source_private",
        "untrusted.invalid",
        "postgresql://",
    ] {
        assert!(
            !error.contains(secret),
            "generic CLI failure echoed synthetic input"
        );
    }
}
