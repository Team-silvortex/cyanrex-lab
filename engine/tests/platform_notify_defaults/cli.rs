use super::{
    fixture::{fifo, Fixture, Watch},
    legacy,
    receiver::Receiver,
    support,
};
use std::{
    ffi::OsString,
    fs::OpenOptions,
    os::unix::{ffi::OsStringExt, fs::OpenOptionsExt},
};

#[test]
fn platform_notify_opt_in_flag_is_accepted_then_missing_socket_fails_before_config() {
    let run = Fixture::checked(|f| {
        let bind = legacy::address();
        let path = f.path("never-opened.json");
        let mut args = legacy::arguments(f, &path, bind);
        args.push("--notify-systemd".into());
        support::run(f, &args, None)
    });
    legacy::rejected(&run, 1);
}

fn tls(mut args: Vec<OsString>, f: &Fixture) -> Vec<OsString> {
    args[0] = "serve-loopback-tls".into();
    args.extend([
        "--cert".into(),
        f.path("certificate.pem").into_os_string(),
        "--key".into(),
        f.path("key.pem").into_os_string(),
    ]);
    args
}

#[test]
fn platform_notify_flag_requires_final_singleton_serve_only_grammar_before_sensitive_open() {
    let (runs, unopened, positive, no_dial, silent, oracle) = Fixture::checked(|f| {
        let receiver = Receiver::new();
        let db = legacy::listener();
        let pipe = f.path("private-fifo.json");
        fifo(&pipe);
        let plain = legacy::arguments(f, &pipe, legacy::address());
        let tls = tls(plain.clone(), f);
        let mut cases = Vec::new();
        for baseline in [plain, tls] {
            let mut duplicate = baseline.clone();
            duplicate.extend(["--notify-systemd".into(), "--notify-systemd".into()]);
            cases.push(duplicate);
            let mut value = baseline.clone();
            value.extend(["--notify-systemd".into(), "true".into()]);
            cases.push(value);
            let mut equals = baseline.clone();
            equals.push("--notify-systemd=true".into());
            cases.push(equals);
            let mut reordered = baseline.clone();
            reordered.insert(7, "--notify-systemd".into());
            cases.push(reordered);
            let mut non_utf8 = baseline;
            non_utf8.push(OsString::from_vec(vec![0xff]));
            cases.push(non_utf8);
        }
        for command in ["check-process", "check-config", "check-storage"] {
            let mut args = f.arguments(&pipe);
            args[0] = command.into();
            if command == "check-process" {
                args.truncate(7);
            }
            args.push("--notify-systemd".into());
            cases.push(args);
        }
        let mut watch = Watch::new(&[&pipe]);
        let runs = cases
            .iter()
            .map(|args| support::run(f, args, Some(receiver.path().as_os_str())))
            .collect::<Vec<_>>();
        let unopened = !watch.activity();
        let control = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&pipe)
            .unwrap();
        let positive = watch.activity();
        drop(control);
        let silent = receiver.take().is_none();
        let oracle = receiver.control() && receiver.finish();
        (
            runs,
            unopened,
            positive,
            legacy::no_dial(&db),
            silent,
            oracle,
        )
    });
    for run in runs {
        legacy::rejected(&run, 64);
    }
    assert!(unopened && positive && no_dial && silent && oracle);
}

#[test]
fn platform_notify_enabled_missing_malformed_or_unreachable_socket_precedes_fifo_and_dial() {
    let (runs, unopened, positive, no_dial, no_host) = Fixture::checked(|f| {
        let pipe = f.path("notify-before-input.json");
        fifo(&pipe);
        let db = legacy::listener();
        let bind = legacy::address();
        let mut watch = Watch::new(&[&pipe]);
        let values = [
            None,
            Some(OsString::new()),
            Some("relative.sock".into()),
            Some("@".into()),
            Some(f.path("missing.sock").into_os_string()),
            Some(OsString::from_vec(vec![0xff])),
            Some("vsock:1:2".into()),
            Some(format!("@{}", "x".repeat(108)).into()),
            Some(format!("/{}", "x".repeat(4096)).into()),
        ];
        let mut runs = Vec::new();
        for args in [
            legacy::arguments(f, &pipe, bind),
            tls(legacy::arguments(f, &pipe, bind), f),
        ] {
            let mut args = args;
            args.push("--notify-systemd".into());
            for value in &values {
                runs.push(support::run(f, &args, value.as_deref()));
            }
        }
        let unopened = !watch.activity();
        let control = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&pipe)
            .unwrap();
        let positive = watch.activity();
        drop(control);
        (
            runs,
            unopened,
            positive,
            legacy::no_dial(&db),
            legacy::no_host(bind),
        )
    });
    for run in runs {
        legacy::rejected(&run, 1);
    }
    assert!(unopened && positive && no_dial && no_host);
}

#[test]
fn platform_notify_disabled_ignores_ambient_socket_without_activation_and_reaches_explicit_peer() {
    let (runs, reached, safe, silent, oracle, no_retry, no_host) = Fixture::checked(|f| {
        let receiver = Receiver::new();
        let db = legacy::listener();
        let bind = legacy::address();
        let root = legacy::root(f, "artifacts");
        let input = legacy::input(
            f,
            "valid.json",
            &legacy::document(&root, db.local_addr().unwrap().port(), bind),
        );
        let mut runs = Vec::new();
        let mut reached = Vec::new();
        for value in [
            receiver.path().as_os_str().to_owned(),
            "relative-invalid.socket".into(),
            OsString::from_vec(vec![0xff]),
        ] {
            let peer = db.try_clone().unwrap();
            let task = std::thread::spawn(move || {
                use std::{
                    io::Read,
                    time::{Duration, Instant},
                };
                let until = Instant::now() + Duration::from_secs(3);
                loop {
                    match peer.accept() {
                        Ok((mut socket, _)) => {
                            socket
                                .set_read_timeout(Some(Duration::from_secs(1)))
                                .unwrap();
                            let mut prefix = [0; 8];
                            return socket.read_exact(&mut prefix).is_ok()
                                && prefix[4..] == 196608u32.to_be_bytes();
                        }
                        Err(e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                && Instant::now() < until =>
                        {
                            std::thread::sleep(Duration::from_millis(5))
                        }
                        _ => return false,
                    }
                }
            });
            runs.push(support::run(
                f,
                &legacy::arguments(f, &input, bind),
                Some(&value),
            ));
            reached.push(task.join().unwrap());
        }
        let silent = receiver.take().is_none();
        let oracle = receiver.control() && receiver.finish();
        (
            runs,
            reached,
            f.identity.safe_ids(),
            silent,
            oracle,
            legacy::no_dial(&db),
            legacy::no_host(bind),
        )
    });
    for run in runs {
        legacy::rejected(&run, 1);
    }
    assert!(silent && oracle && no_retry && no_host);
    assert!(
        reached.iter().all(|value| *value == safe),
        "only safe capability-reduced identity may reach explicit PostgreSQL peer"
    );
}
