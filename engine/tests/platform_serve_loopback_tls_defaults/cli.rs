use super::support::*;
use std::{ffi::OsString, fs::OpenOptions, os::unix::fs::OpenOptionsExt};
#[test]
fn platform_tls_cli_grammar_and_identity_precede_all_private_fifo_access() {
    let (grammar, identity, no_open, positive, no_db, no_ready) = Fixture::checked(|f| {
        let db = listener();
        let bind = address();
        let config = f.path("config-fifo");
        let cert = f.path("cert-fifo");
        let key = f.path("key-fifo");
        for path in [&config, &cert, &key] {
            fifo(path);
        }
        let base = arguments(f, &config, bind, &cert, &key);
        let mut cases = vec![vec![OsString::from("serve-loopback-tls")]];
        for raw in [
            "127.0.0.1:0",
            "127.0.0.1:80",
            "127.0.0.1:01200",
            "127.0.0.2:12000",
            "localhost:12000",
            "[::]:12000",
            "[::ffff:127.0.0.1]:12000",
            "[0:0:0:0:0:0:0:1]:12000",
            "0.0.0.0:12000",
            CANARY,
        ] {
            let mut args = base.clone();
            args[10] = raw.into();
            cases.push(args);
        }
        for index in [8, 12, 14] {
            let mut args = base.clone();
            args[index] = "relative-private-input".into();
            cases.push(args);
        }
        let mut reordered = base.clone();
        reordered.swap(11, 13);
        cases.push(reordered);
        let mut extra = base.clone();
        extra.push(CANARY.into());
        cases.push(extra);
        let mut missing = base.clone();
        missing.pop();
        cases.push(missing);
        let mut duplicate = base.clone();
        duplicate.extend(["--key".into(), key.as_os_str().to_owned()]);
        cases.push(duplicate);
        let mut watch = Watch::new(&[&config, &cert, &key]);
        let grammar = cases.iter().map(|args| run(f, args)).collect::<Vec<_>>();
        let identity = [(2, f.identity.uid[0]), (4, f.identity.gid[0])]
            .into_iter()
            .map(|(index, current)| {
                let mut args = base.clone();
                args[index] = if current == 1 { "2" } else { "1" }.into();
                run(f, &args)
            })
            .collect::<Vec<_>>();
        let no_open = !watch.activity();
        let control = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&key)
            .unwrap();
        let positive = watch.activity();
        drop(control);
        (
            grammar,
            identity,
            no_open,
            positive,
            no_dial(&db),
            no_host(bind),
        )
    });
    for value in &grammar {
        rejected(value, 64);
    }
    for value in &identity {
        rejected(value, 1);
    }
    assert!(no_open && positive && no_db && no_ready);
}
#[test]
fn platform_tls_cli_format_and_origin_reject_before_cert_key_and_root_open() {
    let (runs, no_open, positive, no_db, no_ready) = Fixture::checked(|f| {
        let db = listener();
        let bind = address();
        let root = root(f, "artifact-root");
        let cert = f.path("cert-fifo");
        let key = f.path("key-fifo");
        fifo(&cert);
        fifo(&key);
        let original = document(&root, db.local_addr().unwrap().port(), bind);
        let mut variants = Vec::new();
        for version in [1, 2] {
            let mut v = original.clone();
            v["format_version"] = version.into();
            v.as_object_mut().unwrap().remove("marker_identity");
            if version == 1 {
                v.as_object_mut().unwrap().remove("catalog_identity");
            }
            variants.push(v);
        }
        for origin in [
            format!("http://{bind}"),
            format!("https://localhost:{}", bind.port()),
            format!(
                "https://127.0.0.1:{}",
                if bind.port() == 65535 {
                    65534
                } else {
                    bind.port() + 1
                }
            ),
            format!("https://{bind}/"),
        ] {
            let mut v = original.clone();
            v["origin"] = origin.into();
            variants.push(v);
        }
        let mut watch = Watch::new(&[&cert, &key, &root]);
        let runs = variants
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let path = input(f, &format!("origin-{i}.json"), v);
                run(f, &arguments(f, &path, bind, &cert, &key))
            })
            .collect::<Vec<_>>();
        let no_open = !watch.activity();
        let control = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&cert)
            .unwrap();
        let positive = watch.activity();
        drop(control);
        (runs, no_open, positive, no_dial(&db), no_host(bind))
    });
    for value in &runs {
        rejected(value, 1);
    }
    assert!(no_open && positive && no_db && no_ready);
}
