use super::*;

#[test]
fn source_is_a_local_interface_incarnation_and_canonical_instance_label() {
    for instance in ["a", "0", "node-one", "node--two", &"f".repeat(63)] {
        let local = LanSource::new(11, instance).unwrap();
        assert_eq!(local.interface_epoch(), 11);
        assert_eq!(local.instance(), instance);
    }
    let highest_epoch = LanSource::new(u64::MAX, "node-one").unwrap();
    assert_eq!(highest_epoch.interface_epoch(), u64::MAX);
}

#[test]
fn source_rejects_empty_zero_noncanonical_or_secret_bearing_names() {
    assert!(matches!(
        LanSource::new(0, "node-one"),
        Err(LanDiscoveryError::InvalidSource)
    ));
    for instance in [
        "",
        "-node",
        "node-",
        "Node",
        "node.local",
        "node_one",
        "node/name",
        "node:4433",
        " node",
        "node ",
        "node\n",
        "node\0",
        "노드",
        "节点",
        "node–one",
        "https://node.invalid",
        &"b".repeat(64),
    ] {
        assert!(matches!(
            LanSource::new(11, instance),
            Err(LanDiscoveryError::InvalidSource)
        ));
    }
}

#[test]
fn cache_configuration_is_nonempty_unique_approved_interfaces_and_bounded_capacity() {
    for epochs in [vec![], vec![0], vec![11, 11], (1..=9).collect()] {
        assert!(matches!(
            LanCandidateCache::new(policy(), epochs, 1),
            Err(LanDiscoveryError::InvalidConfig)
        ));
    }
    for capacity in [0, MAX_LAN_CANDIDATES + 1, usize::MAX] {
        assert!(matches!(
            LanCandidateCache::new(policy(), vec![11], capacity),
            Err(LanDiscoveryError::InvalidConfig)
        ));
    }
    assert!(LanCandidateCache::new(policy(), (1..=8).collect(), MAX_LAN_CANDIDATES).is_ok());
}

#[test]
fn unapproved_interface_is_rejected_without_replacing_the_approved_source() {
    let mut cache = configured(2);
    let allowed = source(11, "node-one");
    add(&mut cache, &allowed);
    let forbidden = source(33, "node-one");
    assert!(matches!(
        cache.observe(&forbidden, &raw(&advertisement()), 30, time(1_011, 201)),
        Err(LanDiscoveryError::InvalidSource)
    ));
    assert!(matches!(
        cache.remove(&forbidden, time(1_011, 201)),
        Err(LanDiscoveryError::InvalidSource)
    ));
    let candidates = cache.snapshot(time(1_011, 201)).unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].source().interface_epoch(), 11);
    assert_eq!(candidates[0].source().instance(), "node-one");
}

#[test]
fn equal_unverified_claims_on_distinct_names_and_interfaces_are_not_merged() {
    let mut cache = configured(3);
    for local in [
        source(11, "node-one"),
        source(22, "node-one"),
        source(11, "node-two"),
    ] {
        add(&mut cache, &local);
    }
    let candidates = cache.snapshot(time(1_011, 201)).unwrap();
    assert_eq!(candidates.len(), 3);
    for candidate in &candidates {
        assert_eq!(candidate.candidate().claimed_node(), &fingerprint('b'));
    }
    assert_eq!(
        candidates
            .iter()
            .map(|entry| (entry.source().interface_epoch(), entry.source().instance()))
            .collect::<Vec<_>>(),
        vec![(11, "node-one"), (22, "node-one"), (11, "node-two")]
    );
}

#[test]
fn removing_an_instance_is_exact_to_its_interface_and_does_not_revoke_another_claim() {
    let mut cache = configured(3);
    for local in [
        source(11, "node-one"),
        source(22, "node-one"),
        source(11, "node-two"),
    ] {
        add(&mut cache, &local);
    }
    assert!(cache
        .remove(&source(11, "node-one"), time(1_011, 201))
        .unwrap());
    assert!(!cache
        .remove(&source(11, "node-one"), time(1_011, 201))
        .unwrap());
    let candidates = cache.snapshot(time(1_011, 201)).unwrap();
    assert_eq!(
        candidates
            .iter()
            .map(|entry| (entry.source().interface_epoch(), entry.source().instance()))
            .collect::<Vec<_>>(),
        vec![(22, "node-one"), (11, "node-two")]
    );
    for candidate in candidates {
        assert_eq!(candidate.candidate().claimed_node(), &fingerprint('b'));
    }
}

#[test]
fn the_same_instance_label_in_a_new_interface_incarnation_is_a_different_source() {
    let mut cache = LanCandidateCache::new(policy(), vec![11, 111], 2).unwrap();
    add(&mut cache, &source(11, "node-one"));
    add(&mut cache, &source(111, "node-one"));
    assert!(cache
        .remove(&source(11, "node-one"), time(1_011, 201))
        .unwrap());
    let candidates = cache.snapshot(time(1_011, 201)).unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].source().interface_epoch(), 111);
}
