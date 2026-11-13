use super::*;

#[test]
fn valid_descriptor_round_trips_with_no_identity_or_time_reallocation() {
    let value = advertisement();
    let peer = parse(&value);
    let encoded = peer.encode().unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&encoded).unwrap(), value);
    let reparsed = PeerAdvertisement::parse(&encoded).unwrap();
    let candidate = reparsed.select(&policy(), NOW).unwrap();
    assert_eq!(candidate.claimed_node(), &fingerprint('b'));
    assert_eq!(
        candidate.claimed_authority(),
        AUTHORITY.parse::<AuthorityId>().unwrap()
    );
    assert_eq!(candidate.expires_at(), 1_100);
}

#[test]
fn descriptor_wire_budget_is_enforced_before_parsing_including_whitespace() {
    let original = serde_json::to_vec(&advertisement()).unwrap();
    let mut exact = original;
    exact.resize(MAX_ADVERTISEMENT_BYTES, b' ');
    assert!(PeerAdvertisement::parse(&exact).is_ok());
    exact.push(b' ');
    assert!(matches!(
        PeerAdvertisement::parse(&exact),
        Err(DiscoveryError::InvalidAdvertisement)
    ));
}

#[test]
fn malformed_utf8_nonobjects_trailing_data_and_incomplete_json_are_rejected() {
    for invalid in [
        &b""[..],
        &b"\xff"[..],
        &b"null"[..],
        &b"[]"[..],
        &b"{} {}"[..],
        &b"{\"schema_version\":"[..],
    ] {
        assert!(matches!(
            PeerAdvertisement::parse(invalid),
            Err(DiscoveryError::InvalidAdvertisement)
        ));
    }
}

#[test]
fn every_top_level_field_is_required_without_null_or_unknown_overrides() {
    let value = advertisement();
    for key in value.as_object().unwrap().keys() {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(key);
        invalid_wire(&missing);
        let mut null = value.clone();
        null[key] = Value::Null;
        invalid_wire(&null);
    }
    for (key, val) in [
        ("verified", json!(true)),
        ("owner", json!(AUTHORITY)),
        ("cookie", json!("private-secret")),
        ("product_version", json!("0.6.5")),
    ] {
        let mut unknown = value.clone();
        unknown[key] = val;
        invalid_wire(&unknown);
    }
}

#[test]
fn schema_and_authority_are_exact_not_normalized_or_adopted() {
    for schema in [json!(0), json!(2), json!(1.0), json!("1"), json!(true)] {
        let mut value = advertisement();
        value["schema_version"] = schema;
        invalid_wire(&value);
    }
    for authority in [
        "00000000-0000-0000-0000-000000000000",
        "8B075B0B-8118-4C37-9224-337BA22F40C6",
        "8b075b0b81184c379224337ba22f40c6",
        "teacher",
    ] {
        let mut value = advertisement();
        value["authority_id"] = json!(authority);
        invalid_wire(&value);
    }
}

#[test]
fn fingerprint_claim_validation_is_not_bypassed_by_wire_deserialization() {
    for claimed in [
        json!("B".repeat(64)),
        json!("b".repeat(63)),
        json!("b".repeat(65)),
        json!("z".repeat(64)),
        json!(format!("{} ", "b".repeat(64))),
        json!(42),
        json!(["b".repeat(64)]),
    ] {
        let mut value = advertisement();
        value["node_fingerprint"] = claimed;
        invalid_wire(&value);
    }
}

#[test]
fn nested_records_are_map_only_complete_and_unknown_field_closed() {
    for protocol in [
        json!([2, 4]),
        json!({"min": 2}),
        json!({"max": 4}),
        json!({"min": 2, "max": 4, "fallback": 1}),
        json!({"min": 0, "max": 4}),
        json!({"min": 4, "max": 2}),
        json!({"min": 1, "max": 65_536}),
        json!({"min": "2", "max": 4}),
    ] {
        let mut value = advertisement();
        value["protocol"] = protocol;
        invalid_wire(&value);
    }
    for hint in [
        json!(["direct", "192.168.1.7:4433"]),
        json!({"kind": "direct"}),
        json!({"address": "192.168.1.7:4433"}),
        json!({"kind": "direct", "address": "192.168.1.7:4433", "relay_id": "lab.home"}),
        json!({"kind": "relay", "relay_id": "lab.home", "address": "192.168.1.7:4433"}),
        json!({"kind": "relay"}),
        json!({"kind": "upnp", "address": "192.168.1.7:4433"}),
    ] {
        let mut value = advertisement();
        value["hints"] = json!([hint]);
        invalid_wire(&value);
    }
}

#[test]
fn containers_and_nested_scalars_do_not_coerce_strings_floats_null_or_booleans() {
    for field in ["offered_capabilities", "required_capabilities", "hints"] {
        for scalar in [json!(""), json!({}), json!(false), json!(1)] {
            let mut value = advertisement();
            value[field] = scalar;
            invalid_wire(&value);
        }
    }
    for field in ["min", "max"] {
        for scalar in [json!(null), json!(false), json!(-1), json!(2.0)] {
            let mut value = advertisement();
            value["protocol"][field] = scalar;
            invalid_wire(&value);
        }
    }
    for field in ["kind", "address"] {
        for scalar in [json!(null), json!(false), json!(1), json!([])] {
            let mut value = advertisement();
            value["hints"][0][field] = scalar;
            invalid_wire(&value);
        }
    }
}

