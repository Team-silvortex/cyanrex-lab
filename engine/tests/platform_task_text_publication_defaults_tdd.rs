#![cfg(target_os = "linux")]
//! Explicit Task-bound publication CLI grammar, not proof of resource authorization or saving.
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
    net::SocketAddr,
    os::unix::{ffi::OsStringExt, fs::OpenOptionsExt},
    path::Path,
};

fn arguments(f: &Fixture, config: &Path, bind: SocketAddr, tls: bool) -> Vec<OsString> {
    let mut args = legacy::arguments(f, config, bind);
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
fn platform_task_text_publication_cli_explicit_flag_before_optional_notify_and_ui_reaches_bad_input(
) {
    Fixture::checked(|f| {
        let db = legacy::listener();
        let bind = legacy::address();
        let receiver = receiver::Receiver::new();
        let bad = f.write("bad-config.json", b"{", 0o600);
        let missing = f.path("missing-config.json");
        for tls in [false, true] {
            for notify in [false, true] {
                for ui in [false, true] {
                    for input in [&bad, &missing] {
                        let mut args = arguments(f, input, bind, tls);
                        args.push("--task-text-publication".into());
                        if notify {
                            args.push("--notify-systemd".into());
                        }
                        if ui {
                            args.push("--session-ui".into());
                        }
                        let mut watch = Watch::new(&[&bad]);
                        let run = support::run(f, &args, Some(receiver.path().as_os_str()));
                        legacy::rejected(&run, 1);
                        assert_eq!(run.output.stderr, b"platform loopback host failed\n");
                        if f.identity.safe_ids() && input == &bad {
                            assert!(
                                watch.activity(),
                                "accepted flag grammar must reach its explicit malformed input"
                            );
                        }
                    }
                }
            }
        }
        assert!(legacy::no_dial(&db) && legacy::no_host(bind));
        assert!(receiver.take().is_none());
        assert!(receiver.control() && receiver.finish());
    });
}

#[test]
fn platform_task_text_publication_cli_wrong_order_alias_duplicate_and_diagnostics_precede_private_open(
) {
    Fixture::checked(|f| {
        let db = legacy::listener();
        let bind = legacy::address();
        let receiver = receiver::Receiver::new();
        let config = f.path("private-config-fifo");
        let cert = f.path("certificate-fifo");
        let key = f.path("key-fifo");
        for path in [&config, &cert, &key] {
            fifo(path);
        }
        let mut cases = Vec::<Vec<OsString>>::new();
        for tls in [false, true] {
            let base = arguments(f, &config, bind, tls);
            for suffix in [
                vec!["--task-text-publication", "--task-text-publication"],
                vec!["--notify-systemd", "--task-text-publication"],
                vec!["--session-ui", "--task-text-publication"],
                vec![
                    "--task-text-publication",
                    "--session-ui",
                    "--notify-systemd",
                ],
                vec![
                    "--task-text-publication",
                    "--notify-systemd",
                    "--notify-systemd",
                ],
                vec![
                    "--task-text-publication",
                    "--notify-systemd",
                    "--session-ui",
                    "--session-ui",
                ],
                vec![
                    "--task-text-publication",
                    "--notify-systemd",
                    "--session-ui",
                    "extra-nineteenth-argument",
                ],
                vec!["--task-text-publication", "true"],
                vec!["--task-text-publication=true"],
                vec!["--task-text-publication=1"],
                vec!["--TASK-TEXT-PUBLICATION"],
                vec!["--task_text_publication"],
            ] {
                let mut args = base.clone();
                args.extend(suffix.into_iter().map(OsString::from));
                cases.push(args);
            }
            for index in [7, 8] {
                let mut middle = base.clone();
                middle.insert(index, "--task-text-publication".into());
                cases.push(middle);
            }
            let mut non_utf8 = base.clone();
            non_utf8.push(OsString::from_vec(vec![0xff]));
            cases.push(non_utf8);
            let mut huge = base;
            huge.push("x".repeat(4097).into());
            cases.push(huge);
        }
        for command in ["check-process", "check-config", "check-storage"] {
            for notify in [false, true] {
                for ui in [false, true] {
                    let mut args = f.arguments(&config);
                    args[0] = command.into();
                    if command == "check-process" {
                        args.truncate(7);
                    }
                    args.push("--task-text-publication".into());
                    if notify {
                        args.push("--notify-systemd".into());
                    }
                    if ui {
                        args.push("--session-ui".into());
                    }
                    cases.push(args);
                }
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
            "rejected grammar opened private configuration or TLS input"
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
        assert!(legacy::no_dial(&db) && legacy::no_host(bind) && receiver.take().is_none());
        assert!(receiver.control() && receiver.finish());
    });
}

#[test]
fn platform_task_text_publication_cli_valid_flags_preserve_identity_before_private_input() {
    Fixture::checked(|f| {
        let db = legacy::listener();
        let bind = legacy::address();
        let receiver = receiver::Receiver::new();
        let config = f.path("private-config-fifo");
        let cert = f.path("certificate-fifo");
        let key = f.path("key-fifo");
        for path in [&config, &cert, &key] {
            fifo(path);
        }
        let mut watch = Watch::new(&[&config, &cert, &key]);
        for tls in [false, true] {
            for (index, current) in [(2, f.identity.uid[0]), (4, f.identity.gid[0])] {
                let mut args = arguments(f, &config, bind, tls);
                args[index] = if current == 1 { "2" } else { "1" }.into();
                args.extend([
                    "--task-text-publication".into(),
                    "--notify-systemd".into(),
                    "--session-ui".into(),
                ]);
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
        assert!(legacy::no_dial(&db) && legacy::no_host(bind) && receiver.take().is_none());
        assert!(receiver.control() && receiver.finish());
    });
}
