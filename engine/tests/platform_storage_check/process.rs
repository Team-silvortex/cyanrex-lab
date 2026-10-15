use super::catalog_fixture::Fixture;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::{self, OpenOptions, Permissions},
    io::{self, Write},
    os::unix::{
        fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Output, Stdio},
    time::{Duration, Instant},
};
use tokio::process::{Child, Command};
use uuid::Uuid;

pub(super) const SUCCESS: &[u8] = b"platform storage observations completed; no host started\n";
pub(super) const FAILURE: &[u8] = b"platform storage check failed; no host started\n";
const PASSWORD: &str = "synthetic-platform-catalog-fixture-only";

#[derive(PartialEq, Eq)]
struct Identity {
    uid: [u32; 4],
    gid: [u32; 4],
    groups: Vec<u32>,
    caps: [u64; 5],
}

impl Identity {
    fn current() -> Self {
        let status = fs::read_to_string("/proc/thread-self/status").unwrap();
        let field = |name: &str| {
            status
                .lines()
                .find_map(|line| line.strip_prefix(name))
                .unwrap()
                .trim()
                .to_owned()
        };
        let ids = |name: &str| {
            field(name)
                .split_whitespace()
                .map(|id| id.parse().unwrap())
                .collect::<Vec<u32>>()
                .try_into()
                .unwrap()
        };
        let mut groups = field("Groups:")
            .split_whitespace()
            .map(|id| id.parse().unwrap())
            .collect::<Vec<u32>>();
        groups.sort_unstable();
        Self {
            uid: ids("Uid:"),
            gid: ids("Gid:"),
            groups,
            caps: ["CapPrm:", "CapEff:", "CapInh:", "CapAmb:", "CapBnd:"]
                .map(|name| u64::from_str_radix(&field(name), 16).unwrap()),
        }
    }

    fn groups(&self) -> String {
        if self.groups.is_empty() {
            "none".to_owned()
        } else {
            self.groups
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",")
        }
    }
}

fn nnp() -> i32 {
    // SAFETY: read-only inspection of this calling thread's kernel flag.
    unsafe {
        libc::prctl(
            libc::PR_GET_NO_NEW_PRIVS,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
        )
    }
}

