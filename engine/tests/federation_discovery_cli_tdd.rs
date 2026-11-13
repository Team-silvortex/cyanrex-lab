#![cfg(target_os = "linux")]
//! Default command tests never open a host discovery socket.
use std::{
    ffi::OsString,
    os::{
        fd::{FromRawFd, OwnedFd},
        unix::ffi::OsStringExt,
    },
    process::{Command, Stdio},
};

const CANARY: &str = "synthetic-cli-private-value-must-not-echo-7e9b";

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cyanrex-discovery"))
}

fn broken_pipe() -> OwnedFd {
    let mut descriptors = [-1; 2];
    // SAFETY: pipe2 initializes two fresh descriptors into this real two-element output array.
    assert_eq!(
        unsafe { libc::pipe2(descriptors.as_mut_ptr(), libc::O_CLOEXEC) },
        0
    );
    // SAFETY: the successful pipe returned two separately owned descriptors, each wrapped once.
    let (reader, writer) = unsafe {
        (
            OwnedFd::from_raw_fd(descriptors[0]),
            OwnedFd::from_raw_fd(descriptors[1]),
        )
    };
    drop(reader);
    writer
}

fn wrong_identity() -> Vec<String> {
    // Read-only UID observation; the selected nonzero UID deliberately cannot match this process.
    let uid = if unsafe { libc::getuid() } == 1 { 2 } else { 1 };
    vec![
        "browse-lan".into(),
        "--uid".into(),
        uid.to_string(),
        "--gid".into(),
        "1".into(),
        "--groups".into(),
        "none".into(),
        "--interface".into(),
        "nothere".into(),
        "--address".into(),
        "10.23.0.1".into(),
        "--local-node".into(),
        "a".repeat(64),
        "--protocol".into(),
        "1:3".into(),
        "--timeout-ms".into(),
        "350".into(),
        "--capabilities".into(),
        "none".into(),
        "--require".into(),
        "none".into(),
    ]
}

fn reject(output: std::process::Output, code: i32) {
    assert_eq!(output.status.code(), Some(code));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty() && output.stderr.len() <= 512);
    assert!(!String::from_utf8_lossy(&output.stderr).contains(CANARY));
}

#[test]
fn exact_help_is_a_successful_fixed_description_without_process_or_network_work() {
    let result = command().arg("--help").output().unwrap();
    assert!(result.status.success());
    assert!(result.stderr.is_empty());
    assert!(result.stdout.len() <= 512);
    let text = String::from_utf8(result.stdout).unwrap();
    for field in [
        "cyanrex-discovery",
        "browse-lan",
        "--interface",
        "--local-node",
        "--protocol",
    ] {
        assert!(text.contains(field));
    }
}

#[test]
fn missing_unknown_and_extra_help_arguments_fail_without_echoing_inputs() {
    for args in [
        vec![],
        vec![CANARY],
        vec!["--help", CANARY],
        vec!["browse-lan"],
        vec!["serve", CANARY],
    ] {
        reject(command().args(args).output().unwrap(), 64);
    }
}

#[test]
fn non_utf8_and_oversized_arguments_are_usage_errors_with_no_snapshot() {
    let mut args: Vec<OsString> = wrong_identity().into_iter().map(Into::into).collect();
    args[8] = OsString::from_vec(vec![0xff]);
    reject(command().args(&args).output().unwrap(), 64);
    args[8] = OsString::from(CANARY.repeat(200));
    reject(command().args(&args).output().unwrap(), 64);
}

#[test]
fn unsafe_expected_process_is_rejected_before_even_a_missing_interface_is_inspected() {
    let result = command().args(wrong_identity()).output().unwrap();
    reject(result, 77);
}

#[test]
fn inherited_deployment_settings_cannot_select_a_database_config_or_change_the_command() {
    let result = command()
        .args(wrong_identity())
        .env("DATABASE_URL", CANARY)
        .env("CYANREX_DB_FALLBACK", "true")
        .env("CYANREX_DISCOVERY_INTERFACE", CANARY)
        .env("CYANREX_PLATFORM_CONFIG", CANARY)
        .output()
        .unwrap();
    reject(result, 77);
    reject(command().env("DATABASE_URL", CANARY).output().unwrap(), 64);
}

#[test]
fn public_relay_config_and_automatic_modes_are_not_silent_opt_ins() {
    for flag in [
        "--public-direct",
        "--relay",
        "--config",
        "--watch",
        "--register",
        "--join",
    ] {
        let mut args = wrong_identity();
        args.extend([flag.into(), CANARY.into()]);
        reject(command().args(args).output().unwrap(), 64);
    }
}

#[test]
fn broken_help_output_is_a_fixed_io_failure_not_a_panic_or_success() {
    let result = command()
        .arg("--help")
        .stdout(Stdio::from(broken_pipe()))
        .output()
        .unwrap();
    reject(result, 74);
}

#[test]
fn broken_diagnostic_output_preserves_the_usage_exit_code_without_panicking() {
    let result = command()
        .stderr(Stdio::from(broken_pipe()))
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(64));
    assert!(result.stdout.is_empty() && result.stderr.is_empty());
}
