#![cfg(target_os = "linux")]
//! Pure explicit probe arguments and frozen F0 preparation; no interface or socket work.

#[path = "../src/bin/cyanrex-discovery/probe_arguments.rs"]
mod probe_arguments;

use cyanrex_engine::services::{
    federation_discovery::PeerAdvertisement,
    federation_mdns::{parse_response, MdnsEvent},
};
use probe_arguments::parse;
use serde_json::{json, Value};
use std::{cell::Cell, ffi::OsString, os::unix::ffi::OsStringExt, time::Duration};

fn raw() -> String {
    json!({
        "schema_version":1,"node_fingerprint":"a".repeat(64),
        "authority_id":"8b075b0b-8118-4c37-9224-337ba22f40c6",
        "protocol":{"min":1,"max":3},"offered_capabilities":[],
        "required_capabilities":[],"issued_at":1000,"expires_at":1100,
        "hints":[{"kind":"direct","address":"10.23.0.1:4433"}]
    })
    .to_string()
}

fn valid() -> Vec<OsString> {
    [
        "probe-lan",
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
        "--timeout-ms",
        "2000",
        "--instance",
        "node-one",
        "--host",
        "host-one",
        "--port",
        "4433",
        "--ttl",
        "60",
        "--advertisement",
        &raw(),
    ]
    .into_iter()
    .map(Into::into)
    .collect()
}

fn changed(index: usize, value: impl Into<OsString>) -> Vec<OsString> {
    let mut args = valid();
    args[index] = value.into();
    args
}

#[test]
fn exact_shape_and_pure_prepare_do_not_inspect_the_absent_interface() {
    let options = parse(valid()).unwrap();
    let _: &cyanrex_engine::platform_host::PlatformProcessIdentity = &options.identity;
    assert_eq!(options.config.interface_name(), "absent-fixture");
    assert_eq!(options.config.local_address().to_string(), "10.23.0.1");
    assert_eq!(options.config.timeout(), Duration::from_secs(2));
    assert!(options.prepare_at(1000).is_ok());
    assert_eq!(valid().len(), 23);
}

#[test]
fn every_missing_extra_reordered_or_unknown_argument_is_rejected() {
    for length in 0..23 {
        assert!(parse(valid().into_iter().take(length)).is_err());
    }
    let mut extra = valid();
    extra.push("extra".into());
    assert!(parse(extra).is_err());
    for index in (1..23).step_by(2) {
        assert!(parse(changed(index, "--unknown")).is_err());
    }
    let mut swapped = valid();
    swapped.swap(1, 3);
    swapped.swap(2, 4);
    assert!(parse(swapped).is_err());
    for command in ["browse-lan", "probe", "PROBE-LAN", "probe-lan ", "--help"] {
        assert!(parse(changed(0, command)).is_err());
    }
}

#[test]
fn an_infinite_argument_tail_is_consumed_only_to_the_twenty_fourth_item() {
    let count = Cell::new(0);
    let args = valid()
        .into_iter()
        .chain(std::iter::repeat(OsString::from("extra")));
    assert!(parse(args.inspect(|_| count.set(count.get() + 1))).is_err());
    assert_eq!(count.get(), 24);
}

#[test]
fn non_utf8_and_per_argument_raw_byte_excess_are_rejected_at_every_position() {
    for index in 0..23 {
        assert!(parse(changed(index, OsString::from_vec(vec![0xff]))).is_err());
        let limit = if index == 22 { 4096 } else { 2048 };
        assert!(parse(changed(index, "x".repeat(limit + 1))).is_err());
    }
    let mut aggregate = valid();
    for value in aggregate.iter_mut().take(9) {
        *value = "x".repeat(2048).into();
    }
    assert!(parse(aggregate).is_err());
}

#[test]
fn the_advertisement_alone_accepts_exactly_4096_raw_bytes_without_truncation() {
    let mut declaration = raw();
    declaration.push_str(&" ".repeat(4096 - declaration.len()));
    let options = parse(changed(22, declaration)).unwrap();
    assert!(options.prepare_at(1000).is_ok());
    assert!(parse(changed(22, " ".repeat(4097))).is_err());
}

#[test]
fn identity_and_group_values_keep_existing_canonical_positive_sorted_rules() {
    for index in [2, 4] {
        for value in ["0", "01", "+1", "-1", "1 ", "4294967295", "4294967296"] {
            assert!(parse(changed(index, value)).is_err());
        }
        assert!(parse(changed(index, "4294967294")).is_ok());
    }
    for value in ["none", "1", "1,2,4294967294"] {
        assert!(parse(changed(6, value)).is_ok());
    }
    for value in [
        "", "NONE", "0", "01", "2,1", "1,1", "1,,2", ",1", "1,", "1, 2",
    ] {
        assert!(parse(changed(6, value)).is_err());
    }
    let groups = (1..=129)
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(",");
    assert!(parse(changed(6, groups)).is_err());
}

#[test]
fn exact_rfc1918_literals_and_existing_interface_grammar_are_required() {
    for value in ["eth0", "veth-lan", "eth0.10", "LAN_0", "abcdefghijklmno"] {
        assert!(parse(changed(8, value)).is_ok());
    }
    for value in ["", "-eth0", "eth0:1", "eth 0", "eth0/1", "abcdefghijklmnop"] {
        assert!(parse(changed(8, value)).is_err());
    }
    for value in ["10.23.0.1", "172.16.0.1", "192.168.4.5"] {
        assert!(parse(changed(10, value)).is_ok());
    }
    for value in [
        "localhost",
        "http://10.23.0.1",
        "10.23.0.1:5353",
        "010.23.0.1",
        "127.0.0.1",
        "8.8.8.8",
        "169.254.169.254",
        "224.0.0.251",
        "::1",
        "fd00::1",
    ] {
        assert!(parse(changed(10, value)).is_err());
    }
}

