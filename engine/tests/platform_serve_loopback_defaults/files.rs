use super::{
    fixture::{fifo, Fixture, Watch},
    support,
};
use serde_json::json;
use std::{
    fs::{self, File},
    os::unix::fs::{symlink, MetadataExt, PermissionsExt},
};

#[test]
fn platform_serve_cli_rejects_unsafe_private_input_without_dial_or_ready() {
    let (runs, no_dial, no_host) = Fixture::checked(|fixture| {
        let db = support::listener();
        let bind = support::address();
        let root = support::root(fixture, "artifacts");
        let bytes = serde_json::to_vec(&support::document(
            &root,
            db.local_addr().unwrap().port(),
            bind,
        ))
        .unwrap();
        let original = fixture.write("private.json", &bytes, 0o600);
        let link = fixture.path("symlink.json");
        symlink(&original, &link).unwrap();
        let hard = fixture.path("hardlink.json");
        fs::hard_link(&original, &hard).unwrap();
        let broad = fixture.write("readable.json", &bytes, 0o644);
        let malformed = fixture.write("invalid.json", b"{", 0o600);
        let pipe = fixture.path("fifo.json");
        fifo(&pipe);
        let duplicate = fixture.write(
            "duplicate.json",
            &bytes
                .iter()
                .copied()
                .take(bytes.len() - 1)
                .chain(b",\"format_version\":3}".iter().copied())
                .collect::<Vec<_>>(),
            0o600,
        );
        let runs = [link, hard, original, broad, malformed, pipe, duplicate]
            .iter()
            .map(|path| support::run(fixture, &support::arguments(fixture, path, bind), None))
            .collect::<Vec<_>>();
        (runs, support::no_dial(&db), support::no_host(bind))
    });
    for run in &runs {
        support::rejected(run, 1);
    }
    assert!(no_dial && no_host);
}

#[test]
fn platform_serve_cli_rejects_unsafe_artifact_roots_before_database_without_repairs() {
    let (runs, unchanged, no_target_open, positive, no_dial, no_host) =
        Fixture::checked(|fixture| {
            let db = support::listener();
            let bind = support::address();
            let port = db.local_addr().unwrap().port();
            let target = support::root(fixture, "target");
            let before = fs::metadata(&target).unwrap();
            let link = fixture.path("linked-root");
            symlink(&target, &link).unwrap();
            let broad = support::root(fixture, "broad-root");
            fs::set_permissions(&broad, fs::Permissions::from_mode(0o750)).unwrap();
            let absent = fixture.path("missing-root");
            let file = fixture.write("not-directory", b"synthetic", 0o600);
            let mut watch = Watch::new(&[&target]);
            let runs = [&link, &broad, &absent, &file]
                .iter()
                .enumerate()
                .map(|(index, root)| {
                    let input = support::input(
                        fixture,
                        &format!("root-{index}.json"),
                        &support::document(root, port, bind),
                    );
                    support::run(fixture, &support::arguments(fixture, &input, bind), None)
                })
                .collect::<Vec<_>>();
            let no_target_open = !watch.activity();
            let control = File::open(&target).unwrap();
            let positive = watch.activity();
            drop(control);
            let after = fs::metadata(&target).unwrap();
            let unchanged = !absent.exists()
                && fs::symlink_metadata(&link)
                    .unwrap()
                    .file_type()
                    .is_symlink()
                && fs::metadata(&broad).unwrap().mode() & 0o7777 == 0o750
                && (
                    before.dev(),
                    before.ino(),
                    before.uid(),
                    before.gid(),
                    before.mode(),
                ) == (
                    after.dev(),
                    after.ino(),
                    after.uid(),
                    after.gid(),
                    after.mode(),
                );
            (
                runs,
                unchanged,
                no_target_open,
                positive,
                support::no_dial(&db),
                support::no_host(bind),
            )
        });
    for run in &runs {
        support::rejected(run, 1);
    }
    assert!(unchanged && no_target_open && positive && no_dial && no_host);
}

#[test]
fn platform_serve_cli_pgoptions_and_invalid_password_fail_before_database_or_listening() {
    let (runs, no_dial, no_host) = Fixture::checked(|fixture| {
        let db = support::listener();
        let bind = support::address();
        let root = support::root(fixture, "artifacts");
        let value = support::document(&root, db.local_addr().unwrap().port(), bind);
        let input = support::input(fixture, "options.json", &value);
        let args = support::arguments(fixture, &input, bind);
        let mut runs = ["", "-c role=untrusted"]
            .iter()
            .map(|value| support::run(fixture, &args, Some(value)))
            .collect::<Vec<_>>();
        for (index, password) in ["x".repeat(4097), "synthetic\0password".to_owned()]
            .iter()
            .enumerate()
        {
            let mut value = value.clone();
            value["database"]["password"] = json!(password);
            let input = support::input(fixture, &format!("password-{index}.json"), &value);
            runs.push(support::run(
                fixture,
                &support::arguments(fixture, &input, bind),
                None,
            ));
        }
        (runs, support::no_dial(&db), support::no_host(bind))
    });
    for run in &runs {
        support::rejected(run, 1);
    }
    assert!(no_dial && no_host);
}
