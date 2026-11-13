use super::*;

#[test]
fn selection_pins_claims_highest_protocol_intersection_and_only_mutual_capabilities() {
    let candidate = parse(&advertisement()).select(&policy(), NOW).unwrap();
    assert_eq!(candidate.claimed_node(), &fingerprint('b'));
    assert_eq!(
        candidate.claimed_authority(),
        AUTHORITY.parse::<AuthorityId>().unwrap()
    );
    assert_eq!(candidate.protocol(), 3);
    assert_eq!(candidate.capabilities(), &[name(TASK)]);
    assert_eq!(candidate.hints(), &[direct("192.168.1.7:4433")]);
    assert_eq!(candidate.expires_at(), 1_100);
}

#[test]
fn unknown_optional_capabilities_do_not_grant_or_force_a_downgrade() {
    let mut value = advertisement();
    value["offered_capabilities"] = json!(["remote.unknown", TASK, EXTRA]);
    let candidate = parse(&value).select(&policy(), NOW).unwrap();
    assert_eq!(candidate.protocol(), 3);
    assert!(candidate.capabilities().contains(&name(TASK)));
    assert!(candidate.capabilities().contains(&name(EXTRA)));
    assert!(!candidate.capabilities().contains(&name("remote.unknown")));
    assert_eq!(candidate.capabilities().len(), 2);
}

#[test]
fn negotiated_capabilities_preserve_the_peer_offer_order_not_policy_or_sort_order() {
    let mut value = advertisement();
    value["offered_capabilities"] = json!(["remote.unknown", EXTRA, TASK]);
    assert_eq!(
        parse(&value).select(&policy(), NOW).unwrap().capabilities(),
        &[name(EXTRA), name(TASK)]
    );
}

#[test]
fn no_protocol_overlap_fails_without_fallback_to_other_versions() {
    for (min, max) in [(4, 7), (1, 1)] {
        let mut value = advertisement();
        value["protocol"] = json!({"min": min, "max": max});
        let local = DiscoveryPolicy::new(
            fingerprint('a'),
            range(2, 3),
            vec![name(TASK)],
            vec![name(TASK)],
        )
        .unwrap();
        assert!(matches!(
            parse(&value).select(&local, NOW),
            Err(DiscoveryError::UnsupportedProtocol)
        ));
    }
}

#[test]
fn both_sides_mandatory_capabilities_must_be_satisfied() {
    let mut local_missing = advertisement();
    local_missing["offered_capabilities"] = json!([EXTRA]);
    local_missing["required_capabilities"] = json!([]);
    assert!(matches!(
        parse(&local_missing).select(&policy(), NOW),
        Err(DiscoveryError::MissingCapability)
    ));
    let mut remote_missing = advertisement();
    remote_missing["offered_capabilities"] = json!([TASK, "remote.mandatory"]);
    remote_missing["required_capabilities"] = json!([TASK, "remote.mandatory"]);
    assert!(matches!(
        parse(&remote_missing).select(&policy(), NOW),
        Err(DiscoveryError::MissingCapability)
    ));
}

#[test]
fn an_explicitly_empty_capability_contract_is_valid_but_confers_no_authority() {
    let local = DiscoveryPolicy::new(fingerprint('a'), range(1, 1), vec![], vec![]).unwrap();
    let mut value = advertisement();
    value["protocol"] = json!({"min": 1, "max": 1});
    value["offered_capabilities"] = json!([]);
    value["required_capabilities"] = json!([]);
    let candidate = parse(&value).select(&local, NOW).unwrap();
    assert!(candidate.capabilities().is_empty());
    assert_eq!(candidate.protocol(), 1);
}

#[test]
fn policy_rejects_duplicate_or_unsupported_mandatory_capabilities_and_overflow() {
    for (supported, required) in [
        (vec![name(TASK), name(TASK)], vec![]),
        (vec![name(TASK)], vec![name(TASK), name(TASK)]),
        (vec![name(TASK)], vec![name(EXTRA)]),
        (
            (0..=MAX_CAPABILITIES)
                .map(|i| name(&format!("local.c{i}")))
                .collect(),
            vec![],
        ),
    ] {
        assert!(matches!(
            DiscoveryPolicy::new(fingerprint('a'), range(1, 1), supported, required),
            Err(DiscoveryError::InvalidPolicy)
        ));
    }
}

#[test]
fn self_claim_is_rejected_even_when_authority_or_endpoints_differ() {
    let mut value = advertisement();
    value["node_fingerprint"] = json!("a".repeat(64));
    assert!(matches!(
        parse(&value).select(&policy(), NOW),
        Err(DiscoveryError::SelfAdvertisement)
    ));
    value["authority_id"] = json!("267c6b0f-182c-4980-a925-cdc967327c57");
    value["hints"] = json!([{"kind": "direct", "address": "10.1.2.3:4433"}]);
    assert!(matches!(
        parse(&value).select(&policy(), NOW),
        Err(DiscoveryError::SelfAdvertisement)
    ));
}

#[test]
fn foreign_authority_remains_a_claim_not_a_reason_to_merge_or_grant() {
    let mut value = advertisement();
    let other = "267c6b0f-182c-4980-a925-cdc967327c57";
    value["authority_id"] = json!(other);
    let candidate = parse(&value).select(&policy(), NOW).unwrap();
    assert_eq!(
        candidate.claimed_authority(),
        other.parse::<AuthorityId>().unwrap()
    );
    assert_eq!(candidate.claimed_node(), &fingerprint('b'));
}