fn clear_child_capabilities(command: &mut std::process::Command) {
    #[repr(C)]
    struct Header {
        version: u32,
        pid: i32,
    }
    #[repr(C)]
    struct Data {
        effective: u32,
        permitted: u32,
        inheritable: u32,
    }
    // SAFETY: child-side syscalls only, over stack-owned arguments, reduce only its
    // P/E/I/A capabilities. No parent identity, groups or authority is changed.
    unsafe {
        command.pre_exec(|| {
            let header = Header {
                version: 0x2008_0522,
                pid: 0,
            };
            let data = [
                Data {
                    effective: 0,
                    permitted: 0,
                    inheritable: 0,
                },
                Data {
                    effective: 0,
                    permitted: 0,
                    inheritable: 0,
                },
            ];
            if libc::syscall(libc::SYS_capset, &header as *const Header, data.as_ptr()) != 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::prctl(
                libc::PR_CAP_AMBIENT,
                libc::PR_CAP_AMBIENT_CLEAR_ALL as libc::c_ulong,
                0 as libc::c_ulong,
                0 as libc::c_ulong,
                0 as libc::c_ulong,
            ) != 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

fn directory(path: &Path) {
    fs::DirBuilder::new().mode(0o700).create(path).unwrap();
    fs::set_permissions(path, Permissions::from_mode(0o700)).unwrap();
}

fn root_snapshot(path: &Path) -> ((u64, u64, u32, u32, u32), Vec<u8>) {
    let metadata = fs::symlink_metadata(path).unwrap();
    (
        (
            metadata.dev(),
            metadata.ino(),
            metadata.uid(),
            metadata.gid(),
            metadata.mode(),
        ),
        fs::read(path.join("synthetic-unpublished-marker.bin")).unwrap(),
    )
}

pub(super) struct Input {
    directory: PathBuf,
    pub(super) root: PathBuf,
    pub(super) document: Value,
    path: PathBuf,
    bytes: Vec<u8>,
    identity: Identity,
    nnp: i32,
    environment: BTreeMap<OsString, OsString>,
    root_original: ((u64, u64, u32, u32, u32), Vec<u8>),
}

struct InitialDirectoryGuard(Option<PathBuf>);
impl Drop for InitialDirectoryGuard {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            if fs::remove_dir_all(path).is_err() {
                eprintln!("owned storage-check setup cleanup failed");
            }
        }
    }
}

impl Input {
    pub(super) fn new(fixture: &Fixture) -> Self {
        let directory_path = PathBuf::from("/tmp").join(format!(
            "cyanrex-platform-storage-check-{}",
            Uuid::new_v4().simple()
        ));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory_path)
            .unwrap();
        let mut initial_guard = InitialDirectoryGuard(Some(directory_path.clone()));
        fs::set_permissions(&directory_path, Permissions::from_mode(0o700)).unwrap();
        let root = directory_path.join("artifacts");
        directory(&root);
        let marker = root.join("synthetic-unpublished-marker.bin");
        fs::write(
            &marker,
            b"synthetic unpublished fixture; not a stored Artifact\n",
        )
        .unwrap();
        fs::set_permissions(&marker, Permissions::from_mode(0o400)).unwrap();
        let options = fixture.runtime().connect_options();
        let transport = match options.get_socket() {
            Some(path) => json!({"kind": "unix", "directory": path}),
            None => json!({"kind": "loopback", "address": options.get_host()}),
        };
        let config = fixture.config();
        let document = json!({
            "format_version": 1, "origin": config.origin(),
            "authority_id": config.scope().authority_id.as_uuid(),
            "workspace_id": config.scope().workspace_id.as_uuid(),
            "namespaces": {"source": fixture.source, "task": fixture.task, "artifacts": fixture.artifacts},
            "database": {"database": fixture.database, "role": fixture.role,
                "system_identifier": config.target().system_identifier(),
                "database_oid": config.target().database_oid(), "transport": transport,
                "port": options.get_port(), "password": PASSWORD},
            "session_limits": {"per_account": config.limits().per_account(), "total": config.limits().total()},
            "artifact_root": root
        });
        let bytes = serde_json::to_vec(&document).unwrap();
        let path = directory_path.join("input.json");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        file.write_all(&bytes).unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
        let root_original = root_snapshot(&root);
        let input = Self {
            directory: directory_path,
            root,
            document,
            path,
            bytes,
            identity: Identity::current(),
            nnp: nnp(),
            environment: std::env::vars_os().collect(),
            root_original,
        };
        initial_guard.0.take();
        input
    }

    pub(super) fn write_variant(&self, name: &str, document: &Value) -> PathBuf {
        let path = self.directory.join(name);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        file.write_all(&serde_json::to_vec(document).unwrap())
            .unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
        path
    }

    pub(super) fn spawn(&self, path: Option<&Path>) -> Running {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cyanrex-platform"));
        command
            .args([
                "check-storage",
                "--uid",
                &self.identity.uid[0].to_string(),
                "--gid",
                &self.identity.gid[0].to_string(),
                "--groups",
                &self.identity.groups(),
                "--config",
            ])
            .arg(path.unwrap_or(&self.path))
            .current_dir(&self.directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
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
        clear_child_capabilities(command.as_std_mut());
        Running {
            child: Some(command.spawn().unwrap()),
            started: Instant::now(),
        }
    }

    pub(super) fn finish(self) -> bool {
        let unchanged = Identity::current() == self.identity
            && nnp() == self.nnp
            && std::env::vars_os().collect::<BTreeMap<_, _>>() == self.environment
            && fs::read(&self.path).is_ok_and(|bytes| bytes == self.bytes)
            && root_snapshot(&self.root) == self.root_original;
        let cleaned = self.cleanup().is_ok();
        unchanged && cleaned
    }

    fn cleanup(&self) -> io::Result<()> {
        match fs::remove_dir_all(&self.directory) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        if self.cleanup().is_err() {
            eprintln!("owned storage-check input cleanup failed");
        }
    }
}

pub(super) struct Running {
    child: Option<Child>,
    started: Instant,
}
pub(super) struct Run {
    pub(super) output: Output,
    pub(super) timed_out: bool,
}

impl Running {
    pub(super) async fn finish(mut self, kill: bool) -> Run {
        if kill {
            let _ = self.child.as_mut().unwrap().start_kill();
        }
        let timed_out = loop {
            if self.child.as_mut().unwrap().try_wait().unwrap().is_some() {
                break false;
            }
            if self.started.elapsed() >= Duration::from_secs(15) {
                let _ = self.child.as_mut().unwrap().start_kill();
                break true;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        };
        let output = self.child.take().unwrap().wait_with_output().await.unwrap();
        Run { output, timed_out }
    }
}

pub(super) fn passed(run: &Run) {
    assert!(!run.timed_out);
    assert_eq!(run.output.status.code(), Some(0));
    assert_eq!(run.output.stdout, SUCCESS);
    assert!(run.output.stderr.is_empty());
}
pub(super) fn rejected(run: &Run) {
    assert!(!run.timed_out);
    assert_eq!(run.output.status.code(), Some(1));
    assert!(run.output.stdout.is_empty());
    assert_eq!(run.output.stderr, FAILURE);
}
