use super::*;
use serde_json::json;

fn intake() -> Intake {
    Intake::new(
        DiscoveryPolicy::new(
            "a".repeat(64).parse().unwrap(),
            crate::services::federation_discovery::ProtocolRange::new(1, 3).unwrap(),
            vec![],
            vec![],
        )
        .unwrap(),
        7,
    )
    .unwrap()
}
fn time(wall: u64, monotonic: u64) -> CacheTime {
    CacheTime::new(wall, monotonic).unwrap()
}
fn event(address: &str, alias: &str, port: u16) -> MdnsEvent {
    let advertisement = serde_json::to_vec(&json!({
        "schema_version":1,"node_fingerprint":"b".repeat(64),
        "authority_id":"8b075b0b-8118-4c37-9224-337ba22f40c6",
        "protocol":{"min":1,"max":3},"offered_capabilities":[],"required_capabilities":[],
        "issued_at":1000,"expires_at":1100,
        "hints":[{"kind":"direct","address":format!("{address}:4433")}]
    }))
    .unwrap();
    MdnsEvent::Resolved {
        instance: alias.into(),
        advertisement,
        addresses: vec![address.parse().unwrap()],
        port,
        ttl: 30,
    }
}

#[test]
fn sender_resolved_address_and_srv_port_must_all_match_selected_f0_hint() {
    let sender = "10.23.0.2".parse().unwrap();
    let mut state = intake();
    state
        .apply(
            event("10.23.0.3", "wrong-source", 4433),
            sender,
            time(1010, 0),
            1010,
        )
        .unwrap();
    state
        .apply(
            event("10.23.0.2", "wrong-port", 4434),
            sender,
            time(1010, 0),
            1010,
        )
        .unwrap();
    let mut wrong_a = event("10.23.0.2", "wrong-a", 4433);
    if let MdnsEvent::Resolved { addresses, .. } = &mut wrong_a {
        *addresses = vec!["10.23.0.3".parse().unwrap()];
    }
    state.apply(wrong_a, sender, time(1010, 0), 1010).unwrap();
    assert!(state.cache.snapshot(time(1010, 0)).unwrap().is_empty());
    state
        .apply(
            event("10.23.0.2", "valid", 4433),
            sender,
            time(1010, 0),
            1010,
        )
        .unwrap();
    assert_eq!(state.cache.snapshot(time(1010, 0)).unwrap().len(), 1);
}

#[test]
fn goodbye_from_another_sender_cannot_remove_an_admitted_alias() {
    let sender = "10.23.0.2".parse().unwrap();
    let mut state = intake();
    state
        .apply(
            event("10.23.0.2", "peer", 4433),
            sender,
            time(1010, 0),
            1010,
        )
        .unwrap();
    state
        .apply(
            MdnsEvent::Removed {
                instance: "peer".into(),
            },
            "10.23.0.3".parse().unwrap(),
            time(1010, 0),
            1010,
        )
        .unwrap();
    assert_eq!(state.cache.snapshot(time(1010, 0)).unwrap().len(), 1);
    state
        .apply(
            MdnsEvent::Removed {
                instance: "peer".into(),
            },
            sender,
            time(1010, 0),
            1010,
        )
        .unwrap();
    assert!(state.cache.snapshot(time(1010, 0)).unwrap().is_empty());
    assert!(state.origins.is_empty());
}

#[test]
fn conflicting_live_alias_does_not_change_its_origin_but_expired_alias_can_be_readmitted() {
    let mut state = intake();
    let first = "10.23.0.2".parse().unwrap();
    let second = "10.23.0.3".parse().unwrap();
    state
        .apply(event("10.23.0.2", "peer", 4433), first, time(1010, 0), 1010)
        .unwrap();
    state
        .apply(
            event("10.23.0.3", "peer", 4433),
            second,
            time(1010, 0),
            1010,
        )
        .unwrap();
    assert_eq!(state.origins.get("peer"), Some(&first));
    state
        .apply(
            event("10.23.0.3", "peer", 4433),
            second,
            time(1040, 30),
            1040,
        )
        .unwrap();
    assert_eq!(state.origins.get("peer"), Some(&second));
}

#[test]
fn source_origin_bookkeeping_remains_bounded_after_many_expiring_aliases() {
    let mut state = intake();
    for index in 0..80 {
        let mut next = event("10.23.0.2", &format!("peer-{index}"), 4433);
        if let MdnsEvent::Resolved { ttl, .. } = &mut next {
            *ttl = 1;
        }
        let wall = 1010 + index;
        state
            .apply(next, "10.23.0.2".parse().unwrap(), time(wall, index), wall)
            .unwrap();
        assert!(state.origins.len() <= 1);
    }
}

#[test]
fn cache_event_budget_failure_is_not_a_partial_success() {
    let mut state = intake();
    let sender = "10.23.0.2".parse().unwrap();
    for _ in 0..64 {
        state
            .apply(
                event("10.23.0.2", "peer", 4433),
                sender,
                time(1010, 0),
                1010,
            )
            .unwrap();
    }
    assert!(matches!(
        state.apply(
            event("10.23.0.2", "peer", 4433),
            sender,
            time(1010, 0),
            1010
        ),
        Err(MdnsError::BudgetExceeded)
    ));
}

#[test]
fn goodbye_event_budget_failure_has_the_same_terminal_classification_as_observation() {
    let mut state = intake();
    let sender = "10.23.0.2".parse().unwrap();
    for _ in 0..64 {
        state
            .apply(
                event("10.23.0.2", "peer", 4433),
                sender,
                time(1010, 0),
                1010,
            )
            .unwrap();
    }
    assert!(matches!(
        state.apply(
            MdnsEvent::Removed {
                instance: "peer".into()
            },
            sender,
            time(1010, 0),
            1010
        ),
        Err(MdnsError::BudgetExceeded)
    ));
    assert_eq!(state.origins.get("peer"), Some(&sender));
}
