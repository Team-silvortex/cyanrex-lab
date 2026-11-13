use super::*;

#[test]
fn canonical_rfc1918_and_ula_addresses_are_allowed_without_dns_or_network_work() {
    for address in [
        "10.1.2.3:4433",
        "172.16.1.2:4433",
        "172.31.254.253:4433",
        "192.168.1.7:4433",
        "[fc12:3456::7]:4433",
        "[fd12:3456::7]:65535",
    ] {
        let mut value = advertisement();
        value["hints"] = json!([{"kind": "direct", "address": address}]);
        assert_eq!(
            parse(&value).select(&policy(), NOW).unwrap().hints(),
            &[direct(address)]
        );
    }
}

#[test]
fn public_addresses_parse_but_need_an_explicit_per_policy_opt_in() {
    for address in ["8.8.8.8:4433", "[2606:4700:4700::1111]:4433"] {
        let mut value = advertisement();
        value["hints"] = json!([{"kind": "direct", "address": address}]);
        let peer = parse(&value);
        assert!(matches!(
            peer.select(&policy(), NOW),
            Err(DiscoveryError::NoAllowedHint)
        ));
        assert_eq!(
            peer.select(&policy().with_public_direct(), NOW)
                .unwrap()
                .hints(),
            &[direct(address)]
        );
        assert!(matches!(
            peer.select(&policy(), NOW),
            Err(DiscoveryError::NoAllowedHint)
        ));
    }
}

#[test]
fn socket_hints_are_canonical_literals_not_urls_names_ports_or_zone_ids() {
    for address in [
        "teacher.local:4433",
        "https://192.168.1.7:4433",
        "192.168.1.7",
        "192.168.1.7:0",
        "192.168.1.7:04433",
        "192.168.001.7:4433",
        " 192.168.1.7:4433",
        "192.168.1.7:4433 ",
        "192.168.1.7:4433/path",
        "[FD12:3456::7]:4433",
        "[fd12:3456:0:0:0:0:0:7]:4433",
        "fd12:3456::7:4433",
        "[fd12:3456::7%eth0]:4433",
        "[fd12:3456::7%2]:4433",
        "[::ffff:192.168.1.7]:4433",
        "[::ffff:c0a8:107]:4433",
        "192.168.1.7:65536",
    ] {
        let mut value = advertisement();
        value["hints"] = json!([{"kind": "direct", "address": address}]);
        invalid_wire(&value);
    }
}

#[test]
fn ipv4_special_ranges_never_become_targets_even_with_public_direct_enabled() {
    for address in [
        "0.0.0.0:4433",
        "0.1.2.3:4433",
        "127.0.0.1:4433",
        "127.254.253.252:4433",
        "169.254.1.2:4433",
        "169.254.169.254:80",
        "100.64.1.2:4433",
        "100.100.100.200:80",
        "100.127.254.253:4433",
        "192.0.0.1:4433",
        "192.0.2.1:4433",
        "198.51.100.7:4433",
        "203.0.113.7:4433",
        "198.18.1.2:4433",
        "198.19.1.2:4433",
        "192.88.99.1:4433",
        "224.0.0.1:4433",
        "239.255.255.250:1900",
        "240.1.2.3:4433",
        "255.255.255.255:4433",
    ] {
        let mut value = advertisement();
        value["hints"] = json!([{"kind": "direct", "address": address}]);
        invalid_wire(&value);
    }
}

#[test]
fn ipv6_special_ranges_never_become_targets_even_with_public_direct_enabled() {
    for address in [
        "[::]:4433",
        "[::1]:4433",
        "[::7f00:1]:4433",
        "[::ffff:808:808]:4433",
        "[fe80::7]:4433",
        "[febf::7]:4433",
        "[ff02::1]:4433",
        "[fec0::7]:4433",
        "[2001:db8::7]:4433",
        "[2001:2::7]:4433",
        "[2001::7]:4433",
        "[2001:10::7]:4433",
        "[2001:20::7]:4433",
        "[2002:808:808::7]:4433",
        "[64:ff9b::808:808]:4433",
        "[64:ff9b:1::7]:4433",
        "[100::7]:4433",
        "[3fff::7]:4433",
    ] {
        let mut value = advertisement();
        value["hints"] = json!([{"kind": "direct", "address": address}]);
        invalid_wire(&value);
    }
}

fn denied_special_boundary(address: &str) {
    let mut value = advertisement();
    value["hints"] = json!([{"kind": "direct", "address": address}]);
    invalid_wire(&value);
}

fn allowed_public_boundary(address: &str) {
    let mut value = advertisement();
    value["hints"] = json!([{"kind": "direct", "address": address}]);
    let peer = parse(&value);
    assert!(matches!(
        peer.select(&policy(), NOW),
        Err(DiscoveryError::NoAllowedHint)
    ));
    assert_eq!(
        peer.select(&policy().with_public_direct(), NOW)
            .unwrap()
            .hints(),
        &[direct(address)]
    );
}

