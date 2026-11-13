#![cfg(target_os = "linux")]
//! Pure CLI parsing only: no process preparation, interface inspection or socket is performed.

#[path = "../src/bin/cyanrex-discovery/arguments.rs"]
mod arguments;

use arguments::{parse, Options, USAGE};
use cyanrex_engine::services::federation_discovery::{DiscoveryError, PeerAdvertisement};
use serde_json::json;
use std::{cell::Cell, ffi::OsString, net::Ipv4Addr, os::unix::ffi::OsStringExt, time::Duration};

fn valid() -> Vec<OsString> {
    [
        "browse-lan",
        "--uid",
        "1000",
        "--gid",
        "1001",
        "--groups",
        "2,1001",
        "--interface",
        "absent-fixture",
        "--address",
        "10.23.0.1",
        "--local-node",
        &"a".repeat(64),
        "--protocol",
        "1:3",
        "--timeout-ms",
        "123",
        "--capabilities",
        "cyanrex.read,cyanrex.write",
        "--require",
        "cyanrex.read",
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}

fn changed(index: usize, value: impl Into<OsString>) -> Vec<OsString> {
    let mut args = valid();
    args[index] = value.into();
    args
}

fn rejected(args: Vec<OsString>) {
    assert!(matches!(parse(args), Err(())));
}

fn advertisement(
    node: char,
    min: u16,
    max: u16,
    offered: &[&str],
    required: &[&str],
    hints: serde_json::Value,
) -> PeerAdvertisement {
    PeerAdvertisement::parse(
        &serde_json::to_vec(&json!({
            "schema_version": 1,
            "node_fingerprint": node.to_string().repeat(64),
            "authority_id": "11111111-1111-4111-8111-111111111111",
            "protocol": {"min": min, "max": max},
            "offered_capabilities": offered,
            "required_capabilities": required,
            "issued_at": 1000, "expires_at": 1100,
            "hints": hints,
        }))
        .unwrap(),
    )
    .unwrap()
}

fn direct() -> serde_json::Value {
    json!([{"kind": "direct", "address": "10.23.0.2:4433"}])
}

#[test]
fn exact_explicit_shape_constructs_values_without_inspecting_the_absent_interface() {
    let options: Options = parse(valid()).unwrap();
    let _: cyanrex_engine::platform_host::PlatformProcessIdentity = options.identity;
    assert_eq!(options.config.interface_name(), "absent-fixture");
    assert_eq!(options.config.local_address(), Ipv4Addr::new(10, 23, 0, 1));
    assert_eq!(options.config.timeout(), Duration::from_millis(123));
    assert_eq!(valid().len(), 21);
}

#[test]
fn missing_extra_help_alias_and_other_subcommands_do_not_select_discovery() {
    for length in 0..21 {
        rejected(valid().into_iter().take(length).collect());
    }
    let mut extra = valid();
    extra.push("--public".into());
    rejected(extra);
    for command in [
        "",
        "--help",
        "--version",
        "browse",
        "serve",
        "BROWSE-LAN",
        "browse-lan ",
    ] {
        rejected(changed(0, command));
    }
}

#[test]
fn every_flag_is_fixed_and_no_reordering_duplicate_or_old_host_flag_is_accepted() {
    for index in (1..21).step_by(2) {
        for replacement in [
            "--config",
            "--session-ui",
            "--uid",
            "--relay",
            "--public-direct",
        ] {
            if valid()[index] != replacement {
                rejected(changed(index, replacement));
            }
        }
    }
    let mut swapped = valid();
    swapped.swap(1, 3);
    swapped.swap(2, 4);
    rejected(swapped);
}

#[test]
fn the_argument_iterator_is_bounded_even_for_an_infinite_extra_tail() {
    let count = Cell::new(0);
    let args = valid()
        .into_iter()
        .chain(std::iter::repeat(OsString::from("extra")));
    let counted = args.inspect(|_| count.set(count.get() + 1));
    assert!(parse(counted).is_err());
    assert_eq!(count.get(), 22);
}

#[test]
fn non_utf8_at_every_position_is_rejected_without_lossy_normalization() {
    for index in 0..21 {
        rejected(changed(index, OsString::from_vec(vec![0xff, 0xfe])));
    }
}

#[test]
fn per_argument_and_aggregate_raw_byte_budgets_reject_before_semantic_admission() {
    for index in 0..21 {
        rejected(changed(index, "x".repeat(2049)));
    }
    let mut aggregate = valid();
    for item in aggregate.iter_mut().take(9) {
        *item = "x".repeat(2048).into();
    }
    rejected(aggregate);
}

fn capability(index: usize, length: usize) -> String {
    let prefix = format!("cap{index}.");
    format!("{prefix}{}", "a".repeat(length - prefix.len()))
}

#[test]
fn exactly_2048_bytes_is_accepted_when_all_capabilities_are_individually_valid() {
    let mut names = (0..15)
        .map(|index| capability(index, 128))
        .collect::<Vec<_>>();
    names.push(capability(15, 113));
    let capabilities = names.join(",");
    assert_eq!(capabilities.len(), 2048);
    let mut args = changed(18, capabilities);
    args[20] = "none".into();
    assert!(parse(args).is_ok());
    names[15] = capability(15, 114);
    let mut args = changed(18, names.join(","));
    args[20] = "none".into();
    rejected(args);
}

#[test]
fn identity_numbers_are_canonical_positive_u32_and_keep_the_existing_identity_limits() {
    for index in [2, 4] {
        for value in [
            "",
            "0",
            "01",
            "+1",
            "-1",
            " 1",
            "1 ",
            "1.0",
            "1e3",
            "4294967295",
            "4294967296",
            "00000000001",
        ] {
            rejected(changed(index, value));
        }
        assert!(parse(changed(index, "1")).is_ok());
        assert!(parse(changed(index, "4294967294")).is_ok());
    }
}

#[test]
fn supplementary_groups_require_explicit_none_or_strictly_sorted_unique_positive_ids() {
    for groups in ["none", "1", "1,2,4294967294"] {
        assert!(parse(changed(6, groups)).is_ok());
    }
    for groups in [
        "",
        "NONE",
        "none,1",
        "0",
        "01",
        "+1",
        "2,1",
        "1,1",
        "1,,2",
        ",1",
        "1,",
        "1, 2",
        "4294967295",
        "4294967296",
    ] {
        rejected(changed(6, groups));
    }
    let groups = (1..=128)
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(",");
    assert!(parse(changed(6, groups)).is_ok());
    let groups = (1..=129)
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(",");
    rejected(changed(6, groups));
}

#[test]
fn interface_names_have_only_the_existing_explicit_mdns_config_grammar() {
    for interface in ["eth0", "veth-lan", "eth0.10", "LAN_0", "abcdefghijklmno"] {
        assert!(parse(changed(8, interface)).is_ok());
    }
    for interface in [
        "",
        ".",
        "..",
        "eth0:1",
        "eth 0",
        "eth0/1",
        "eth0\n",
        "eth0\0",
        "网卡",
        "abcdefghijklmnop",
    ] {
        rejected(changed(8, interface));
    }
}

#[test]
fn the_address_is_canonical_rfc1918_ipv4_not_a_hostname_url_or_inferred_interface() {
    for address in ["10.23.0.1", "172.16.0.1", "172.31.255.254", "192.168.4.5"] {
        assert!(parse(changed(10, address)).is_ok());
    }
    for address in [
        "",
        "localhost",
        "http://10.23.0.1",
        "10.23.0.1:5353",
        "010.23.0.1",
        "10.23.0.1 ",
        "127.0.0.1",
        "0.0.0.0",
        "8.8.8.8",
        "172.15.0.1",
        "172.32.0.1",
        "169.254.169.254",
        "224.0.0.251",
        "255.255.255.255",
        "::1",
        "fd00::1",
        "::ffff:10.23.0.1",
    ] {
        rejected(changed(10, address));
    }
}

#[test]
fn local_node_is_a_supplied_canonical_claim_never_a_generated_or_verified_identity() {
    assert!(parse(changed(12, "0".repeat(64))).is_ok());
    for value in [
        "a".repeat(63),
        "a".repeat(65),
        "A".repeat(64),
        "g".repeat(64),
        format!("{}\n", "a".repeat(64)),
    ] {
        rejected(changed(12, value));
    }
    let options = parse(valid()).unwrap();
    let own = advertisement('a', 1, 3, &["cyanrex.read"], &[], direct());
    assert_eq!(
        own.select(&options.policy, 1010).err(),
        Some(DiscoveryError::SelfAdvertisement)
    );
}

#[test]
fn protocol_range_requires_exact_canonical_nonzero_u16_values_and_order() {
    for range in ["1:1", "1:65535", "65535:65535"] {
        assert!(parse(changed(14, range)).is_ok());
    }
    for range in [
        "",
        "1",
        "1,3",
        "1:2:3",
        ":3",
        "1:",
        "0:3",
        "1:0",
        "3:1",
        "01:3",
        "1:03",
        "+1:3",
        "1: 3",
        "1.0:3",
        "1:65536",
        "65536:65536",
    ] {
        rejected(changed(14, range));
    }
}

#[test]
fn timeouts_require_canonical_milliseconds_between_one_and_five_thousand() {
    for timeout in ["1", "5000"] {
        assert!(parse(changed(16, timeout)).is_ok());
    }
    for timeout in [
        "",
        "0",
        "01",
        "+1",
        "-1",
        "1.0",
        "1e3",
        "5001",
        "4294967295",
        "4294967296",
        "1 ",
        " 1",
    ] {
        rejected(changed(16, timeout));
    }
}

#[test]
fn capability_lists_are_exact_distinct_qualified_names_or_explicit_none() {
    let mut empty = valid();
    empty[18] = "none".into();
    empty[20] = "none".into();
    assert!(parse(empty).is_ok());
    for value in [
        "",
        "NONE",
        "read",
        "Cyanrex.read",
        "cyanrex.1read",
        "cyanrex.",
        "cyanrex..read",
        ".read",
        "cyanrex.读",
        "cyanrex.read,",
        ",cyanrex.read",
        "cyanrex.read,,cyanrex.write",
        "cyanrex.read, cyanrex.write",
        "cyanrex.read,cyanrex.read",
        "none,cyanrex.read",
    ] {
        let mut args = changed(18, value);
        args[20] = "none".into();
        rejected(args);
        rejected(changed(20, value));
    }
    rejected(changed(18, capability(0, 129)));
}

#[test]
fn supported_and_required_capabilities_each_have_at_most_sixteen_entries() {
    let names = (0..16)
        .map(|index| format!("cap.c{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let mut args = changed(18, &names);
    args[20] = names.clone().into();
    assert!(parse(args).is_ok());
    let too_many = format!("{names},cap.extra");
    let mut args = changed(18, &too_many);
    args[20] = "none".into();
    rejected(args);
    rejected(changed(20, too_many));
}

#[test]
fn required_capabilities_cannot_exceed_the_explicit_supported_set() {
    rejected(changed(20, "cyanrex.missing"));
    rejected(changed(18, "none"));
    let mut args = changed(18, "cyanrex.read");
    args[20] = "none".into();
    assert!(parse(args).is_ok());
}

#[test]
fn parsed_policy_selects_highest_overlap_and_mutual_capabilities_in_peer_order() {
    let options = parse(valid()).unwrap();
    let peer = advertisement(
        'b',
        2,
        5,
        &["cyanrex.write", "peer.optional", "cyanrex.read"],
        &["cyanrex.write"],
        direct(),
    );
    let candidate = peer.select(&options.policy, 1010).unwrap();
    assert_eq!(candidate.protocol(), 3);
    assert_eq!(
        candidate
            .capabilities()
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        ["cyanrex.write", "cyanrex.read"]
    );
    let incompatible = advertisement('b', 4, 5, &["cyanrex.read"], &[], direct());
    assert_eq!(
        incompatible.select(&options.policy, 1010).err(),
        Some(DiscoveryError::UnsupportedProtocol)
    );
}

#[test]
fn parsed_policy_keeps_both_mandatory_capability_directions() {
    let options = parse(valid()).unwrap();
    let absent = advertisement('b', 1, 3, &["cyanrex.write"], &[], direct());
    assert_eq!(
        absent.select(&options.policy, 1010).err(),
        Some(DiscoveryError::MissingCapability)
    );
    let unsupported = advertisement(
        'b',
        1,
        3,
        &["cyanrex.read", "peer.mandatory"],
        &["peer.mandatory"],
        direct(),
    );
    assert_eq!(
        unsupported.select(&options.policy, 1010).err(),
        Some(DiscoveryError::MissingCapability)
    );
}

#[test]
fn parsed_policy_never_enables_public_direct_or_peer_named_relays() {
    let options = parse(valid()).unwrap();
    for hints in [
        json!([{"kind":"direct", "address":"8.8.8.8:443"}]),
        json!([{"kind":"relay", "relay_id":"peer.relay"}]),
    ] {
        let peer = advertisement('b', 1, 3, &["cyanrex.read"], &[], hints);
        assert_eq!(
            peer.select(&options.policy, 1010).err(),
            Some(DiscoveryError::NoAllowedHint)
        );
    }
}

#[test]
fn invalid_input_returns_only_unit_error_and_static_usage_never_echoes_the_value() {
    let canary = "synthetic-private-cli-input-must-not-be-retained-98d7";
    assert_eq!(format!("{:?}", parse(changed(0, canary)).err()), "Some(())");
    assert!(!USAGE.contains(canary));
    for name in [
        "browse-lan",
        "--uid",
        "--gid",
        "--groups",
        "--interface",
        "--address",
        "--local-node",
        "--protocol",
        "--timeout-ms",
        "--capabilities",
        "--require",
    ] {
        assert!(USAGE.contains(name), "missing usage field: {name}");
    }
}