#[test]
fn probe_timeout_is_exactly_the_existing_one_to_two_second_profile() {
    for value in ["1000", "1500", "2000"] {
        assert!(parse(changed(12, value)).is_ok());
    }
    for value in ["0", "999", "2001", "5000", "01000", "+1000", "1e3", "1000 "] {
        assert!(parse(changed(12, value)).is_err());
    }
}

#[test]
fn each_name_is_a_canonical_single_label_and_port_and_ttl_are_bounded() {
    for index in [14, 16] {
        for value in ["a".into(), "7".into(), "a-7".into(), "z".repeat(63)] {
            assert!(parse(changed(index, value)).is_ok());
        }
        for value in [
            "".into(),
            "-a".into(),
            "a-".into(),
            "A".into(),
            "a.b".into(),
            "a_b".into(),
            "노드".into(),
            "z".repeat(64),
        ] {
            assert!(parse(changed(index, value)).is_err());
        }
    }
    for (index, accepted, rejected) in [
        (18, ["1", "65535"], ["0", "65536"]),
        (20, ["1", "120"], ["0", "121"]),
    ] {
        for value in accepted {
            assert!(parse(changed(index, value)).is_ok());
        }
        for value in rejected.into_iter().chain(["01", "+1", "1.0", "1 "]) {
            assert!(parse(changed(index, value)).is_err());
        }
    }
}

#[test]
fn declaration_parsing_is_delayed_until_explicit_pure_preparation() {
    for declaration in ["", "not-json", "{}", "[]", "null"] {
        let options = parse(changed(22, declaration)).unwrap();
        assert!(options.prepare_at(1000).is_err());
    }
}

#[test]
fn preparation_rejects_duplicate_unknown_missing_and_array_records_without_value_erasure() {
    let declaration = raw();
    let mut missing: Value = serde_json::from_str(&declaration).unwrap();
    missing.as_object_mut().unwrap().remove("schema_version");
    for invalid in [
        declaration.replacen('{', "{\"schema_version\":1,", 1),
        declaration.replacen('{', "{\"private-canary\":true,", 1),
        missing.to_string(),
        declaration.replace("\"min\":1", "\"min\":1,\"min\":1"),
        declaration.replace("\"min\":1", "\"min\":1,\"private-canary\":true"),
        declaration.replace("{\"max\":3,\"min\":1}", "[1,3]"),
        format!("{declaration}{declaration}"),
    ] {
        assert!(parse(changed(22, invalid))
            .unwrap()
            .prepare_at(1000)
            .is_err());
    }
}

#[test]
fn preparation_requires_the_declarations_only_hint_to_equal_the_selected_ipv4_endpoint() {
    for hints in [
        json!([{"kind":"direct","address":"10.23.0.2:4433"}]),
        json!([{"kind":"direct","address":"10.23.0.1:4434"}]),
        json!([{"kind":"direct","address":"[fd00::1]:4433"}]),
        json!([{"kind":"relay","relay_id":"lab.home"}]),
        json!([{"kind":"direct","address":"10.23.0.1:4433"},{"kind":"direct","address":"10.23.0.2:4433"}]),
    ] {
        let mut declaration: Value = serde_json::from_str(&raw()).unwrap();
        declaration["hints"] = hints;
        assert!(parse(changed(22, declaration.to_string()))
            .unwrap()
            .prepare_at(1000)
            .is_err());
    }
}

#[test]
fn preparation_does_not_refresh_frozen_issued_or_expiry_times() {
    let options = parse(valid()).unwrap();
    for now in [0, 999, 1100, 1101, 9_007_199_254_740_992, u64::MAX] {
        assert!(options.prepare_at(now).is_err());
    }
    for now in [1000, 1099] {
        let prepared = options.prepare_at(now).unwrap();
        let bytes = prepared.encode_at(now).unwrap();
        match &parse_response(&bytes).unwrap()[0] {
            MdnsEvent::Resolved {
                advertisement, ttl, ..
            } => {
                let declaration: Value = serde_json::from_slice(advertisement).unwrap();
                assert_eq!(declaration["issued_at"], 1000);
                assert_eq!(declaration["expires_at"], 1100);
                assert_eq!(*ttl, if now == 1000 { 60 } else { 1 });
            }
            _ => panic!("a live frozen announcement must remain resolved"),
        }
    }
}

#[test]
fn a_valid_f0_declaration_too_large_for_one_txt_is_rejected_not_truncated() {
    let mut declaration: Value = serde_json::from_str(&raw()).unwrap();
    declaration["offered_capabilities"] = json!((0..16)
        .map(|i| format!("feature.{}{:02}", "a".repeat(118), i))
        .collect::<Vec<_>>());
    let declaration = declaration.to_string();
    assert!(declaration.len() > 1200 && declaration.len() <= 4096);
    assert!(PeerAdvertisement::parse(declaration.as_bytes()).is_ok());
    assert!(parse(changed(22, declaration))
        .unwrap()
        .prepare_at(1000)
        .is_err());
}

#[test]
fn invalid_diagnostics_and_usage_never_reflect_supplied_private_values() {
    let canary = "synthetic-probe-private-value-not-for-output";
    assert_eq!(format!("{:?}", parse(changed(0, canary)).err()), "Some(())");
    assert!(!probe_arguments::USAGE.contains(canary));
    for flag in [
        "probe-lan",
        "--uid",
        "--gid",
        "--groups",
        "--interface",
        "--address",
        "--timeout-ms",
        "--instance",
        "--host",
        "--port",
        "--ttl",
        "--advertisement",
    ] {
        assert!(probe_arguments::USAGE.contains(flag));
    }
}
