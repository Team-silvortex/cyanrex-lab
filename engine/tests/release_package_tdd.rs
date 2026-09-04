use flate2::write::GzEncoder;
use flate2::Compression;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const PACKAGE_ROOT: &str = "cyanrex-lab-1.2.3-20260904-010203";
const ARCHIVE_NAME: &str = "cyanrex-lab-1.2.3-20260904-010203.tar.gz";
const REVISION: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[derive(Clone, Copy)]
enum ArchiveMutation {
    None,
    Duplicate,
    Symlink,
    Traversal,
    Unverified,
    InternalTamper,
    DuplicateMetadataKey,
}

struct Fixture {
    root: PathBuf,
    bundle: PathBuf,
}

impl Fixture {
    fn new(mutation: ArchiveMutation) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cyanrex-release-package-{}-{unique}",
            std::process::id()
        ));
        let source = root.join("source").join(PACKAGE_ROOT);
        let bundle = root.join("bundle");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir(&bundle).unwrap();

        fs::write(source.join("docker-compose.yml"), b"services: {}\n").unwrap();
        fs::write(source.join("cyanrex-images.tar"), b"mock image archive\n").unwrap();
        fs::write(source.join("run.sh"), b"#!/usr/bin/env bash\necho ready\n").unwrap();
        set_mode(&source.join("run.sh"), 0o755);
        let image_hash = sha256_file(&source.join("cyanrex-images.tar"));
        let metadata = format!(
            r#"{{
  "schemaVersion": 1,
  "package": {{"name": "{PACKAGE_ROOT}", "version": "1.2.3", "createdAt": "2026-09-04T01:02:03Z"}},
  "source": {{"revision": "{REVISION}", "state": "clean", "tag": "v1.2.3"}},
  "compose": {{"file": "docker-compose.yml", "source": "docker/docker-compose.distribution.yml"}},
  "images": {{
    "mode": "built",
    "references": {{"engine": "engine:1.2.3", "frontend": "frontend:1.2.3", "postgres": "postgres:16"}},
    "contentIds": {{
      "engine": "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
      "frontend": "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
      "postgres": "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
    }},
    "archive": {{"file": "cyanrex-images.tar", "sha256": "{image_hash}"}}
  }},
  "integrity": {{"algorithm": "sha256", "manifest": "checksums.sha256"}}
}}
"#
        );
        let metadata = if matches!(mutation, ArchiveMutation::DuplicateMetadataKey) {
            metadata.replacen(
                "\"schemaVersion\": 1,",
                "\"schemaVersion\": 1,\n  \"schemaVersion\": 1,",
                1,
            )
        } else {
            metadata
        };
        fs::write(source.join("release-metadata.json"), metadata).unwrap();
        let mut manifest = String::new();
        for name in [
            "cyanrex-images.tar",
            "docker-compose.yml",
            "release-metadata.json",
            "run.sh",
        ] {
            manifest.push_str(&format!("{}  {name}\n", sha256_file(&source.join(name))));
        }
        fs::write(source.join("checksums.sha256"), manifest).unwrap();
        if matches!(mutation, ArchiveMutation::InternalTamper) {
            fs::write(
                source.join("docker-compose.yml"),
                b"services: {tampered: true}\n",
            )
            .unwrap();
        }

        let archive_path = bundle.join(ARCHIVE_NAME);
        write_archive(&archive_path, &source, mutation);
        fs::write(
            bundle.join(format!("{ARCHIVE_NAME}.sha256")),
            format!("{}  {ARCHIVE_NAME}\n", sha256_file(&archive_path)),
        )
        .unwrap();
        Self { root, bundle }
    }

    fn extract(&self, output: &Path) -> Output {
        Command::new(env!("CARGO_BIN_EXE_cyanrex-release"))
            .args(["package", "extract"])
            .arg(&self.bundle)
            .arg("--output")
            .arg(output)
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
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write_archive(path: &Path, source: &Path, mutation: ArchiveMutation) {
    let encoder = GzEncoder::new(File::create(path).unwrap(), Compression::default());
    let mut archive = tar::Builder::new(encoder);
    archive.append_dir(PACKAGE_ROOT, source).unwrap();
    for name in [
        "checksums.sha256",
        "cyanrex-images.tar",
        "docker-compose.yml",
        "release-metadata.json",
        "run.sh",
    ] {
        archive
            .append_path_with_name(source.join(name), format!("{PACKAGE_ROOT}/{name}"))
            .unwrap();
    }
    match mutation {
        ArchiveMutation::None
        | ArchiveMutation::InternalTamper
        | ArchiveMutation::DuplicateMetadataKey => {}
        ArchiveMutation::Duplicate => {
            archive
                .append_path_with_name(source.join("run.sh"), format!("{PACKAGE_ROOT}/run.sh"))
                .unwrap();
        }
        ArchiveMutation::Symlink => {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_mode(0o777);
            header.set_mtime(0);
            header.set_link_name("run.sh").unwrap();
            header.set_cksum();
            archive
                .append_data(
                    &mut header,
                    format!("{PACKAGE_ROOT}/linked-run"),
                    Cursor::new(Vec::new()),
                )
                .unwrap();
        }
        ArchiveMutation::Traversal => {
            let content = b"escape\n";
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Regular);
            header.set_size(content.len() as u64);
            header.set_mode(0o644);
            header.set_mtime(0);
            header.set_path("placeholder").unwrap();
            let unsafe_name = format!("{PACKAGE_ROOT}/../escape");
            header.as_mut_bytes()[..100].fill(0);
            header.as_mut_bytes()[..unsafe_name.len()].copy_from_slice(unsafe_name.as_bytes());
            header.set_cksum();
            archive.append(&header, Cursor::new(content)).unwrap();
        }
        ArchiveMutation::Unverified => {
            let content = b"not checksummed\n";
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Regular);
            header.set_size(content.len() as u64);
            header.set_mode(0o644);
            header.set_mtime(0);
            header.set_cksum();
            archive
                .append_data(
                    &mut header,
                    format!("{PACKAGE_ROOT}/unverified.txt"),
                    Cursor::new(content),
                )
                .unwrap();
        }
    }
    archive.finish().unwrap();
    archive.into_inner().unwrap().finish().unwrap();
}

