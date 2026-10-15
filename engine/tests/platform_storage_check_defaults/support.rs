use super::{
    fixture::{Fixture, Run, CANARY},
    kernel::clear_child_capabilities,
};
use serde_json::json;
use std::{
    ffi::OsString,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub(super) const FAILURE: &[u8] = b"platform storage check failed; no host started\n";

pub(super) fn root(fixture: &Fixture, name: &str) -> PathBuf {
    let path = fixture.path(name);
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    path
}

pub(super) fn input(fixture: &Fixture, name: &str, root: &Path, port: u16) -> PathBuf {
    let document = json!({
        "format_version": 1,
        "origin": "https://platform.example",
        "authority_id": "00000000-0000-0000-0000-000000001069",
        "workspace_id": "00000000-0000-0000-0000-00000000106a",
        "namespaces": {"source": "source_private", "task": "task_private", "artifacts": "artifacts_private"},
        "database": {
            "database": "platform_db", "role": "platform_runtime", "system_identifier": 123,
            "database_oid": 456, "transport": {"kind": "loopback", "address": "127.0.0.1"},
            "port": port, "password": CANARY
        },
        "session_limits": {"per_account": 4, "total": 16},
        "artifact_root": root
    });
    fixture.write(name, &serde_json::to_vec(&document).unwrap(), 0o600)
}

pub(super) fn arguments(fixture: &Fixture, input: &Path) -> Vec<OsString> {
    let mut arguments = fixture.arguments(input);
    arguments[0] = "check-storage".into();
    arguments
}

pub(super) struct ChildGuard(Option<Child>);
impl ChildGuard {
    pub(super) fn spawn(
        fixture: &Fixture,
        arguments: &[OsString],
        pgoptions: Option<&str>,
    ) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cyanrex-platform"));
        command
            .args(arguments)
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
            ("PGDATABASE", CANARY),
            ("PGUSER", CANARY),
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
        Self(Some(command.spawn().unwrap()))
    }

    pub(super) fn finished(&mut self) -> bool {
        self.0.as_mut().unwrap().try_wait().unwrap().is_some()
    }

    pub(super) fn wait(mut self, deadline: Instant) -> Run {
        let timed_out = loop {
            if self.finished() {
                break false;
            }
            if Instant::now() >= deadline {
                let _ = self.0.as_mut().unwrap().kill();
                break true;
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        let output = self.0.take().unwrap().wait_with_output().unwrap();
        Run { output, timed_out }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub(super) fn run(fixture: &Fixture, arguments: &[OsString], pgoptions: Option<&str>) -> Run {
    ChildGuard::spawn(fixture, arguments, pgoptions).wait(Instant::now() + Duration::from_secs(5))
}

pub(super) fn failed(run: &Run) {
    assert!(!run.timed_out, "bounded owned child wait expired");
    assert_eq!(run.output.status.code(), Some(1));
    assert!(run.output.stdout.is_empty());
    assert_eq!(run.output.stderr, FAILURE);
}
