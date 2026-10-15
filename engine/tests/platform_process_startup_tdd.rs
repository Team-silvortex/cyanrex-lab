#![cfg(target_os = "linux")]

use std::{
    collections::BTreeMap,
    fs,
    os::unix::process::CommandExt,
    process::{Command, Output},
    sync::Barrier,
};

use cyanrex_engine::platform_host::{prepare_platform_process, PlatformProcessIdentity};

const SUCCESS: &[u8] = b"platform process check passed; no host started\n";
const CANARY: &str = "synthetic-process-argument-do-not-echo-7e9b";

#[derive(PartialEq, Eq)]
struct KernelIdentity {
    uid: [u32; 4],
    gid: [u32; 4],
    groups: Vec<u32>,
    capabilities: [u64; 4],
    capability_bounding: u64,
}

impl KernelIdentity {
    fn current() -> Self {
        let status = fs::read_to_string("/proc/thread-self/status").unwrap();
        let field = |name: &str| {
            status
                .lines()
                .find_map(|line| line.strip_prefix(name))
                .unwrap()
                .trim()
                .to_owned()
        };
        let ids = |name: &str| -> [u32; 4] {
            field(name)
                .split_whitespace()
                .map(|value| value.parse().unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        };
        let mut groups = field("Groups:")
            .split_whitespace()
            .map(|value| value.parse().unwrap())
            .collect::<Vec<u32>>();
        groups.sort_unstable();
        Self {
            uid: ids("Uid:"),
            gid: ids("Gid:"),
            groups,
            capabilities: ["CapPrm:", "CapEff:", "CapInh:", "CapAmb:"]
                .map(|name| u64::from_str_radix(&field(name), 16).unwrap()),
            capability_bounding: u64::from_str_radix(&field("CapBnd:"), 16).unwrap(),
        }
    }

    fn safe_ids(&self) -> bool {
        self.uid[0] != 0
            && self.uid[0] != u32::MAX
            && self.uid.iter().all(|uid| *uid == self.uid[0])
            && self.gid[0] != 0
            && self.gid[0] != u32::MAX
            && self.gid.iter().all(|gid| *gid == self.gid[0])
            && self.groups.len() <= 128
            && self.groups.iter().all(|gid| *gid != 0 && *gid != u32::MAX)
            && self.groups.windows(2).all(|pair| pair[0] < pair[1])
    }

    fn safe(&self) -> bool {
        self.safe_ids() && self.capabilities == [0; 4]
    }

    fn arguments(&self) -> Vec<String> {
        let valid_groups = self.groups.len() <= 128
            && self.groups.iter().all(|gid| *gid != 0 && *gid != u32::MAX)
            && self.groups.windows(2).all(|pair| pair[0] < pair[1]);
        arguments(
            self.uid[0].max(1),
            self.gid[0].max(1),
            if valid_groups { &self.groups } else { &[] },
        )
    }
}

fn arguments(uid: u32, gid: u32, groups: &[u32]) -> Vec<String> {
    vec![
        "check-process".to_owned(),
        "--uid".to_owned(),
        uid.to_string(),
        "--gid".to_owned(),
        gid.to_string(),
        "--groups".to_owned(),
        if groups.is_empty() {
            "none".to_owned()
        } else {
            groups
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",")
        },
    ]
}

fn command(arguments: &[String]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cyanrex-platform"));
    command.args(arguments);
    command
}

