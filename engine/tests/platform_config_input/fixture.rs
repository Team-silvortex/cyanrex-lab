use super::kernel::{clear_child_capabilities, no_new_privileges, Identity};
use std::{
    collections::BTreeMap,
    ffi::{CString, OsString},
    fs::{self, File, OpenOptions, Permissions},
    io::{self, Read, Write},
    os::{
        fd::FromRawFd,
        unix::{
            ffi::OsStrExt,
            fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
        },
    },
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    time::{Duration, Instant},
};
use uuid::Uuid;

pub(super) const SUCCESS: &[u8] = b"platform configuration check passed; no host started\n";
pub(super) const CANARY: &str = "synthetic-config-password-never-echo-9d721";

pub(super) struct Run {
    pub(super) output: Output,
    pub(super) timed_out: bool,
}

struct ChildGuard(Option<Child>);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub(super) struct Fixture {
    directory: PathBuf,
    pub(super) identity: Identity,
}

impl Fixture {
    fn new() -> Self {
        let directory = PathBuf::from("/tmp").join(format!(
            "cyanrex-platform-config-input-{}",
            Uuid::new_v4().simple()
        ));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        fs::set_permissions(&directory, Permissions::from_mode(0o700)).unwrap();
        Self {
            directory,
            identity: Identity::current(),
        }
    }

    pub(super) fn path(&self, name: &str) -> PathBuf {
        self.directory.join(name)
    }

    pub(super) fn write(&self, name: &str, bytes: &[u8], mode: u32) -> PathBuf {
        let path = self.path(name);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        file.write_all(bytes).unwrap();
        fs::set_permissions(&path, Permissions::from_mode(mode)).unwrap();
        path
    }

    pub(super) fn arguments(&self, path: &Path) -> Vec<OsString> {
        let groups = if self.identity.safe_ids() && !self.identity.groups.is_empty() {
            self.identity
                .groups
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",")
        } else {
            "none".to_owned()
        };
        vec![
            "check-config".into(),
            "--uid".into(),
            self.identity.uid[0].max(1).to_string().into(),
            "--gid".into(),
            self.identity.gid[0].max(1).to_string().into(),
            "--groups".into(),
            groups.into(),
            "--config".into(),
            path.as_os_str().to_owned(),
        ]
    }

    pub(super) fn run(&self, arguments: &[OsString], polluted: bool) -> Run {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cyanrex-platform"));
        command
            .args(arguments)
            .current_dir(&self.directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for name in [
            "PGHOST",
            "PGHOSTADDR",
            "PGPORT",
            "PGDATABASE",
            "PGUSER",
            "PGPASSWORD",
            "PGPASSFILE",
            "PGSERVICE",
            "PGSERVICEFILE",
            "PGOPTIONS",
            "PGAPPNAME",
            "DATABASE_URL",
        ] {
            command.env_remove(name);
        }
        if polluted {
            for (name, value) in [
                ("PGHOST", CANARY),
                ("PGHOSTADDR", "192.0.2.99"),
                ("PGPORT", "1"),
                ("PGDATABASE", CANARY),
                ("PGUSER", CANARY),
                ("PGPASSWORD", CANARY),
                ("PGSERVICE", CANARY),
                ("PGOPTIONS", "-c role=synthetic_untrusted_role"),
                ("PGAPPNAME", CANARY),
                ("DATABASE_URL", "postgresql://invalid.invalid/never"),
            ] {
                command.env(name, value);
            }
            command.env("PGPASSFILE", self.path("ambient.pgpass"));
            command.env("PGSERVICEFILE", self.path("ambient.service"));
        }
        clear_child_capabilities(&mut command);
        let mut guard = ChildGuard(Some(command.spawn().unwrap()));
        let deadline = Instant::now() + Duration::from_secs(5);
        let timed_out = loop {
            if guard.0.as_mut().unwrap().try_wait().unwrap().is_some() {
                break false;
            }
            if Instant::now() >= deadline {
                let _ = guard.0.as_mut().unwrap().kill();
                break true;
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        let output = guard.0.take().unwrap().wait_with_output().unwrap();
        Run { output, timed_out }
    }

    pub(super) fn checked<T>(case: impl FnOnce(&Self) -> T) -> T {
        let fixture = Self::new();
        let environment = std::env::vars_os().collect::<BTreeMap<_, _>>();
        let nnp = no_new_privileges();
        let result = catch_unwind(AssertUnwindSafe(|| case(&fixture)));
        let same_identity = Identity::current() == fixture.identity;
        let same_nnp = no_new_privileges() == nnp;
        let same_environment = std::env::vars_os().collect::<BTreeMap<_, _>>() == environment;
        let cleaned = fixture.cleanup().is_ok();
        drop(fixture);
        assert!(
            cleaned,
            "owned configuration fixture must be cleaned before assertions"
        );
        assert!(
            same_identity && same_nnp && same_environment,
            "child checks must preserve parent state"
        );
        match result {
            Ok(value) => value,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }

    fn cleanup(&self) -> io::Result<()> {
        fn restore(path: &Path) -> io::Result<()> {
            let metadata = match fs::symlink_metadata(path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(error),
            };
            if metadata.is_dir() {
                fs::set_permissions(path, Permissions::from_mode(0o700))?;
                for entry in fs::read_dir(path)? {
                    restore(&entry?.path())?;
                }
            }
            Ok(())
        }
        restore(&self.directory)?;
        match fs::remove_dir_all(&self.directory) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if self.cleanup().is_err() {
            eprintln!("owned configuration fixture cleanup failed");
        }
    }
}

pub(super) fn passed(run: &Run) {
    assert!(!run.timed_out, "bounded child wait expired");
    assert_eq!(run.output.status.code(), Some(0));
    assert_eq!(run.output.stdout, SUCCESS);
    assert!(run.output.stderr.is_empty());
}

pub(super) fn rejected(run: &Run, code: i32) {
    assert!(!run.timed_out, "bounded child wait expired");
    assert_eq!(run.output.status.code(), Some(code));
    assert!(run.output.stdout.is_empty());
    assert!(!run.output.stderr.is_empty() && run.output.stderr.len() <= 256);
    let error = String::from_utf8_lossy(&run.output.stderr);
    for secret in [
        CANARY,
        "platform_config",
        "/tmp/",
        "source_private",
        "password",
    ] {
        assert!(
            !error.contains(secret),
            "generic errors must not echo input details"
        );
    }
}

pub(super) struct Watch(File);
impl Watch {
    pub(super) fn new(paths: &[&Path]) -> Self {
        // SAFETY: successful inotify_init1 creates one owned descriptor.
        let descriptor = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        assert!(descriptor >= 0);
        let file = unsafe { File::from_raw_fd(descriptor) };
        for path in paths {
            let name = CString::new(path.as_os_str().as_bytes()).unwrap();
            // SAFETY: descriptor is live and name is a NUL-terminated owned path.
            let watched = unsafe {
                libc::inotify_add_watch(descriptor, name.as_ptr(), libc::IN_OPEN | libc::IN_ACCESS)
            };
            assert!(watched >= 0);
        }
        Self(file)
    }

    pub(super) fn activity(&mut self) -> bool {
        let mut bytes = [0; 8192];
        match self.0.read(&mut bytes) {
            Ok(length) => length != 0,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => false,
            Err(_) => panic!("owned inotify oracle unavailable"),
        }
    }
}

pub(super) fn fifo(path: &Path) {
    let name = CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: mkfifo receives an owned NUL-terminated synthetic path.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    fs::set_permissions(path, Permissions::from_mode(0o600)).unwrap();
}
