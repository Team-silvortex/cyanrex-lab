use super::*;

#[test]
fn first_observation_retains_source_claim_and_a_bounded_initial_deadline() {
    let mut cache = configured(1);
    let local = source(11, "node-one");
    add(&mut cache, &local);
    let candidates = cache.snapshot(time(1_010, 200)).unwrap();
    assert_eq!(candidates.len(), 1);
    let candidate = &candidates[0];
    assert_eq!(candidate.source().interface_epoch(), 11);
    assert_eq!(candidate.source().instance(), "node-one");
    assert_eq!(candidate.candidate().claimed_node(), &fingerprint('b'));
    assert_eq!(
        candidate.candidate().claimed_authority().to_string(),
        AUTHORITY
    );
    assert_eq!(candidate.candidate().protocol(), 3);
    assert_eq!(candidate.candidate().capabilities(), &[name(TASK)]);
    assert_eq!(candidate.candidate().expires_at(), 1_100);
    assert_eq!(candidate.deadline_monotonic(), 230);
}

#[test]
fn exact_or_json_format_only_duplicates_do_not_extend_original_ttl() {
    let mut cache = configured(1);
    let local = source(11, "node-one");
    add(&mut cache, &local);
    let pretty = serde_json::to_vec_pretty(&advertisement()).unwrap();
    assert!(matches!(
        cache.observe(&local, &pretty, MAX_LAN_TTL_SECONDS, time(1_029, 229)),
        Ok(LanObservation::Duplicate)
    ));
    assert_eq!(
        cache.snapshot(time(1_029, 229)).unwrap()[0].deadline_monotonic(),
        230
    );
    assert!(cache.snapshot(time(1_030, 230)).unwrap().is_empty());
}

#[test]
fn escaped_known_key_duplicate_still_uses_canonical_complete_advertisement() {
    let mut cache = configured(1);
    let local = source(11, "node-one");
    add(&mut cache, &local);
    let escaped = serde_json::to_string(&advertisement()).unwrap().replacen(
        "\"schema_version\"",
        "\"\\u0073chema_version\"",
        1,
    );
    assert!(matches!(
        cache.observe(&local, escaped.as_bytes(), 120, time(1_011, 201)),
        Ok(LanObservation::Duplicate)
    ));
    assert_eq!(
        cache.snapshot(time(1_011, 201)).unwrap()[0].deadline_monotonic(),
        230
    );
}

#[test]
fn active_source_changes_cannot_override_claims_hints_or_compatibility() {
    let mut cache = configured(1);
    let local = source(11, "node-one");
    add(&mut cache, &local);
    for (field, replacement) in [
        ("node_fingerprint", json!("c".repeat(64))),
        ("authority_id", json!(OTHER_AUTHORITY)),
        ("issued_at", json!(1_001)),
        ("expires_at", json!(1_101)),
        ("protocol", json!({"min": 2, "max": 3})),
        ("offered_capabilities", json!([TASK, "remote.changed"])),
        (
            "hints",
            json!([{"kind": "direct", "address": "10.1.2.3:4433"}]),
        ),
    ] {
        let mut changed = advertisement();
        changed[field] = replacement;
        assert!(matches!(
            cache.observe(&local, &raw(&changed), 30, time(1_011, 201)),
            Err(LanDiscoveryError::SourceConflict)
        ));
    }
    let original = cache.snapshot(time(1_011, 201)).unwrap();
    assert_eq!(original.len(), 1);
    assert_eq!(original[0].candidate().claimed_node(), &fingerprint('b'));
    assert_eq!(
        original[0].candidate().claimed_authority().to_string(),
        AUTHORITY
    );
    assert_eq!(original[0].deadline_monotonic(), 230);
}

#[test]
fn malformed_and_incompatible_observations_do_not_replace_existing_content() {
    let mut cache = configured(1);
    let local = source(11, "node-one");
    add(&mut cache, &local);
    assert!(matches!(
        cache.observe(&local, b"not-json-private-secret", 30, time(1_011, 201)),
        Err(LanDiscoveryError::RejectedAdvertisement(
            DiscoveryError::InvalidAdvertisement
        ))
    ));
    let mut incompatible = advertisement();
    incompatible["protocol"] = json!({"min": 4, "max": 4});
    assert!(matches!(
        cache.observe(&local, &raw(&incompatible), 30, time(1_011, 201)),
        Err(LanDiscoveryError::SourceConflict)
    ));
    assert!(matches!(
        cache.observe(
            &source(11, "incompatible-node"),
            &raw(&incompatible),
            30,
            time(1_011, 201)
        ),
        Err(LanDiscoveryError::RejectedAdvertisement(
            DiscoveryError::UnsupportedProtocol
        ))
    ));
    let original = cache.snapshot(time(1_011, 201)).unwrap();
    assert_eq!(original.len(), 1);
    assert_eq!(original[0].candidate().protocol(), 3);
    assert_eq!(original[0].deadline_monotonic(), 230);
}

