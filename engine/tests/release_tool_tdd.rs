use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const PROGRAM: &str = "release-kernel-smoke-0123456789abcdef";
const REVISION: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

struct Fixture {
    root: PathBuf,
    environment: PathBuf,
    event: PathBuf,
    metadata: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cyanrex-release-tool-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let environment = root.join("environment.json");
        let event = root.join("event.json");
        let metadata = root.join("release-metadata.json");
        fs::write(
            &environment,
            r#"{
  "overall_ok": true,
  "generated_at": "2026-09-04T01:02:04Z",
  "runtime_mode": "native-linux",
  "checks": [{"name": "kernel", "ok": true, "detail": "mock kernel"}]
}
"#,
        )
        .unwrap();
        fs::write(
            &event,
            format!(
                r#"{{
  "timestamp": "2026-09-04T01:02:05Z",
  "event_type": "ebpf.kernel_ringbuf",
  "payload": {{"program_name": "{PROGRAM}", "bytes": 64}}
}}
"#
            ),
        )
        .unwrap();
        fs::write(
            &metadata,
            format!(
                r#"{{
  "schemaVersion": 1,
  "package": {{"name": "cyanrex-lab-1.2.3", "version": "1.2.3", "createdAt": "2026-09-04T01:02:03Z"}},
  "source": {{"revision": "{REVISION}", "state": "clean", "tag": "v1.2.3"}},
  "images": {{
    "mode": "built",
    "references": {{"engine": "engine:1.2.3", "frontend": "frontend:1.2.3", "postgres": "postgres:16"}},
    "contentIds": {{
      "engine": "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
      "frontend": "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
      "postgres": "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
    }},
    "archive": {{"sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}}
  }}
}}
"#
            ),
        )
        .unwrap();
        Self {
            root,
            environment,
            event,
            metadata,
        }
    }

    fn rust_create(&self, output: &Path) -> Output {
        Command::new(env!("CARGO_BIN_EXE_cyanrex-release"))
            .args(["evidence", "create", "--output"])
            .arg(output)
            .arg("--environment")
            .arg(&self.environment)
            .arg("--event")
            .arg(&self.event)
            .args([
                "--program-name",
                PROGRAM,
                "--pin-path",
                "/sys/fs/bpf/cyanrex/native-test",
                "--release-metadata",
            ])
            .arg(&self.metadata)
            .output()
            .unwrap()
    }

    fn rust_verify(&self, report: &Path) -> Output {
        Command::new(env!("CARGO_BIN_EXE_cyanrex-release"))
            .args(["evidence", "verify"])
            .arg(report)
            .arg("--release-metadata")
            .arg(&self.metadata)
            .args([
                "--expect-version",
                "1.2.3",
                "--expect-revision",
                REVISION,
                "--expect-tag",
                "v1.2.3",
                "--expect-source-state",
                "clean",
                "--expect-image-mode",
                "built",
            ])
            .output()
            .unwrap()
    }

    fn python_tool() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("scripts/live-kernel-evidence.py")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn python_and_rust_evidence_are_mutually_compatible() {
    let fixture = Fixture::new();
    let python_report = fixture.root.join("python-report.json");
    let python_create = Command::new("python3")
        .arg(Fixture::python_tool())
        .args(["create", "--output"])
        .arg(&python_report)
        .arg("--environment")
        .arg(&fixture.environment)
        .arg("--event")
        .arg(&fixture.event)
        .args([
            "--program-name",
            PROGRAM,
            "--pin-path",
            "/sys/fs/bpf/cyanrex/python-test",
            "--release-metadata",
        ])
        .arg(&fixture.metadata)
        .output()
        .unwrap();
    assert_success(&python_create);
    assert_success(&fixture.rust_verify(&python_report));

    let rust_report = fixture.root.join("rust-report.json");
    assert_success(&fixture.rust_create(&rust_report));
    let python_verify = Command::new("python3")
        .arg(Fixture::python_tool())
        .args(["verify"])
        .arg(&rust_report)
        .arg("--release-metadata")
        .arg(&fixture.metadata)
        .output()
        .unwrap();
    assert_success(&python_verify);

    let python: Value = serde_json::from_slice(&fs::read(python_report).unwrap()).unwrap();
    let rust: Value = serde_json::from_slice(&fs::read(rust_report).unwrap()).unwrap();
    assert_eq!(python["candidate"], rust["candidate"]);
    assert_eq!(python["environment"], rust["environment"]);
    assert_eq!(python["exercise"]["event"], rust["exercise"]["event"]);
}

#[test]
fn native_writer_rejects_duplicates_and_existing_outputs() {
    let fixture = Fixture::new();
    let duplicate = fixture.root.join("duplicate.json");
    fs::write(&duplicate, r#"{"schemaVersion":2,"schemaVersion":2}"#).unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_cyanrex-release"))
        .args(["evidence", "verify"])
        .arg(&duplicate)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("duplicate key"));

    let report = fixture.root.join("native-report.json");
    assert_success(&fixture.rust_create(&report));
    let original = fs::read(&report).unwrap();
    let overwrite = fixture.rust_create(&report);
    assert!(!overwrite.status.success());
    assert!(String::from_utf8_lossy(&overwrite.stderr).contains("output already exists"));
    assert_eq!(fs::read(&report).unwrap(), original);
    assert!(!fs::read_dir(&fixture.root).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".native-report.json.tmp-")
    }));
}

#[test]
fn native_verifier_accepts_v1_and_rejects_tampering() {
    let fixture = Fixture::new();
    let report = fixture.root.join("native-report.json");
    assert_success(&fixture.rust_create(&report));
    let original: Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();

    let mut legacy = original.clone();
    legacy["schemaVersion"] = Value::from(1);
    legacy["exercise"]["event"]
        .as_object_mut()
        .unwrap()
        .remove("programName");
    let legacy_path = fixture.root.join("legacy-report.json");
    fs::write(&legacy_path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert_success(&fixture.rust_verify(&legacy_path));

    let mut tampered = original;
    tampered["exercise"]["event"]["bytes"] = Value::from(0);
    let tampered_path = fixture.root.join("tampered-report.json");
    fs::write(&tampered_path, serde_json::to_vec(&tampered).unwrap()).unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_cyanrex-release"))
        .args(["evidence", "verify"])
        .arg(&tampered_path)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("positive integer"));

    let mut mismatched: Value =
        serde_json::from_slice(&fs::read(&fixture.metadata).unwrap()).unwrap();
    mismatched["images"]["archive"]["sha256"] = Value::from("c".repeat(64));
    let mismatched_path = fixture.root.join("mismatched-metadata.json");
    fs::write(&mismatched_path, serde_json::to_vec(&mismatched).unwrap()).unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_cyanrex-release"))
        .args(["evidence", "verify"])
        .arg(&report)
        .arg("--release-metadata")
        .arg(&mismatched_path)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("does not match"));
}