#[test]
fn raw_duplicates_are_rejected_at_every_level_even_with_equal_values() {
    let original = serde_json::to_string(&advertisement()).unwrap();
    for (from, to) in [
        (
            "\"schema_version\":1",
            "\"schema_version\":1,\"schema_version\":1",
        ),
        (
            "\"schema_version\":1",
            "\"schema_version\":1,\"\\u0073chema_version\":1",
        ),
        ("\"max\":4", "\"max\":4,\"max\":4"),
        ("\"min\":2", "\"min\":2,\"\\u006din\":2"),
        (
            "\"kind\":\"direct\"",
            "\"kind\":\"direct\",\"kind\":\"direct\"",
        ),
        (
            "\"address\":\"192.168.1.7:4433\"",
            "\"address\":\"192.168.1.7:4433\",\"\\u0061ddress\":\"192.168.1.7:4433\"",
        ),
    ] {
        assert!(
            original.contains(from),
            "test replacement must be effective"
        );
        let invalid = original.replacen(from, to, 1);
        assert!(matches!(
            PeerAdvertisement::parse(invalid.as_bytes()),
            Err(DiscoveryError::InvalidAdvertisement)
        ));
    }
}

#[test]
fn one_escaped_known_key_retains_ordinary_json_meaning() {
    let original = serde_json::to_string(&advertisement()).unwrap();
    let escaped = original.replacen("\"schema_version\"", "\"\\u0073chema_version\"", 1);
    assert!(PeerAdvertisement::parse(escaped.as_bytes()).is_ok());
}

#[test]
fn relay_discriminant_and_identifier_duplicates_are_rejected_before_selection() {
    let mut value = advertisement();
    value["hints"] = json!([{"kind": "relay", "relay_id": "lab.home"}]);
    let original = serde_json::to_string(&value).unwrap();
    for (from, to) in [
        (
            "\"kind\":\"relay\"",
            "\"kind\":\"relay\",\"kind\":\"relay\"",
        ),
        (
            "\"relay_id\":\"lab.home\"",
            "\"relay_id\":\"lab.home\",\"relay_id\":\"lab.home\"",
        ),
        (
            "\"relay_id\":\"lab.home\"",
            "\"relay_id\":\"lab.home\",\"\\u0072elay_id\":\"lab.home\"",
        ),
    ] {
        assert!(original.contains(from));
        let invalid = original.replacen(from, to, 1);
        assert!(matches!(
            PeerAdvertisement::parse(invalid.as_bytes()),
            Err(DiscoveryError::InvalidAdvertisement)
        ));
    }
}

#[test]
fn capabilities_are_bounded_distinct_canonical_names_and_required_is_offered() {
    for field in ["offered_capabilities", "required_capabilities"] {
        for capabilities in [
            json!([TASK, TASK]),
            json!(["TASK.READ"]),
            json!(["read"]),
            json!(["cyanrex..task"]),
            json!(["cyanrex/task"]),
            json!([null]),
            json!([1]),
            json!([format!("cyanrex.{}", "a".repeat(121))]),
            json!((0..=MAX_CAPABILITIES)
                .map(|i| format!("cyanrex.c{i}"))
                .collect::<Vec<_>>()),
        ] {
            let mut value = advertisement();
            value[field] = capabilities;
            invalid_wire(&value);
        }
    }
    let mut mismatch = advertisement();
    mismatch["required_capabilities"] = json!(["cyanrex.absent.v1"]);
    invalid_wire(&mismatch);
    let mut maximum = advertisement();
    maximum["offered_capabilities"] = json!((0..MAX_CAPABILITIES)
        .map(|i| format!("cyanrex.c{i}"))
        .collect::<Vec<_>>());
    maximum["required_capabilities"] = maximum["offered_capabilities"].clone();
    assert!(PeerAdvertisement::parse(&serde_json::to_vec(&maximum).unwrap()).is_ok());
}

#[test]
fn leases_are_positive_safe_integers_with_bounded_nonzero_duration() {
    for (issued, expires) in [
        (json!(0), json!(100)),
        (json!(1_000), json!(1_000)),
        (json!(1_001), json!(1_000)),
        (json!(1_000), json!(1_301)),
        (json!(-1), json!(100)),
        (json!(1_000.0), json!(1_100)),
        (json!(1_000), json!(1_100.0)),
        (json!("1000"), json!(1_100)),
        (json!(1_000), json!(9_007_199_254_740_992_u64)),
    ] {
        let mut value = advertisement();
        value["issued_at"] = issued;
        value["expires_at"] = expires;
        invalid_wire(&value);
    }
    for duration in [1, MAX_LEASE_SECONDS] {
        let mut value = advertisement();
        value["expires_at"] = json!(1_000 + duration);
        assert!(PeerAdvertisement::parse(&serde_json::to_vec(&value).unwrap()).is_ok());
    }
}

#[test]
fn maximum_json_safe_lease_retains_exact_integer_values_without_overflow() {
    let max = 9_007_199_254_740_991_u64;
    let mut value = advertisement();
    value["issued_at"] = json!(max - 1);
    value["expires_at"] = json!(max);
    let peer = parse(&value);
    assert_eq!(peer.select(&policy(), max - 1).unwrap().expires_at(), max);
    assert!(matches!(
        peer.select(&policy(), max),
        Err(DiscoveryError::Expired)
    ));
    assert_eq!(
        serde_json::from_slice::<Value>(&peer.encode().unwrap()).unwrap(),
        value
    );
    value["issued_at"] = json!(max + 1);
    value["expires_at"] = json!(max + 2);
    invalid_wire(&value);
}