#[test]
fn full_cache_does_not_evict_active_entries_or_overwrite_an_equal_claim() {
    let mut cache = configured(1);
    let first = source(11, "node-one");
    add(&mut cache, &first);
    assert!(matches!(
        cache.observe(
            &source(22, "node-one"),
            &raw(&advertisement()),
            30,
            time(1_011, 201)
        ),
        Err(LanDiscoveryError::Capacity)
    ));
    assert!(matches!(
        cache.observe(&first, &raw(&advertisement()), 30, time(1_011, 201)),
        Ok(LanObservation::Duplicate)
    ));
    let candidates = cache.snapshot(time(1_011, 201)).unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].source().interface_epoch(), 11);
    assert_eq!(candidates[0].deadline_monotonic(), 230);
}

#[test]
fn exact_maximum_capacity_is_bounded_even_when_all_sources_repeat_one_claim() {
    let mut cache = LanCandidateCache::new(policy(), vec![11], MAX_LAN_CANDIDATES).unwrap();
    for index in 0..MAX_LAN_CANDIDATES {
        add(&mut cache, &source(11, &format!("node-{index}")));
    }
    let candidates = cache.snapshot(time(1_010, 201)).unwrap();
    assert_eq!(candidates.len(), MAX_LAN_CANDIDATES);
    assert_eq!(candidates[0].source().instance(), "node-0");
    assert_eq!(
        candidates[MAX_LAN_CANDIDATES - 1].source().instance(),
        "node-63"
    );
    assert!(matches!(
        cache.observe(
            &source(11, "overflow"),
            &raw(&advertisement()),
            30,
            time(1_010, 201)
        ),
        Err(LanDiscoveryError::Capacity)
    ));
    assert_eq!(
        cache.snapshot(time(1_010, 201)).unwrap().len(),
        MAX_LAN_CANDIDATES
    );
}

#[test]
fn expired_capacity_can_be_reclaimed_without_evicting_an_active_peer() {
    let mut cache = configured(2);
    assert!(matches!(
        cache.observe(
            &source(11, "short"),
            &raw(&advertisement()),
            1,
            time(1_010, 200)
        ),
        Ok(LanObservation::Added)
    ));
    add(&mut cache, &source(11, "long"));
    assert!(matches!(
        cache.observe(
            &source(22, "new"),
            &raw(&advertisement()),
            30,
            time(1_011, 201)
        ),
        Ok(LanObservation::Added)
    ));
    let candidates = cache.snapshot(time(1_011, 201)).unwrap();
    assert_eq!(
        candidates
            .iter()
            .map(|entry| entry.source().instance())
            .collect::<Vec<_>>(),
        vec!["long", "new"]
    );
    assert_eq!(candidates[0].deadline_monotonic(), 230);
    assert_eq!(candidates[1].deadline_monotonic(), 231);
}

#[test]
fn changed_advertisement_may_be_added_only_after_original_source_expiry() {
    let mut cache = configured(1);
    let local = source(11, "node-one");
    add(&mut cache, &local);
    let mut changed = advertisement();
    changed["node_fingerprint"] = json!("c".repeat(64));
    assert!(matches!(
        cache.observe(&local, &raw(&changed), 30, time(1_030, 230)),
        Ok(LanObservation::Added)
    ));
    let candidates = cache.snapshot(time(1_030, 230)).unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].candidate().claimed_node(), &fingerprint('c'));
    assert_eq!(candidates[0].deadline_monotonic(), 260);
}

#[test]
fn initial_insertion_order_is_stable_across_duplicates_and_explicit_removal() {
    let mut cache = configured(3);
    for instance in ["third", "first", "second"] {
        add(&mut cache, &source(11, instance));
    }
    assert!(matches!(
        cache.observe(
            &source(11, "first"),
            &raw(&advertisement()),
            120,
            time(1_011, 201)
        ),
        Ok(LanObservation::Duplicate)
    ));
    assert!(cache
        .remove(&source(11, "first"), time(1_011, 201))
        .unwrap());
    assert!(matches!(
        cache.observe(
            &source(11, "first"),
            &raw(&advertisement()),
            30,
            time(1_011, 201)
        ),
        Ok(LanObservation::Added)
    ));
    assert_eq!(
        cache
            .snapshot(time(1_011, 201))
            .unwrap()
            .iter()
            .map(|entry| entry.source().instance())
            .collect::<Vec<_>>(),
        vec!["third", "second", "first"]
    );
}