fn sha256_file(path: &Path) -> String {
    let source = fs::read(path).unwrap();
    Sha256::digest(source)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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
fn rust_and_python_extract_the_same_verified_package() {
    let fixture = Fixture::new(ArchiveMutation::None);
    let rust_output = fixture.root.join("rust-output");
    assert_success(&fixture.extract(&rust_output));
    let rust_package = rust_output.join(PACKAGE_ROOT);
    assert_eq!(
        fs::read(rust_package.join("run.sh")).unwrap(),
        b"#!/usr/bin/env bash\necho ready\n"
    );
    assert_eq!(file_mode(&rust_package.join("run.sh")), 0o755);
    assert_eq!(
        file_mode(&rust_package.join("release-metadata.json")),
        0o644
    );

    let python_output = fixture.root.join("python-output");
    let python_tool = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("scripts/release-package.py");
    let python = Command::new("python3")
        .arg(python_tool)
        .arg("extract")
        .arg(&fixture.bundle)
        .arg("--output")
        .arg(&python_output)
        .output()
        .unwrap();
    assert_success(&python);
    for name in [
        "checksums.sha256",
        "cyanrex-images.tar",
        "docker-compose.yml",
        "release-metadata.json",
        "run.sh",
    ] {
        assert_eq!(
            fs::read(rust_package.join(name)).unwrap(),
            fs::read(python_output.join(PACKAGE_ROOT).join(name)).unwrap()
        );
    }

    let original = fs::read(rust_package.join("run.sh")).unwrap();
    let overwrite = fixture.extract(&rust_output);
    assert!(!overwrite.status.success());
    assert!(String::from_utf8_lossy(&overwrite.stderr).contains("refusing to overwrite"));
    assert_eq!(fs::read(rust_package.join("run.sh")).unwrap(), original);
}

#[test]
fn native_extractor_rejects_unsafe_or_unverified_members() {
    for (mutation, expected) in [
        (ArchiveMutation::Duplicate, "duplicate member"),
        (ArchiveMutation::Symlink, "unsupported member type"),
        (ArchiveMutation::Traversal, "escapes its package root"),
        (
            ArchiveMutation::Unverified,
            "checksum manifest file set is invalid",
        ),
        (
            ArchiveMutation::InternalTamper,
            "checksum manifest does not match",
        ),
        (ArchiveMutation::DuplicateMetadataKey, "duplicate key"),
    ] {
        let fixture = Fixture::new(mutation);
        let output_path = fixture.root.join("rejected-output");
        let output = fixture.extract(&output_path);
        assert!(
            !output.status.success(),
            "unsafe archive unexpectedly passed"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "wrong rejection for mutation: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output_path.exists());
    }
}

#[test]
fn native_extractor_rejects_outer_checksum_tampering() {
    let fixture = Fixture::new(ArchiveMutation::None);
    fs::write(
        fixture.bundle.join(format!("{ARCHIVE_NAME}.sha256")),
        format!("{}  {ARCHIVE_NAME}\n", "0".repeat(64)),
    )
    .unwrap();
    let output_path = fixture.root.join("rejected-output");
    let output = fixture.extract(&output_path);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("SHA-256 mismatch"));
    assert!(!output_path.exists());
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) {}

#[cfg(unix)]
fn file_mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[cfg(not(unix))]
fn file_mode(_path: &Path) -> u32 {
    0
}
