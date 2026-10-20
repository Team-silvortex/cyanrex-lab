use super::fixture::{fifo, success, Fixture};
use std::fs;

#[test]
fn controls_command_is_discoverable_and_describes_its_read_only_scope() {
    let fixture = Fixture::new();
    for args in [&["--help"][..], &["package", "--help"][..]] {
        let output = fixture.command(args, None);
        success(&output);
        assert!(String::from_utf8_lossy(&output.stdout).contains("verify-controls"));
    }
}

#[test]
fn controls_command_accepts_current_controls_without_runtime_configuration() {
    let fixture = Fixture::new();
    let output = fixture.run();
    success(&output);
    let text = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "deploy.sh",
        "docker-compose.yml",
        "cyanrex-release",
        "metadata",
        "not",
        "health",
    ] {
        assert!(text.contains(expected), "missing success scope: {expected}");
    }
    assert!(!fixture.package.join(".env").exists());
}

#[test]
fn controls_command_ignores_runtime_files_and_never_executes_env_or_external_tools() {
    let fixture = Fixture::new();
    fixture.runtime();
    success(&fixture.run());
}

#[test]
fn controls_command_does_not_read_runtime_env_or_image_archive_bodies() {
    let fixture = Fixture::new();
    fifo(&fixture.package.join(".env"));
    fs::remove_file(fixture.package.join("cyanrex-images.tar")).unwrap();
    fifo(&fixture.package.join("cyanrex-images.tar"));
    success(&fixture.run());
    fs::remove_file(fixture.package.join("cyanrex-images.tar")).unwrap();
    success(&fixture.run());
}

#[test]
fn original_full_package_verifier_still_rejects_runtime_extras() {
    let fixture = Fixture::new();
    success(&fixture.command(&["package", "verify"], Some(&fixture.package)));
    fs::write(
        fixture.package.join(".env"),
        "synthetic-private-runtime-value\n",
    )
    .unwrap();
    super::fixture::denied(
        &fixture.command(&["package", "verify"], Some(&fixture.package)),
        "file set is invalid",
    );
    success(&fixture.run());
}