fn capability_free_child(arguments: &[String]) -> Command {
    #[repr(C)]
    struct CapabilityHeader {
        version: u32,
        pid: i32,
    }
    #[repr(C)]
    struct CapabilityData {
        effective: u32,
        permitted: u32,
        inheritable: u32,
    }
    let mut child = command(arguments);
    // SAFETY: only direct syscalls and stack-owned values are used after fork.
    // The fixture reduces only the child's P/E/I/A capabilities; it does not
    // acquire authority or change IDs, groups, the parent, or production code.
    unsafe {
        child.pre_exec(|| {
            let header = CapabilityHeader {
                version: 0x2008_0522,
                pid: 0,
            };
            let data = [
                CapabilityData {
                    effective: 0,
                    permitted: 0,
                    inheritable: 0,
                },
                CapabilityData {
                    effective: 0,
                    permitted: 0,
                    inheritable: 0,
                },
            ];
            if libc::syscall(
                libc::SYS_capset,
                &header as *const CapabilityHeader,
                data.as_ptr(),
            ) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            if libc::prctl(
                libc::PR_CAP_AMBIENT,
                libc::PR_CAP_AMBIENT_CLEAR_ALL as libc::c_ulong,
                0 as libc::c_ulong,
                0 as libc::c_ulong,
                0 as libc::c_ulong,
            ) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    child
}

fn no_new_privileges() -> i32 {
    // SAFETY: PR_GET_NO_NEW_PRIVS only reads the calling thread's kernel state.
    let value = unsafe {
        libc::prctl(
            libc::PR_GET_NO_NEW_PRIVS,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
        )
    };
    assert!(matches!(value, 0 | 1));
    value
}

fn rejected(output: &Output, code: i32) {
    assert_eq!(output.status.code(), Some(code));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
    assert!(output.stderr.len() <= 256);
    assert!(!String::from_utf8_lossy(&output.stderr).contains(CANARY));
}

fn passed(output: &Output) {
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, SUCCESS);
    assert!(output.stderr.is_empty());
}

fn assert_current_outcome(output: &Output, identity: &KernelIdentity) {
    if identity.safe() {
        passed(output);
    } else if !identity.safe_ids() || identity.capabilities[2] != 0 || identity.capabilities[3] != 0
    {
        rejected(output, 1);
    } else if output.status.code() == Some(0) {
        // Ordinary exec may drop the parent's permitted/effective capabilities.
        // The dedicated capability-free fixture provides mandatory positive proof.
        passed(output);
    } else {
        rejected(output, 1);
    }
}

fn assert_capability_free_outcome(output: &Output, identity: &KernelIdentity) {
    if identity.safe_ids() {
        passed(output);
    } else {
        rejected(output, 1);
    }
}

#[test]
fn platform_process_identity_accepts_only_explicit_nonzero_sorted_bounded_groups() {
    assert!(PlatformProcessIdentity::new(1, 2, &[]).is_ok());
    assert!(PlatformProcessIdentity::new(1000, 1001, &[3, 11, 30]).is_ok());
    let maximum = (1..=128).collect::<Vec<_>>();
    assert!(PlatformProcessIdentity::new(1, 2, &maximum).is_ok());
    for (uid, gid, groups) in [
        (0, 2, vec![]),
        (1, 0, vec![]),
        (u32::MAX, 2, vec![]),
        (1, u32::MAX, vec![]),
        (1, 2, vec![0]),
        (1, 2, vec![u32::MAX]),
        (1, 2, vec![3, 3]),
        (1, 2, vec![11, 3]),
        (1, 2, (1..=129).collect()),
    ] {
        assert!(PlatformProcessIdentity::new(uid, gid, &groups).is_err());
    }
}

#[test]
fn platform_process_cli_checks_real_kernel_identity_without_starting_a_host() {
    let identity = KernelIdentity::current();
    let parent_nnp = no_new_privileges();
    let parent_environment = std::env::vars_os().collect::<BTreeMap<_, _>>();
    let output = command(&identity.arguments()).output().unwrap();
    assert_current_outcome(&output, &identity);
    assert!(KernelIdentity::current() == identity);
    assert_eq!(no_new_privileges(), parent_nnp);
    assert!(std::env::vars_os().collect::<BTreeMap<_, _>>() == parent_environment);
}

#[test]
fn platform_process_cli_accepts_real_capability_free_child_and_preserves_parent_state() {
    let identity = KernelIdentity::current();
    let parent_nnp = no_new_privileges();
    let parent_environment = std::env::vars_os().collect::<BTreeMap<_, _>>();
    let output = capability_free_child(&identity.arguments())
        .output()
        .unwrap();
    assert_capability_free_outcome(&output, &identity);
    assert!(KernelIdentity::current() == identity);
    assert_eq!(no_new_privileges(), parent_nnp);
    assert!(std::env::vars_os().collect::<BTreeMap<_, _>>() == parent_environment);
}

#[test]
fn platform_process_cli_rejects_wrong_uid_gid_and_supplementary_groups() {
    let identity = KernelIdentity::current();
    let parent_nnp = no_new_privileges();
    let mut baseline = identity.arguments();
    let mut failures = Vec::new();
    for (index, current) in [(2, identity.uid[0]), (4, identity.gid[0])] {
        let mut wrong = baseline.clone();
        wrong[index] = if current == 1 { "2" } else { "1" }.to_owned();
        failures.push(wrong);
    }
    let absent_group = (1..=u32::MAX)
        .find(|gid| !identity.groups.contains(gid))
        .unwrap();
    baseline[6] = absent_group.to_string();
    failures.push(baseline);
    let mut generic_error = None;
    for arguments in failures {
        let output = capability_free_child(&arguments).output().unwrap();
        rejected(&output, 1);
        if let Some(expected) = &generic_error {
            assert_eq!(&output.stderr, expected);
        } else {
            generic_error = Some(output.stderr);
        }
        assert_eq!(no_new_privileges(), parent_nnp);
        assert!(KernelIdentity::current() == identity);
    }
}

#[test]
fn platform_process_cli_rejects_malformed_incomplete_or_reordered_arguments_without_echo() {
    let baseline = arguments(1000, 1000, &[1000, 1001]);
    let parent_nnp = no_new_privileges();
    let mut failures = vec![vec![], vec!["check-process".to_owned()]];
    for index in [2, 4, 6] {
        let mut missing = baseline.clone();
        missing.remove(index);
        failures.push(missing);
        for value in [
            CANARY,
            "",
            "-1",
            "+1",
            " 1",
            "1 ",
            "4294967296",
            "4294967295",
            "0",
        ] {
            let mut malformed = baseline.clone();
            malformed[index] = value.to_owned();
            failures.push(malformed);
        }
    }
    for groups in [",1", "1,", "1,,2", "2,1", "1,1", "0,1", "none,1", "1,none"] {
        let mut malformed = baseline.clone();
        malformed[6] = groups.to_owned();
        failures.push(malformed);
    }
    let mut too_many = baseline.clone();
    too_many[6] = (1..=129)
        .map(|gid| gid.to_string())
        .collect::<Vec<_>>()
        .join(",");
    failures.push(too_many);
    let mut reordered = baseline.clone();
    reordered.swap(1, 3);
    failures.push(reordered);
    let mut duplicate = baseline.clone();
    duplicate.extend(["--uid".to_owned(), CANARY.to_owned()]);
    failures.push(duplicate);
    let mut extra = baseline.clone();
    extra.push(CANARY.to_owned());
    failures.push(extra);
    let mut unknown = baseline.clone();
    unknown[0] = CANARY.to_owned();
    failures.push(unknown);
    let mut generic_usage = None;
    for arguments in failures {
        let output = command(&arguments).output().unwrap();
        rejected(&output, 64);
        if let Some(expected) = &generic_usage {
            assert_eq!(&output.stderr, expected);
        } else {
            generic_usage = Some(output.stderr);
        }
        assert_eq!(no_new_privileges(), parent_nnp);
    }
}

#[test]
fn platform_process_cli_accepts_already_restricted_children_idempotently() {
    let identity = KernelIdentity::current();
    let parent_nnp = no_new_privileges();
    let mut child = capability_free_child(&identity.arguments());
    // SAFETY: the forked child invokes only the async-signal-safe prctl syscall
    // before exec. It changes only its own irreversible no-new-privileges flag.
    unsafe {
        child.pre_exec(|| {
            if libc::prctl(
                libc::PR_SET_NO_NEW_PRIVS,
                1 as libc::c_ulong,
                0 as libc::c_ulong,
                0 as libc::c_ulong,
                0 as libc::c_ulong,
            ) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    assert_capability_free_outcome(&child.output().unwrap(), &identity);
    assert!(KernelIdentity::current() == identity);
    assert_eq!(no_new_privileges(), parent_nnp);
}

#[test]
fn platform_process_library_nonmain_multithreaded_harness_rejection_preserves_nnp() {
    // This harness is both non-main and multithreaded; rejection can also come
    // from inherited capabilities. Pure status tests isolate the Threads predicate.
    let identity = KernelIdentity::current();
    let arguments = identity.arguments();
    let uid = arguments[2].parse().unwrap();
    let gid = arguments[4].parse().unwrap();
    let groups = if arguments[6] == "none" {
        vec![]
    } else {
        arguments[6]
            .split(',')
            .map(|group| group.parse().unwrap())
            .collect::<Vec<_>>()
    };
    let expected = PlatformProcessIdentity::new(uid, gid, &groups).unwrap();
    let parent_nnp = no_new_privileges();
    let entered = Barrier::new(2);
    let release = Barrier::new(2);
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            entered.wait();
            release.wait();
        });
        entered.wait();
        let threads = fs::read_dir("/proc/self/task").map(|tasks| tasks.count());
        let rejected = prepare_platform_process(&expected).is_err();
        let after = no_new_privileges();
        release.wait();
        worker.join().unwrap();
        assert!(threads.unwrap() >= 2);
        assert!(rejected);
        assert_eq!(after, parent_nnp);
    });
    assert_eq!(no_new_privileges(), parent_nnp);
}