#[test]
fn ipv4_special_prefix_first_last_and_adjacent_candidates_do_not_bleed_policy() {
    // These are strings only, not assertions that an address is assigned or reachable.
    for (before, first, last, after) in [
        (
            "100.63.255.255:4433",
            "100.64.0.0:4433",
            "100.127.255.255:4433",
            "100.128.0.0:4433",
        ),
        (
            "191.255.255.255:4433",
            "192.0.0.0:4433",
            "192.0.0.255:4433",
            "192.0.1.0:4433",
        ),
        (
            "198.17.255.255:4433",
            "198.18.0.0:4433",
            "198.19.255.255:4433",
            "198.20.0.0:4433",
        ),
    ] {
        allowed_public_boundary(before);
        denied_special_boundary(first);
        denied_special_boundary(last);
        allowed_public_boundary(after);
    }
}

#[test]
fn ipv6_special_prefix_first_last_and_adjacent_candidates_do_not_bleed_policy() {
    // IPv6 public-candidate classification is not allocation, routing or peer identity proof.
    for (before, first, last, after) in [
        (
            "[2000:ffff:ffff:ffff:ffff:ffff:ffff:ffff]:4433",
            "[2001::]:4433",
            "[2001:1ff:ffff:ffff:ffff:ffff:ffff:ffff]:4433",
            "[2001:200::]:4433",
        ),
        (
            "[2001:db7:ffff:ffff:ffff:ffff:ffff:ffff]:4433",
            "[2001:db8::]:4433",
            "[2001:db8:ffff:ffff:ffff:ffff:ffff:ffff]:4433",
            "[2001:db9::]:4433",
        ),
        (
            "[2001:ffff:ffff:ffff:ffff:ffff:ffff:ffff]:4433",
            "[2002::]:4433",
            "[2002:ffff:ffff:ffff:ffff:ffff:ffff:ffff]:4433",
            "[2003::]:4433",
        ),
        (
            "[3ffe:ffff:ffff:ffff:ffff:ffff:ffff:ffff]:4433",
            "[3fff::]:4433",
            "[3fff:fff:ffff:ffff:ffff:ffff:ffff:ffff]:4433",
            "[3fff:1000::]:4433",
        ),
    ] {
        allowed_public_boundary(before);
        denied_special_boundary(first);
        denied_special_boundary(last);
        allowed_public_boundary(after);
    }
}

#[test]
fn hint_lists_are_nonempty_bounded_and_deduplicated_by_exact_target() {
    let mut value = advertisement();
    for hints in [
        json!([]),
        json!([{"kind": "direct", "address": "192.168.1.7:4433"}, {"kind": "direct", "address": "192.168.1.7:4433"}]),
        json!([{"kind": "relay", "relay_id": "lab.home"}, {"kind": "relay", "relay_id": "lab.home"}]),
        json!((1..=MAX_HINTS + 1)
            .map(|i| json!({"kind": "direct", "address": format!("10.1.2.{i}:4433")}))
            .collect::<Vec<_>>()),
    ] {
        value["hints"] = hints;
        invalid_wire(&value);
    }
    value["hints"] = json!((1..=MAX_HINTS)
        .map(|i| json!({"kind": "direct", "address": format!("10.1.2.{i}:4433")}))
        .collect::<Vec<_>>());
    assert_eq!(
        parse(&value).select(&policy(), NOW).unwrap().hints().len(),
        MAX_HINTS
    );
}

#[test]
fn different_ports_on_the_same_literal_remain_distinct_candidates_in_offer_order() {
    let mut value = advertisement();
    value["hints"] = json!([
        {"kind": "direct", "address": "192.168.1.7:8443"},
        {"kind": "direct", "address": "192.168.1.7:4433"}
    ]);
    assert_eq!(
        parse(&value).select(&policy(), NOW).unwrap().hints(),
        &[direct("192.168.1.7:8443"), direct("192.168.1.7:4433")]
    );
}

#[test]
fn relay_names_are_only_canonical_symbolic_ids_never_resolved_or_followed() {
    for relay in [
        "teacher",
        "LAB.home",
        "lab..home",
        "https://relay.invalid",
        "lab.home/private",
        "lab.home?token=private",
    ] {
        let mut value = advertisement();
        value["hints"] = json!([{"kind": "relay", "relay_id": relay}]);
        invalid_wire(&value);
    }
    let mut value = advertisement();
    value["hints"] = json!([{"kind": "relay", "relay_id": "lab.home"}]);
    assert!(matches!(
        parse(&value).select(&policy(), NOW),
        Err(DiscoveryError::NoAllowedHint)
    ));
    assert_eq!(
        parse(&value)
            .select(&policy().with_relays(vec![name("lab.home")]).unwrap(), NOW)
            .unwrap()
            .hints(),
        &[ConnectionHint::Relay(name("lab.home"))]
    );
}
