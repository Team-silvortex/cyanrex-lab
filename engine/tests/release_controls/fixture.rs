use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::ffi::CString;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(super) const CONTROLS: [&str; 3] = ["deploy.sh", "docker-compose.yml", "cyanrex-release"];
pub(super) const SECRET: &str = "synthetic-runtime-secret-never-print";

pub(super) struct Fixture {
    pub root: PathBuf,
    pub package: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        Self::version(env!("CARGO_PKG_VERSION"))
    }

    pub fn version(version: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cyanrex-release-controls-{}-{unique}",
            std::process::id()
        ));
        let name = format!("cyanrex-lab-{version}-20261018-010203");
        let package = root.join(&name);
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir(&package).unwrap();
        fs::create_dir(root.join("bin")).unwrap();
        for name in ["docker", "docker-compose", "ssh", "bash", "sh"] {
            write_executable(&root.join("bin").join(name), trap());
        }
        for name in ["deploy.sh", "cyanrex-release"] {
            write_executable(&package.join(name), trap());
        }
        fs::write(package.join("docker-compose.yml"), "services: {}\n").unwrap();
        fs::write(
            package.join("cyanrex-images.tar"),
            "synthetic unused image bytes\n",
        )
        .unwrap();
        let image_hash = hash(&package.join("cyanrex-images.tar"));
        let metadata = json!({
            "schemaVersion": 1,
            "package": {"name":name,"version":version,"createdAt":"2026-10-18T01:02:03Z"},
            "source": {"revision":null,"state":"unavailable","tag":null},
            "compose": {"file":"docker-compose.yml","source":"docker/docker-compose.distribution.yml"},
            "images": {
                "mode":"prebuilt",
                "references":{"engine":"engine:synthetic","frontend":"frontend:synthetic","postgres":"postgres:16"},
                "contentIds":{"engine":format!("sha256:{}","a".repeat(64)),
                    "frontend":format!("sha256:{}","b".repeat(64)),"postgres":format!("sha256:{}","c".repeat(64))},
                "archive":{"file":"cyanrex-images.tar","sha256":image_hash}
            },
            "integrity":{"algorithm":"sha256","manifest":"checksums.sha256"}
        });
        fs::write(package.join("release-metadata.json"), metadata.to_string()).unwrap();
        let fixture = Self { root, package };
        fixture.refresh_manifest();
        fixture
    }

    pub fn refresh_manifest(&self) {
        let names = [
            "deploy.sh",
            "docker-compose.yml",
            "cyanrex-release",
            "cyanrex-images.tar",
            "release-metadata.json",
        ];
        let manifest = names
            .iter()
            .map(|name| format!("{}  {name}\n", hash(&self.package.join(name))))
            .collect::<String>();
        fs::write(self.package.join("checksums.sha256"), manifest).unwrap();
    }

    pub fn runtime(&self) {
        let text = format!("PRIVATE_VALUE={SECRET}\n{}", trap());
        fs::write(self.package.join(".env"), text).unwrap();
        fs::set_permissions(self.package.join(".env"), fs::Permissions::from_mode(0o600)).unwrap();
        fs::create_dir(self.package.join("runtime-data")).unwrap();
        fs::write(self.package.join("runtime-data/opaque.bin"), SECRET).unwrap();
        fifo(&self.package.join("runtime-data/unreadable-pipe"));
        symlink(".", self.package.join("runtime-data/recursive-link")).unwrap();
    }

    pub fn run(&self) -> Output {
        self.command(&["package", "verify-controls"], Some(&self.package))
    }

    pub fn command(&self, args: &[&str], path: Option<&Path>) -> Output {
        let before = snapshot(&self.root);
        let mut command = Command::new(env!("CARGO_BIN_EXE_cyanrex-release"));
        command.args(args);
        if let Some(path) = path {
            command.arg(path);
        }
        command
            .current_dir(&self.root)
            .env("PATH", self.root.join("bin"))
            .env("TEST_OPERATIONS", self.root.join("operations.log"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("read-only CLI exceeded its test deadline (possible FIFO read or child command)");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let output = child.wait_with_output().unwrap();
        assert_eq!(
            snapshot(&self.root),
            before,
            "read-only CLI changed fixture files"
        );
        assert!(
            !self.root.join("operations.log").exists(),
            "a deployment/tool/env trap ran"
        );
        for bytes in [&output.stdout, &output.stderr] {
            assert!(
                !String::from_utf8_lossy(bytes).contains(SECRET),
                "CLI leaked runtime secret"
            );
        }
        output
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn trap() -> &'static str {
    "#!/bin/sh\nprintf 'unexpected command or sourced environment' > \"$TEST_OPERATIONS\"\nexit 97\n"
}

fn write_executable(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

pub(super) fn fifo(path: &Path) {
    let path = CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
}

fn hash(path: &Path) -> String {
    format!("{:x}", Sha256::digest(fs::read(path).unwrap()))
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, String> {
    let mut result = BTreeMap::new();
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let metadata = fs::symlink_metadata(entry.path()).unwrap();
            let value = if metadata.file_type().is_dir() {
                pending.push(entry.path());
                "directory".to_owned()
            } else if metadata.file_type().is_file() {
                hash(&entry.path())
            } else if metadata.file_type().is_symlink() {
                format!("symlink:{:?}", fs::read_link(entry.path()).unwrap())
            } else {
                "special".to_owned()
            };
            result.insert(
                entry.path().strip_prefix(root).unwrap().to_owned(),
                format!("{}:{value}", metadata.permissions().mode()),
            );
        }
    }
    result
}

pub(super) fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}

pub(super) fn denied(output: &Output, expected: &str) {
    assert!(!output.status.success(), "unexpected success");
    assert!(
        output.stdout.is_empty(),
        "failure claimed verification success"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(expected),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
