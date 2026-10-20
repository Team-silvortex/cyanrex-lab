use super::{
    fixture::{fifo, Fixture, Watch, CANARY},
    support,
};
use serde_json::json;
use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    os::unix::{ffi::OsStringExt, fs::OpenOptionsExt},
};

#[test]
fn platform_serve_cli_grammar_and_literal_bind_reject_before_sensitive_input_open() {
    let (runs, boundaries, no_open, positive) = Fixture::checked(|fixture| {
        let pipe = fixture.path("private-fifo.json");
        fifo(&pipe);
        let bind = support::address();
        let baseline = support::arguments(fixture, &pipe, bind);
        let mut cases = vec![Vec::<OsString>::new(), vec!["serve-loopback".into()]];
        for raw in [
            "0.0.0.0:12000",
            "127.0.0.2:12000",
            "192.0.2.1:12000",
            "localhost:12000",
            "[::]:12000",
            "[::2]:12000",
            "[::ffff:127.0.0.1]:12000",
            "127.0.0.1:0",
            "127.0.0.1:80",
            "127.0.0.1:1023",
            "127.0.0.1:65536",
            "127.0.0.1:01200",
            "[0:0:0:0:0:0:0:1]:12000",
            "http://127.0.0.1:12000",
            "127.0.0.1",
            CANARY,
        ] {
            let mut args = baseline.clone();
            args[10] = raw.into();
            cases.push(args);
        }
        let mut non_utf8 = baseline.clone();
        non_utf8[10] = OsString::from_vec(vec![0xff]);
        cases.push(non_utf8);
        let mut missing = baseline.clone();
        missing.pop();
        cases.push(missing);
        let mut duplicate = baseline.clone();
        duplicate.extend(["--bind".into(), CANARY.into()]);
        cases.push(duplicate);
        let mut extra = baseline.clone();
        extra.push(CANARY.into());
        cases.push(extra);
        let mut reordered = baseline.clone();
        reordered.swap(7, 9);
        cases.push(reordered);
        let mut relative = baseline.clone();
        relative[8] = "relative.json".into();
        cases.push(relative);
        let mut huge = baseline;
        huge[10] = "x".repeat(4097).into();
        cases.push(huge);
        let mut watch = Watch::new(&[&pipe]);
        let runs = cases
            .iter()
            .map(|args| support::run(fixture, args, None))
            .collect::<Vec<_>>();
        let no_open = !watch.activity();
        let control = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&pipe)
            .unwrap();
        let positive = watch.activity();
        drop(control);
        let bad = fixture.write("bad-config.json", b"{", 0o600);
        let boundaries = [
            "127.0.0.1:1024",
            "127.0.0.1:65535",
            "[::1]:1024",
            "[::1]:65535",
        ]
        .iter()
        .map(|raw| {
            let mut args = support::arguments(fixture, &bad, bind);
            args[10] = raw.into();
            support::run(fixture, &args, None)
        })
        .collect::<Vec<_>>();
        (runs, boundaries, no_open, positive)
    });
    for run in &runs {
        support::rejected(run, 64);
    }
    for run in &boundaries {
        support::rejected(run, 1);
    }
    assert!(
        no_open && positive,
        "usage errors preceded an independently detected sensitive open"
    );
}

#[test]
fn platform_serve_cli_wrong_identity_precedes_private_fifo_or_database_access() {
    let (runs, no_open, positive, no_dial, no_host) = Fixture::checked(|fixture| {
        let db = support::listener();
        let bind = support::address();
        let pipe = fixture.path("identity-gate.json");
        fifo(&pipe);
        let baseline = support::arguments(fixture, &pipe, bind);
        let mut watch = Watch::new(&[&pipe]);
        let mut cases = Vec::new();
        for (index, current) in [(2, fixture.identity.uid[0]), (4, fixture.identity.gid[0])] {
            let mut args = baseline.clone();
            args[index] = if current == 1 { "2" } else { "1" }.into();
            cases.push(args);
        }
        let runs = cases
            .iter()
            .map(|args| support::run(fixture, args, None))
            .collect::<Vec<_>>();
        let no_open = !watch.activity();
        let control = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&pipe)
            .unwrap();
        let positive = watch.activity();
        drop(control);
        (
            runs,
            no_open,
            positive,
            support::no_dial(&db),
            support::no_host(bind),
        )
    });
    for run in &runs {
        support::rejected(run, 1);
    }
    assert!(no_open && positive && no_dial && no_host);
}

#[test]
fn platform_serve_cli_requires_format_three_and_exact_http_origin_before_root_or_dial() {
    let (runs, no_root_open, positive, no_dial, no_host, valid_config, safe) =
        Fixture::checked(|fixture| {
            let db = support::listener();
            let bind = support::address();
            let root = support::root(fixture, "artifacts");
            let original = support::document(&root, db.local_addr().unwrap().port(), bind);
            // Old check-config still accepts the exact format3 declaration without observing storage.
            let valid = support::input(fixture, "valid.json", &original);
            let valid_config = fixture.run(&fixture.arguments(&valid), false);
            let mut values = Vec::new();
            let mut v1 = original.clone();
            v1["format_version"] = json!(1);
            v1.as_object_mut().unwrap().remove("catalog_identity");
            v1.as_object_mut().unwrap().remove("marker_identity");
            values.push(v1);
            let mut v2 = original.clone();
            v2["format_version"] = json!(2);
            v2.as_object_mut().unwrap().remove("marker_identity");
            values.push(v2);
            for origin in [
                format!("https://{bind}"),
                format!("http://localhost:{}", bind.port()),
                format!(
                    "http://127.0.0.1:{}",
                    if bind.port() == 65535 {
                        65534
                    } else {
                        bind.port() + 1
                    }
                ),
                format!("http://{bind}/"),
                "http://192.0.2.1:12000".to_owned(),
            ] {
                let mut value = original.clone();
                value["origin"] = json!(origin);
                values.push(value);
            }
            let mut watch = Watch::new(&[&root]);
            let runs = values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let path = support::input(fixture, &format!("case-{index}.json"), value);
                    support::run(fixture, &support::arguments(fixture, &path, bind), None)
                })
                .collect::<Vec<_>>();
            let no_root_open = !watch.activity();
            let control = File::open(&root).unwrap();
            let positive = watch.activity();
            drop(control);
            (
                runs,
                no_root_open,
                positive,
                support::no_dial(&db),
                support::no_host(bind),
                valid_config,
                fixture.identity.safe_ids(),
            )
        });
    for run in &runs {
        support::rejected(run, 1);
    }
    assert!(no_root_open && positive && no_dial && no_host);
    assert!(!valid_config.timed_out);
    if safe {
        assert_eq!(valid_config.output.status.code(), Some(0));
    } else {
        support::rejected(&valid_config, 1);
    }
}