#[test]
fn fixed_policy_never_adopts_public_or_relay_hints_from_an_advertisement() {
    let mut cache = configured(2);
    for hints in [
        json!([{"kind": "direct", "address": "8.8.8.8:4433"}]),
        json!([{"kind": "relay", "relay_id": "lab.home"}]),
    ] {
        let mut value = advertisement();
        value["hints"] = hints;
        assert!(matches!(
            cache.observe(&source(11, "node-one"), &raw(&value), 30, time(1_010, 200)),
            Err(LanDiscoveryError::RejectedAdvertisement(
                DiscoveryError::NoAllowedHint
            ))
        ));
    }
    assert!(cache.snapshot(time(1_010, 200)).unwrap().is_empty());
}

#[test]
fn explicit_local_policy_only_retains_filtered_hints_not_network_authentication() {
    let mut cache = LanCandidateCache::new(policy().with_public_direct(), vec![11], 1).unwrap();
    let mut value = advertisement();
    value["hints"] = json!([{"kind": "direct", "address": "8.8.8.8:4433"}, {"kind": "relay", "relay_id": "lab.home"}]);
    assert!(matches!(
        cache.observe(&source(11, "node-one"), &raw(&value), 30, time(1_010, 200)),
        Ok(LanObservation::Added)
    ));
    let candidates = cache.snapshot(time(1_010, 200)).unwrap();
    assert_eq!(candidates[0].candidate().hints().len(), 1);
    assert_eq!(candidates[0].candidate().claimed_node(), &fingerprint('b'));
}

#[test]
fn removing_missing_or_expired_sources_is_not_a_node_wide_revocation() {
    let mut cache = configured(1);
    assert!(!cache
        .remove(&source(11, "missing"), time(1_010, 200))
        .unwrap());
    add(&mut cache, &source(11, "node-one"));
    assert!(!cache
        .remove(&source(11, "missing"), time(1_011, 201))
        .unwrap());
    assert_eq!(cache.snapshot(time(1_011, 201)).unwrap().len(), 1);
    assert!(!cache
        .remove(&source(11, "node-one"), time(1_030, 230))
        .unwrap());
    assert!(cache.snapshot(time(1_030, 230)).unwrap().is_empty());
}

#[test]
fn stop_clears_candidate_availability_and_permanently_closes_all_entry_points() {
    let mut cache = configured(1);
    let local = source(11, "node-one");
    add(&mut cache, &local);
    cache.stop();
    cache.stop();
    assert!(matches!(
        cache.snapshot(time(1_011, 201)),
        Err(LanDiscoveryError::Closed)
    ));
    assert!(matches!(
        cache.observe(&local, &raw(&advertisement()), 30, time(1_011, 201)),
        Err(LanDiscoveryError::Closed)
    ));
    assert!(matches!(
        cache.remove(&local, time(1_011, 201)),
        Err(LanDiscoveryError::Closed)
    ));
}

#[test]
fn cache_errors_are_static_and_do_not_echo_raw_packets_or_source_names() {
    let mut cache = configured(1);
    let local = source(11, "private-node");
    let secret = "private-cookie-private-password-private-address";
    let error = match cache.observe(&local, secret.as_bytes(), 30, time(1_010, 200)) {
        Err(error) => error,
        Ok(_) => panic!("invalid packet must be rejected"),
    };
    for error in [
        error,
        LanDiscoveryError::InvalidConfig,
        LanDiscoveryError::InvalidSource,
        LanDiscoveryError::InvalidTxt,
        LanDiscoveryError::TooLarge,
        LanDiscoveryError::InvalidTtl,
        LanDiscoveryError::InvalidTime,
        LanDiscoveryError::ClockRollback,
        LanDiscoveryError::Closed,
        LanDiscoveryError::RateLimited,
        LanDiscoveryError::Capacity,
        LanDiscoveryError::SourceConflict,
    ] {
        let rendered = format!("{error:?} {error}");
        for forbidden in [
            secret,
            "private-node",
            AUTHORITY,
            "192.168.1.7",
            &"b".repeat(64),
        ] {
            assert!(!rendered.contains(forbidden));
        }
        assert!(rendered.len() <= 200);
    }
}