#[test]
fn expiry_is_strict_read_time_and_is_never_extended_during_encoding_or_selection() {
    let peer = parse(&advertisement());
    assert_eq!(peer.select(&policy(), 1_099).unwrap().expires_at(), 1_100);
    for now in [1_100, 1_101] {
        assert!(matches!(
            peer.select(&policy(), now),
            Err(DiscoveryError::Expired)
        ));
    }
    let encoded = peer.encode().unwrap();
    assert!(matches!(
        PeerAdvertisement::parse(&encoded)
            .unwrap()
            .select(&policy(), 1_100),
        Err(DiscoveryError::Expired)
    ));
}

#[test]
fn future_skew_and_local_clock_are_safe_bounded_integers() {
    let mut value = advertisement();
    value["issued_at"] = json!(NOW + MAX_FUTURE_SKEW_SECONDS);
    value["expires_at"] = json!(NOW + MAX_FUTURE_SKEW_SECONDS + 100);
    assert!(parse(&value).select(&policy(), NOW).is_ok());
    value["issued_at"] = json!(NOW + MAX_FUTURE_SKEW_SECONDS + 1);
    assert!(matches!(
        parse(&value).select(&policy(), NOW),
        Err(DiscoveryError::InvalidTime)
    ));
    for now in [0, 9_007_199_254_740_992, u64::MAX] {
        assert!(matches!(
            parse(&advertisement()).select(&policy(), now),
            Err(DiscoveryError::InvalidTime)
        ));
    }
}

#[test]
fn only_explicit_public_and_operator_configured_relay_hints_are_selectable() {
    let mut value = advertisement();
    value["hints"] = json!([
        {"kind": "direct", "address": "8.8.8.8:4433"},
        {"kind": "relay", "relay_id": "lab.home"}
    ]);
    assert!(matches!(
        parse(&value).select(&policy(), NOW),
        Err(DiscoveryError::NoAllowedHint)
    ));
    let public = policy().with_public_direct();
    assert_eq!(
        parse(&value).select(&public, NOW).unwrap().hints(),
        &[direct("8.8.8.8:4433")]
    );
    let relay = policy().with_relays(vec![name("lab.home")]).unwrap();
    assert_eq!(
        parse(&value).select(&relay, NOW).unwrap().hints(),
        &[ConnectionHint::Relay(name("lab.home"))]
    );
}

#[test]
fn hints_are_stably_partitioned_private_then_public_then_operator_allowed_relays() {
    let mut value = advertisement();
    value["hints"] = json!([
        {"kind": "relay", "relay_id": "lab.second"},
        {"kind": "direct", "address": "8.8.8.8:4433"},
        {"kind": "direct", "address": "10.2.3.4:4433"},
        {"kind": "relay", "relay_id": "lab.unconfigured"},
        {"kind": "direct", "address": "[2606:4700:4700::1111]:4433"},
        {"kind": "direct", "address": "[fd12:3456::7]:4433"},
        {"kind": "relay", "relay_id": "lab.first"}
    ]);
    let local = policy()
        .with_public_direct()
        .with_relays(vec![name("lab.first"), name("lab.second")])
        .unwrap();
    assert_eq!(
        parse(&value).select(&local, NOW).unwrap().hints(),
        &[
            direct("10.2.3.4:4433"),
            direct("[fd12:3456::7]:4433"),
            direct("8.8.8.8:4433"),
            direct("[2606:4700:4700::1111]:4433"),
            ConnectionHint::Relay(name("lab.second")),
            ConnectionHint::Relay(name("lab.first"))
        ]
    );
}

#[test]
fn relay_policy_rejects_duplicates_and_unbounded_operator_input() {
    assert!(policy().with_relays(vec![]).is_ok());
    assert!(matches!(
        policy().with_relays(vec![name("lab.home"), name("lab.home")]),
        Err(DiscoveryError::InvalidPolicy)
    ));
    assert!(matches!(
        policy().with_relays(
            (0..=MAX_HINTS)
                .map(|i| name(&format!("lab.r{i}")))
                .collect()
        ),
        Err(DiscoveryError::InvalidPolicy)
    ));
}

#[test]
fn errors_use_static_classifications_and_never_echo_untrusted_values() {
    let mut value = advertisement();
    let secret = "https://private-user:private-password@private-host.invalid/private-cookie";
    value["hints"] = json!([{"kind": "direct", "address": secret}]);
    let error = match PeerAdvertisement::parse(&serde_json::to_vec(&value).unwrap()) {
        Err(error) => error,
        Ok(_) => panic!("private URL must not parse as a socket address"),
    };
    assert!(matches!(error, DiscoveryError::InvalidAdvertisement));
    for error in [
        error,
        DiscoveryError::InvalidPolicy,
        DiscoveryError::Expired,
        DiscoveryError::UnsupportedProtocol,
        DiscoveryError::MissingCapability,
        DiscoveryError::SelfAdvertisement,
        DiscoveryError::NoAllowedHint,
        DiscoveryError::InvalidTime,
    ] {
        let rendered = format!("{error:?} {error}");
        for forbidden in [
            secret,
            "private-user",
            "private-password",
            "private-host",
            "private-cookie",
            AUTHORITY,
            &"b".repeat(64),
        ] {
            assert!(!rendered.contains(forbidden));
        }
        assert!(rendered.len() <= 160);
    }
}
