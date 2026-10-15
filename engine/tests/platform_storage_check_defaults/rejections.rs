use super::{
    fixture::{fifo, Fixture, Watch, CANARY},
    peer::{listener, no_second_connection},
    support,
};
use std::{
    fs::{self, File, OpenOptions},
    os::unix::fs::{symlink, MetadataExt, OpenOptionsExt, PermissionsExt},
};

#[test]
fn platform_storage_cli_rejects_any_pgoptions_before_dial_and_preserves_usage_boundary() {
    let (outputs, no_connection, syntax) = Fixture::checked(|fixture| {
        let listener = listener();
        let root = support::root(fixture, "artifacts");
        let input = support::input(
            fixture,
            "input.json",
            &root,
            listener.local_addr().unwrap().port(),
        );
        let arguments = support::arguments(fixture, &input);
        let outputs = ["", "-c role=synthetic_untrusted_role"]
            .map(|value| support::run(fixture, &arguments, Some(value)));
        let no_connection = no_second_connection(&listener);
        drop(listener);
        let mut missing = arguments.clone();
        missing.pop();
        let mut extra = arguments.clone();
        extra.push(CANARY.into());
        let mut relative = arguments.clone();
        relative[8] = "relative.json".into();
        let syntax =
            [missing, extra, relative].map(|arguments| support::run(fixture, &arguments, None));
        (outputs, no_connection, syntax)
    });
    for output in &outputs {
        support::failed(output);
    }
    assert!(
        no_connection,
        "PGOPTIONS rejection must precede opening a socket"
    );
    for output in &syntax {
        assert!(!output.timed_out);
        assert_eq!(output.output.status.code(), Some(64));
        assert!(output.output.stdout.is_empty());
        assert!(!output.output.stderr.is_empty() && output.output.stderr.len() <= 512);
        assert!(!String::from_utf8_lossy(&output.output.stderr).contains(CANARY));
    }
}

#[test]
fn platform_storage_cli_rejects_unsafe_roots_before_dial_or_repair_and_uid_before_fifo_open() {
    let (
        outputs,
        no_connection,
        unchanged,
        no_target_open,
        watch_positive,
        wrong_uid,
        no_fifo_open,
        fifo_positive,
    ) = Fixture::checked(|fixture| {
        let listener = listener();
        let port = listener.local_addr().unwrap().port();
        let target = support::root(fixture, "target-artifacts");
        let before = fs::metadata(&target).unwrap();
        let link = fixture.path("symlink-artifacts");
        symlink(&target, &link).unwrap();
        let unsafe_root = support::root(fixture, "unsafe-artifacts");
        fs::set_permissions(&unsafe_root, fs::Permissions::from_mode(0o750)).unwrap();
        let missing = fixture.path("missing-artifacts");
        let mut target_watch = Watch::new(&[&target]);
        let mut outputs = Vec::new();
        for (index, path) in [&missing, &link, &unsafe_root].into_iter().enumerate() {
            let input = support::input(fixture, &format!("root-{index}.json"), path, port);
            outputs.push(support::run(
                fixture,
                &support::arguments(fixture, &input),
                None,
            ));
        }
        let no_target_open = !target_watch.activity();
        let positive = File::open(&target).unwrap();
        let watch_positive = target_watch.activity();
        drop(positive);
        drop(target_watch);
        let after = fs::metadata(&target).unwrap();
        let unchanged = !missing.exists()
            && fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
            && fs::metadata(&unsafe_root).unwrap().mode() & 0o7777 == 0o750
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
        let no_connection = no_second_connection(&listener);
        drop(listener);

        let pipe = fixture.path("process-gate-fifo.json");
        fifo(&pipe);
        let mut fifo_watch = Watch::new(&[&pipe]);
        let mut arguments = support::arguments(fixture, &pipe);
        arguments[2] = if fixture.identity.uid[0] == 1 {
            "2"
        } else {
            "1"
        }
        .into();
        let wrong_uid = support::run(fixture, &arguments, None);
        let no_fifo_open = !fifo_watch.activity();
        let positive = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&pipe)
            .unwrap();
        let fifo_positive = fifo_watch.activity();
        drop(positive);
        drop(fifo_watch);
        (
            outputs,
            no_connection,
            unchanged,
            no_target_open,
            watch_positive,
            wrong_uid,
            no_fifo_open,
            fifo_positive,
        )
    });
    for output in &outputs {
        support::failed(output);
    }
    support::failed(&wrong_uid);
    assert!(no_connection && unchanged && no_target_open && watch_positive);
    assert!(
        no_fifo_open && fifo_positive,
        "process identity rejection must precede an independently observed FIFO open"
    );
}
