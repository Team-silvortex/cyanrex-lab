use super::{status::Status, *};

fn record() -> String {
    "Name:\tplatform-test\nTgid:\t71\nPid:\t71\nThreads:\t1\n\
        Uid:\t1000 1000 1000 1000\nGid:\t1001 1001 1001 1001\nGroups:\t2 1001\n\
        CapInh:\t0000000000000000\nCapPrm:\t0000000000000000\n\
        CapEff:\t0000000000000000\nCapAmb:\t0000000000000000\n\
        CapBnd:\tffffffffffffffff\nNoNewPrivs:\t0\n"
        .to_owned()
}

fn policy() -> PlatformProcessIdentity {
    PlatformProcessIdentity::new(1000, 1001, &[2, 1001]).unwrap()
}

#[test]
fn startup_policy_allows_retained_bounding_set_but_requires_fresh_nnp_for_threads() {
    let mut status = Status::parse(&record()).unwrap();
    assert!(status.validate(&policy(), 71, 71, true, false).is_ok());
    assert!(status.validate(&policy(), 71, 71, true, true).is_err());
    status.nnp = 1;
    assert!(status.validate(&policy(), 71, 71, true, true).is_ok());
    status.pid = 72;
    status.threads = 2;
    assert!(status.validate(&policy(), 71, 72, false, true).is_ok());
    assert!(status.validate(&policy(), 71, 72, true, true).is_err());
}

#[test]
fn startup_policy_rejects_every_root_mismatched_saved_and_filesystem_identity() {
    let original = Status::parse(&record()).unwrap();
    for index in 0..4 {
        for replacement in [0, 19, u32::MAX] {
            let mut uid = original.clone();
            uid.uid[index] = replacement;
            assert!(uid.validate(&policy(), 71, 71, true, false).is_err());
            let mut gid = original.clone();
            gid.gid[index] = replacement;
            assert!(gid.validate(&policy(), 71, 71, true, false).is_err());
        }
    }
}

#[test]
fn startup_policy_rejects_every_nonzero_current_capability_and_changed_group_set() {
    let original = Status::parse(&record()).unwrap();
    for index in 0..4 {
        for bit in [1, 1 << 35, 1 << 63] {
            let mut status = original.clone();
            status.capabilities[index] = bit;
            assert!(status.validate(&policy(), 71, 71, true, false).is_err());
        }
    }
    for groups in [
        vec![],
        vec![0, 2, 1001],
        vec![2],
        vec![2, 3, 1001],
        vec![1001, 2],
        vec![2, 2, 1001],
    ] {
        let mut status = original.clone();
        status.groups = groups;
        assert!(status.validate(&policy(), 71, 71, true, false).is_err());
    }
}

#[test]
fn startup_policy_rejects_wrong_native_pid_tid_multithreading_and_invalid_nnp() {
    let original = Status::parse(&record()).unwrap();
    assert!(original.validate(&policy(), 70, 71, true, false).is_err());
    assert!(original.validate(&policy(), 71, 70, true, false).is_err());
    let mut status = original.clone();
    status.threads = 2;
    assert!(status.validate(&policy(), 71, 71, true, false).is_err());
    status.threads = 0;
    assert!(status.validate(&policy(), 71, 71, false, false).is_err());
    status = original;
    status.nnp = 2;
    assert!(status.validate(&policy(), 71, 71, true, false).is_err());
    for raw in [-1, 2, i32::MAX] {
        assert!(status::nnp_value(raw).is_err());
    }
    assert_eq!(status::nnp_value(0).unwrap(), 0);
    assert_eq!(status::nnp_value(1).unwrap(), 1);
}

#[test]
fn startup_status_parser_rejects_missing_duplicate_corrupt_and_over_budget_fields() {
    for field in [
        "Uid",
        "Gid",
        "Groups",
        "CapInh",
        "CapPrm",
        "CapEff",
        "CapAmb",
        "CapBnd",
        "NoNewPrivs",
        "Pid",
        "Tgid",
        "Threads",
    ] {
        let input = record();
        let line = input
            .lines()
            .find(|line| line.starts_with(&format!("{field}:")))
            .unwrap();
        assert!(
            Status::parse(&input.replace(&format!("{line}\n"), "")).is_err(),
            "missing {field}"
        );
        assert!(
            Status::parse(&format!("{input}{line}\n")).is_err(),
            "duplicate {field}"
        );
        assert!(
            Status::parse(&input.replace(line, &format!("{field}: synthetic-invalid"))).is_err(),
            "corrupt {field}"
        );
    }
    for value in [
        "-1 1000 1000 1000",
        "1000 1000 1000",
        "1000 1000 1000 1000 1000",
        "4294967296 1000 1000 1000",
    ] {
        assert!(Status::parse(&record().replace("1000 1000 1000 1000", value)).is_err());
    }
    assert!(Status::parse(&record().replace("ffffffffffffffff", "1ffffffffffffffff")).is_err());
    assert!(Status::parse(&format!("{}{}", record(), "x".repeat(16 * 1024))).is_err());
    let groups = (1..=129)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        Status::parse(&record().replace("Groups:\t2 1001", &format!("Groups:\t{groups}"))).is_err()
    );
}
