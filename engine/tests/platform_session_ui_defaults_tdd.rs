#![cfg(target_os = "linux")]
//! Explicit Session-page CLI grammar and pre-secret rejection, not live browser authorization.
#[allow(dead_code)]
#[path = "platform_config_input/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "platform_config_input/kernel.rs"]
mod kernel;
#[allow(dead_code)]
#[path = "platform_serve_loopback_defaults/support.rs"]
mod legacy;
#[allow(dead_code)]
#[path = "platform_notify_defaults/receiver.rs"]
mod receiver;
#[path = "platform_notify_defaults/support.rs"]
mod support;

use fixture::{fifo, Fixture, Watch};
use std::{
    ffi::OsString,
    fs::OpenOptions,
    os::unix::{ffi::OsStringExt, fs::OpenOptionsExt},
    path::Path,
};

fn arguments(f: &Fixture, config: &Path, tls: bool) -> Vec<OsString> {
    let mut args = legacy::arguments(f, config, legacy::address());
    if tls {
        args[0] = "serve-loopback-tls".into();
        args.extend([
            "--cert".into(),
            f.path("certificate-fifo").into_os_string(),
            "--key".into(),
            f.path("key-fifo").into_os_string(),
        ]);
    }
    args
}

#[test]
fn platform_session_ui_cli_final_flag_and_notify_then_ui_reach_explicit_bad_configuration() {
    Fixture::checked(|f| {
        let db = legacy::listener();
        let receiver = receiver::Receiver::new();
        let bad = f.write("bad-config.json", b"{", 0o600);
        let missing = f.path("missing-config.json");
        for tls in [false, true] {
            for notify in [false, true] {
                for input in [&bad, &missing] {
                    let mut args = arguments(f, input, tls);
                    if notify {
                        args.push("--notify-systemd".into());
                    }
                    args.push("--session-ui".into());
                    let mut watch = Watch::new(&[&bad]);
                    let run = support::run(f, &args, Some(receiver.path().as_os_str()));
                    legacy::rejected(&run, 1);
                    assert_eq!(run.output.stderr, b"platform loopback host failed\n");
                    if f.identity.safe_ids() && input == &bad {
                        assert!(
                            watch.activity(),
                            "accepted grammar must reach its explicit malformed input"
                        );
                    }
                }
            }
        }
        assert!(legacy::no_dial(&db));
        assert!(receiver.take().is_none());
        assert!(receiver.control() && receiver.finish());
    });
}

#[test]
fn platform_session_ui_cli_invalid_suffixes_and_diagnostic_modes_refuse_before_private_fifo_open() {
    Fixture::checked(|f| {
        let receiver = receiver::Receiver::new();
        let db = legacy::listener();
        let config = f.path("private-config-fifo");
        let cert = f.path("certificate-fifo");
        let key = f.path("key-fifo");
        for path in [&config, &cert, &key] {
            fifo(path);
        }
        let mut cases = Vec::<Vec<OsString>>::new();
        for tls in [false, true] {
            let base = arguments(f, &config, tls);
            for suffix in [
                vec!["--session-ui", "--session-ui"],
                vec!["--session-ui", "--notify-systemd"],
                vec!["--notify-systemd", "--notify-systemd", "--session-ui"],
                vec!["--notify-systemd", "--session-ui", "--session-ui"],
                vec![
                    "--notify-systemd",
                    "--session-ui",
                    "unexpected-eighteenth-argument",
                ],
                vec!["--session-ui", "true"],
                vec!["--session-ui=true"],
                vec!["--session-ui=1"],
                vec!["--SESSION-UI"],
                vec!["--session_ui"],
            ] {
                let mut args = base.clone();
                args.extend(suffix.into_iter().map(OsString::from));
                cases.push(args);
            }
            let mut middle = base.clone();
            middle.insert(7, "--session-ui".into());
            cases.push(middle);
            let mut inside = base.clone();
            inside.insert(8, "--session-ui".into());
            cases.push(inside);
            let mut non_utf8 = base;
            non_utf8.push(OsString::from_vec(vec![0xff]));
            cases.push(non_utf8);
        }
        for command in ["check-process", "check-config", "check-storage"] {
            for notify in [false, true] {
                let mut args = f.arguments(&config);
                args[0] = command.into();
                if command == "check-process" {
                    args.truncate(7);
                }
                if notify {
                    args.push("--notify-systemd".into());
                }
                args.push("--session-ui".into());
                cases.push(args);
            }
        }
        let mut watch = Watch::new(&[&config, &cert, &key]);
        for args in cases {
            legacy::rejected(
                &support::run(f, &args, Some(receiver.path().as_os_str())),
                64,
            );
        }
        assert!(
            !watch.activity(),
            "invalid UI grammar opened private configuration/TLS input"
        );
        let control = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&key)
            .unwrap();
        assert!(
            watch.activity(),
            "independent FIFO watcher positive control did not fire"
        );
        drop(control);
        assert!(legacy::no_dial(&db) && receiver.take().is_none());
        assert!(receiver.control() && receiver.finish());
    });
}

#[test]
fn platform_session_ui_cli_wrong_identity_precedes_private_inputs_even_with_both_valid_flags() {
    Fixture::checked(|f| {
        let receiver = receiver::Receiver::new();
        let db = legacy::listener();
        let config = f.path("private-config-fifo");
        let cert = f.path("certificate-fifo");
        let key = f.path("key-fifo");
        for path in [&config, &cert, &key] {
            fifo(path);
        }
        let mut watch = Watch::new(&[&config, &cert, &key]);
        for tls in [false, true] {
            for (index, current) in [(2, f.identity.uid[0]), (4, f.identity.gid[0])] {
                let mut args = arguments(f, &config, tls);
                args[index] = if current == 1 { "2" } else { "1" }.into();
                args.extend(["--notify-systemd".into(), "--session-ui".into()]);
                let run = support::run(f, &args, Some(receiver.path().as_os_str()));
                legacy::rejected(&run, 1);
                assert_eq!(run.output.stderr, b"platform loopback host failed\n");
            }
        }
        assert!(!watch.activity());
        let control = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&config)
            .unwrap();
        assert!(watch.activity());
        drop(control);
        assert!(legacy::no_dial(&db) && receiver.take().is_none());
        assert!(receiver.control() && receiver.finish());
    });
}
