use super::fixture::{denied, fifo, Fixture, CONTROLS};
use serde_json::Value;
use std::fs;
use std::os::unix::fs::symlink;

#[test]
fn controls_command_rejects_each_missing_or_tampered_control() {
    for name in CONTROLS {
        let missing = Fixture::new();
        fs::remove_file(missing.package.join(name)).unwrap();
        denied(&missing.run(), "installed package member");
        let tampered = Fixture::new();
        fs::write(tampered.package.join(name), "modified synthetic control\n").unwrap();
        denied(
            &tampered.run(),
            &format!("deployment control checksum mismatch: {name}"),
        );
    }
}

#[test]
fn controls_command_rejects_symlink_fifo_and_directory_controls_without_blocking() {
    for name in CONTROLS {
        for kind in ["symlink", "fifo", "directory"] {
            let fixture = Fixture::new();
            let path = fixture.package.join(name);
            fs::remove_file(&path).unwrap();
            match kind {
                "symlink" => symlink("release-metadata.json", &path).unwrap(),
                "fifo" => fifo(&path),
                "directory" => fs::create_dir(&path).unwrap(),
                _ => unreachable!(),
            }
            denied(&fixture.run(), "regular file");
        }
    }
}

#[test]
fn controls_command_rejects_manifest_and_metadata_inconsistency() {
    let inconsistent = Fixture::new();
    fs::write(inconsistent.package.join("release-metadata.json"), "{}\n").unwrap();
    denied(
        &inconsistent.run(),
        "checksum manifest does not match release-metadata.json",
    );

    let invalid = Fixture::new();
    fs::write(invalid.package.join("release-metadata.json"), "{}\n").unwrap();
    invalid.refresh_manifest();
    denied(&invalid.run(), "release metadata fields are invalid");

    let archive = Fixture::new();
    let path = archive.package.join("release-metadata.json");
    let mut metadata: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    metadata["images"]["archive"]["sha256"] = "0".repeat(64).into();
    fs::write(path, metadata.to_string()).unwrap();
    archive.refresh_manifest();
    denied(
        &archive.run(),
        "image archive SHA-256 does not match release metadata",
    );

    let duplicate = Fixture::new();
    let manifest = duplicate.package.join("checksums.sha256");
    let text = fs::read_to_string(&manifest).unwrap();
    fs::write(
        manifest,
        format!("{text}{}\n", text.lines().next().unwrap()),
    )
    .unwrap();
    denied(&duplicate.run(), "duplicate");
}

#[test]
fn controls_command_rejects_wrong_version_invalid_and_non_directory_paths() {
    let wrong_version = Fixture::version("0.0.0");
    denied(&wrong_version.run(), "installed package version mismatch");
    let fixture = Fixture::new();
    for path in [
        fixture.root.join("missing"),
        fixture.package.join("deploy.sh"),
    ] {
        denied(
            &fixture.command(&["package", "verify-controls"], Some(&path)),
            "installed package",
        );
    }
    let link = fixture.root.join("linked-package");
    symlink(&fixture.package, &link).unwrap();
    denied(
        &fixture.command(&["package", "verify-controls"], Some(&link)),
        "must be a directory",
    );
    let invalid = fixture.root.join("not-a-release-package");
    fs::create_dir(&invalid).unwrap();
    denied(
        &fixture.command(&["package", "verify-controls"], Some(&invalid)),
        "archive name",
    );
}

#[test]
fn controls_command_rejects_invalid_arguments_before_any_work() {
    let fixture = Fixture::new();
    denied(
        &fixture.command(&["package", "verify-controls"], None),
        "requires exactly one",
    );
    denied(
        &fixture.command(&["package", "verify-controls", "one", "two"], None),
        "requires exactly one",
    );
    for args in [
        &["package", "verify-controls", "--unknown", "value"][..],
        &["package", "verify-controls", "--expect-version", "0.0.0"][..],
        &[
            "package",
            "verify-controls",
            "--directory",
            "one",
            "--directory",
            "two",
        ][..],
    ] {
        denied(&fixture.command(args, None), "unknown option");
    }
}
