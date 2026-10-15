use super::fixture::{fifo, rejected, Fixture, Watch, CANARY};
use std::{ffi::OsString, fs::OpenOptions, os::unix::fs::OpenOptionsExt};

#[test]
fn platform_config_cli_process_gate_precedes_fifo_open_and_usage_is_strict() {
    let (wrong_uid, no_open, positive, syntax, generic_reference) = Fixture::checked(|fixture| {
        let pipe = fixture.path("identity-gate-fifo.json");
        fifo(&pipe);
        let mut watch = Watch::new(&[&pipe]);
        let baseline = fixture.arguments(&pipe);
        let mut wrong = baseline.clone();
        wrong[2] = if fixture.identity.uid[0] == 1 {
            "2"
        } else {
            "1"
        }
        .into();
        let wrong_uid = fixture.run(&wrong, false);
        let no_open = !watch.activity();
        let control = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&pipe)
            .unwrap();
        let positive = watch.activity();
        drop(control);
        let bad = fixture.write("bad-json.json", b"{", 0o600);
        let generic_reference = fixture.run(&fixture.arguments(&bad), false);
        let mut cases = vec![Vec::<OsString>::new(), vec!["check-config".into()]];
        let mut missing = baseline.clone();
        missing.pop();
        cases.push(missing);
        let mut reordered = baseline.clone();
        reordered.swap(1, 3);
        cases.push(reordered);
        let mut duplicate = baseline.clone();
        duplicate.extend(["--config".into(), CANARY.into()]);
        cases.push(duplicate);
        let mut extra = baseline.clone();
        extra.push(CANARY.into());
        cases.push(extra);
        let mut relative = baseline.clone();
        relative[8] = "relative-input.json".into();
        cases.push(relative);
        for index in [2, 4, 6] {
            let mut invalid = baseline.clone();
            invalid[index] = CANARY.into();
            cases.push(invalid);
        }
        let syntax = cases
            .iter()
            .map(|arguments| fixture.run(arguments, false))
            .collect::<Vec<_>>();
        (wrong_uid, no_open, positive, syntax, generic_reference)
    });
    rejected(&wrong_uid, 1);
    rejected(&generic_reference, 1);
    assert_eq!(wrong_uid.output.stderr, generic_reference.output.stderr);
    assert!(
        no_open,
        "identity rejection must precede even opening the input FIFO"
    );
    assert!(
        positive,
        "the FIFO open oracle must independently detect a real open"
    );
    let usage = &syntax[0].output.stderr;
    for output in &syntax {
        rejected(output, 64);
        assert_eq!(&output.output.stderr, usage);
    }
}
