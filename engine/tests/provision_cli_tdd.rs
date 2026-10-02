#![cfg(unix)]
#[allow(dead_code)]
#[path = "provision_cli/local.rs"]
mod local;
use local::*;
use serde_json::json;
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    process::Command,
};

#[test]
fn provision_help_and_version_never_connect_or_load_runtime() {
    for flag in ["--help", "--version"] {
        let result = Command::new(env!("CARGO_BIN_EXE_cyanrex-provision"))
            .arg(flag)
            .env("DATABASE_URL", "not-a-url")
            .output()
            .unwrap();
        assert!(result.status.success());
        assert!(result.stderr.is_empty());
        assert!(!String::from_utf8_lossy(&result.stdout).contains("\n+"));
    }
    let f = Local::new();
    rejected(&f.run("apply"), "invalid_arguments");
    assert!(!f.enrollment.exists());
}

#[test]
fn provision_reconciliation_accepts_no_write_options_or_runtime_fallback() {
    let f = Local::new();
    rejected(&f.run("reconcile"), "storage_unavailable");
    for option in [
        "--password-file",
        "--enrollment-file",
        "--confirm",
        "--repair",
    ] {
        rejected(
            &f.command("reconcile")
                .args([option, "unused"])
                .output()
                .unwrap(),
            "invalid_arguments",
        );
    }
    assert!(!f.enrollment.exists());
}

#[test]
fn provision_arguments_are_strict_and_do_not_echo_values() {
    let f = Local::new();
    for args in [
        vec!["synthetic-database-secret"],
        vec!["--password", PASSWORD],
        vec!["--config", "duplicate"],
        vec!["--confirm", "wrong"],
        vec!["--help", "extra"],
    ] {
        rejected(
            &f.command("plan").args(args).output().unwrap(),
            "invalid_arguments",
        );
    }
    rejected(
        &f.apply("not-a-digest").output().unwrap(),
        "invalid_arguments",
    );
}

#[test]
fn provision_config_rejects_ambiguous_and_nonlocal_targets_before_connecting() {
    let f = Local::new();
    for (key, value) in [
        ("format_version", json!(2)),
        ("unknown", json!(PASSWORD)),
        ("schema", json!("public")),
        ("schema", json!("pg_shadow")),
        ("schema", json!("safe,public")),
        ("schema", json!("missing; DROP SCHEMA public")),
        ("port", json!(0)),
        (
            "authority_id",
            json!("00000000-0000-0000-0000-000000000000"),
        ),
        ("username", json!("bad user")),
        (
            "transport",
            json!({"kind":"loopback", "address":"example.com"}),
        ),
        (
            "transport",
            json!({"kind":"loopback", "address":"192.168.1.2"}),
        ),
        ("transport", json!({"kind":"unix", "directory":"relative"})),
    ] {
        let mut target = Local::target();
        target[key] = value;
        f.set_config(&target);
        rejected(&f.run("plan"), "invalid_config");
    }
    for text in ["{bad-json", "{\"format_version\":1,\"format_version\":1}"] {
        f.write(&f.config, text.as_bytes());
        rejected(&f.run("plan"), "invalid_config");
    }
}

#[test]
fn provision_input_files_require_private_owned_bounded_regular_files() {
    let f = Local::new();
    fs::set_permissions(&f.config, fs::Permissions::from_mode(0o644)).unwrap();
    rejected(&f.run("plan"), "unsafe_input_file");
    fs::set_permissions(&f.config, fs::Permissions::from_mode(0o600)).unwrap();
    let link = f.root.join("hardlink");
    fs::hard_link(&f.config, &link).unwrap();
    rejected(&f.run("plan"), "unsafe_input_file");
    fs::remove_file(link).unwrap();
    f.write(&f.config, &vec![b'x'; 16385]);
    rejected(&f.run("plan"), "unsafe_input_file");
    fs::remove_file(&f.config).unwrap();
    symlink(&f.password, &f.config).unwrap();
    rejected(&f.run("plan"), "unsafe_input_file");
}

#[test]
fn provision_rejects_symlink_and_untrusted_directory_components() {
    let f = Local::new();
    let link = f.root.join("linked");
    symlink(&f.root, &link).unwrap();
    rejected(
        &Command::new(env!("CARGO_BIN_EXE_cyanrex-provision"))
            .args(["plan", "--config"])
            .arg(link.join("target.json"))
            .output()
            .unwrap(),
        "unsafe_input_file",
    );
    fs::set_permissions(&f.root, fs::Permissions::from_mode(0o777)).unwrap();
    rejected(&f.run("plan"), "unsafe_input_file");
    fs::set_permissions(&f.root, fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn provision_invalid_password_is_rejected_before_database_or_output() {
    let f = Local::new();
    for bytes in [b"short".to_vec(), vec![b'x'; 4097], vec![0xff; 12]] {
        f.write(&f.password, &bytes);
        let result = f.apply(&"a".repeat(64)).output().unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        redacted(&result);
        assert!(!f.enrollment.exists());
    }
}

#[test]
fn provision_fifo_input_is_rejected_without_blocking() {
    use std::{
        ffi::CString,
        os::unix::ffi::OsStrExt,
        time::{Duration, Instant},
    };
    let f = Local::new();
    fs::remove_file(&f.config).unwrap();
    let name = CString::new(f.config.as_os_str().as_bytes()).unwrap();
    // SAFETY: a valid C pathname in this test's owned temporary directory.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let mut child = f
        .command("plan")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("FIFO input blocked the CLI");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    rejected(&child.wait_with_output().unwrap(), "unsafe_input_file");
}

#[test]
fn provision_unavailable_database_has_no_runtime_fallback_or_output_file() {
    let f = Local::new();
    rejected(&f.run("plan"), "storage_unavailable");
    rejected(&f.run("inspect"), "storage_unavailable");
    rejected(
        &f.apply(&"a".repeat(64)).output().unwrap(),
        "storage_unavailable",
    );
    assert!(!f.enrollment.exists());
    assert_eq!(fs::read_dir(&f.root).unwrap().count(), 2);
}
