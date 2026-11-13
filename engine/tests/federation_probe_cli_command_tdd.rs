#![cfg(target_os = "linux")]
//! Default subprocess checks always use a wrong identity, never the host LAN.

use std::{
    ffi::OsString,
    os::{
        fd::{FromRawFd, OwnedFd},
        unix::ffi::OsStringExt,
    },
    process::{Command, Stdio},
};

const CANARY: &str = "synthetic-probe-cli-private-value-must-not-echo";

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cyanrex-discovery"))
}

fn wrong_identity() -> Vec<String> {
    // SAFETY: read-only process identity observation; the selected UID cannot match this process.
    let uid = if unsafe { libc::getuid() } == 1 { 2 } else { 1 };
    [
        "probe-lan",
        "--uid",
        &uid.to_string(),
        "--gid",
        "1",
        "--groups",
        "none",
        "--interface",
        "nothere",
        "--address",
        "10.23.0.1",
        "--timeout-ms",
        "2000",
        "--instance",
        "node-one",
        "--host",
        "host-one",
        "--port",
        "4433",
        "--ttl",
        "60",
        "--advertisement",
        "{}",
    ]
    .into_iter()
    .map(Into::into)
    .collect()
}

fn reject(output: std::process::Output, code: i32) {
    assert_eq!(output.status.code(), Some(code));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty() && output.stderr.len() <= 512);
    assert!(!String::from_utf8_lossy(&output.stderr).contains(CANARY));
}

fn broken_pipe() -> OwnedFd {
    let mut fds = [-1; 2];
    // SAFETY: pipe2 initializes exactly these two fresh descriptors.
    assert_eq!(unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) }, 0);
    // SAFETY: each successfully created descriptor is transferred exactly once to its owner.
    let (reader, writer) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    drop(reader);
    writer
}

#[test]
fn global_help_lists_both_explicit_commands_within_the_existing_bound() {
    let output = command().arg("--help").output().unwrap();
    assert!(output.status.success() && output.stderr.is_empty());
    assert!(output.stdout.len() <= 512);
    let text = String::from_utf8(output.stdout).unwrap();
    for phrase in [
        "browse-lan",
        "probe-lan",
        "--local-node",
        "--advertisement",
        "--instance",
        "--ttl",
    ] {
        assert!(text.contains(phrase), "missing {phrase}");
    }
}

#[test]
fn process_gate_precedes_json_preparation_and_missing_interface_inspection() {
    for declaration in [
        "{}".into(),
        "not-json".into(),
        format!("{{\"{CANARY}\":true}}"),
        " ".repeat(4096),
    ] {
        let mut args = wrong_identity();
        args[22] = declaration;
        reject(command().args(args).output().unwrap(), 77);
    }
}

#[test]
fn all_malformed_command_shapes_are_usage_errors_without_input_reflection() {
    for length in 1..23 {
        reject(
            command()
                .args(wrong_identity().into_iter().take(length))
                .output()
                .unwrap(),
            64,
        );
    }
    for flag in [
        "--config",
        "--watch",
        "--retry",
        "--announce",
        "--register",
        "--advertisement-stdin",
    ] {
        let mut args = wrong_identity();
        args.extend([flag.into(), CANARY.into()]);
        reject(command().args(args).output().unwrap(), 64);
    }
    let mut args = wrong_identity();
    args[1] = CANARY.into();
    reject(command().args(args).output().unwrap(), 64);
}

#[test]
fn invalid_utf8_numeric_values_and_oversized_advertisements_fail_before_process_checks() {
    for (index, value) in [(12, "999"), (18, "0"), (20, "121")] {
        let mut args = wrong_identity();
        args[index] = value.into();
        reject(command().args(args).output().unwrap(), 64);
    }
    let mut args: Vec<OsString> = wrong_identity().into_iter().map(Into::into).collect();
    args[22] = OsString::from_vec(vec![0xff]);
    reject(command().args(&args).output().unwrap(), 64);
    args[22] = OsString::from("x".repeat(4097));
    reject(command().args(args).output().unwrap(), 64);
}

#[test]
fn inherited_settings_and_stdin_cannot_supply_a_missing_declaration_or_change_dispatch() {
    let result = command()
        .args(wrong_identity())
        .env("DATABASE_URL", CANARY)
        .env("CYANREX_PLATFORM_CONFIG", CANARY)
        .env("CYANREX_DISCOVERY_ADVERTISEMENT", CANARY)
        .stdin(Stdio::piped())
        .output()
        .unwrap();
    reject(result, 77);
    reject(
        command()
            .args(wrong_identity().into_iter().take(21))
            .env("CYANREX_DISCOVERY_ADVERTISEMENT", "{}")
            .output()
            .unwrap(),
        64,
    );
}

#[test]
fn extra_help_and_broken_help_keep_existing_fixed_exit_behavior() {
    reject(command().args(["--help", CANARY]).output().unwrap(), 64);
    reject(
        command()
            .arg("--help")
            .stdout(Stdio::from(broken_pipe()))
            .output()
            .unwrap(),
        74,
    );
}

#[test]
fn broken_error_pipe_preserves_both_usage_and_process_gate_codes() {
    for (args, code) in [(vec!["probe-lan".into()], 64), (wrong_identity(), 77)] {
        let output = command()
            .args(args)
            .stderr(Stdio::from(broken_pipe()))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(code));
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
    }
}
