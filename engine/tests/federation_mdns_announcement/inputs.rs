use super::*;

#[test]
fn canonical_single_label_minimum_and_maximum_lengths_are_not_normalized() {
    for label in ["a".into(), "7".into(), "a-7".into(), "z".repeat(63)] {
        let prepared = MdnsAnnouncement::new(
            &label,
            &label,
            endpoint(),
            &advertisement(&raw_advertisement()),
            1000,
            1,
        )
        .unwrap();
        let rr = records(&prepared.encode_at(1000).unwrap());
        assert_eq!(rr[1].owner, format!("{label}.{SERVICE}"));
        assert_eq!(rr[3].owner, format!("{label}.local."));
    }
}

#[test]
fn instance_and_host_reject_noncanonical_or_multiple_labels_independently() {
    for invalid in [
        String::new(),
        "a".repeat(64),
        "-a".into(),
        "a-".into(),
        "A".into(),
        "node.local.".into(),
        "node.one".into(),
        "node_one".into(),
        "노드".into(),
        "node\0secret".into(),
        "node\nsecret".into(),
        "node secret".into(),
        " node".into(),
        "node ".into(),
    ] {
        let ad = advertisement(&raw_advertisement());
        assert!(MdnsAnnouncement::new(&invalid, "host-one", endpoint(), &ad, 1000, 1).is_err());
        assert!(MdnsAnnouncement::new("node-one", &invalid, endpoint(), &ad, 1000, 1).is_err());
    }
}

#[test]
fn matching_single_hint_accepts_all_three_rfc1918_ranges_only_as_claims() {
    for ip in [
        Ipv4Addr::new(10, 0, 0, 1),
        Ipv4Addr::new(172, 16, 0, 1),
        Ipv4Addr::new(172, 31, 255, 1),
        Ipv4Addr::new(192, 168, 255, 1),
    ] {
        let endpoint = SocketAddrV4::new(ip, 65535);
        let mut raw = raw_advertisement();
        raw["hints"][0]["address"] = endpoint.to_string().into();
        let prepared = MdnsAnnouncement::new(
            "node-one",
            "host-one",
            endpoint,
            &advertisement(&raw),
            1000,
            1,
        )
        .unwrap();
        let rr = records(&prepared.encode_at(1000).unwrap());
        assert_eq!(rr[3].data, ip.octets());
        assert_eq!(word(&rr[1].data, 4), 65535);
    }
}

#[test]
fn non_rfc1918_endpoint_and_zero_port_are_rejected_without_interface_lookup() {
    for ip in [
        Ipv4Addr::UNSPECIFIED,
        Ipv4Addr::LOCALHOST,
        Ipv4Addr::BROADCAST,
        Ipv4Addr::new(8, 8, 8, 8),
        Ipv4Addr::new(100, 64, 0, 1),
        Ipv4Addr::new(169, 254, 1, 1),
        Ipv4Addr::new(172, 15, 255, 1),
        Ipv4Addr::new(172, 32, 0, 1),
        Ipv4Addr::new(192, 167, 1, 1),
        Ipv4Addr::new(224, 0, 0, 251),
    ] {
        assert!(MdnsAnnouncement::new(
            "node-one",
            "host-one",
            SocketAddrV4::new(ip, 4433),
            &advertisement(&raw_advertisement()),
            1000,
            1
        )
        .is_err());
    }
    assert!(MdnsAnnouncement::new(
        "node-one",
        "host-one",
        SocketAddrV4::new(*endpoint().ip(), 0),
        &advertisement(&raw_advertisement()),
        1000,
        1
    )
    .is_err());
}

#[test]
fn a_matching_public_hint_cannot_turn_public_endpoint_into_lan_publication() {
    let endpoint = SocketAddrV4::new(Ipv4Addr::new(8, 8, 8, 8), 4433);
    let mut raw = raw_advertisement();
    raw["hints"][0]["address"] = endpoint.to_string().into();
    assert!(MdnsAnnouncement::new(
        "node-one",
        "host-one",
        endpoint,
        &advertisement(&raw),
        1000,
        1
    )
    .is_err());
}

#[test]
fn hint_must_be_one_exact_ipv4_endpoint_not_an_extra_or_locally_selected_subset() {
    for hints in [
        serde_json::json!([{"kind":"direct","address":"192.168.1.8:4433"}]),
        serde_json::json!([{"kind":"direct","address":"192.168.1.7:4434"}]),
        serde_json::json!([{"kind":"direct","address":"[fc00::7]:4433"}]),
        serde_json::json!([{"kind":"relay","relay_id":"relay.local"}]),
        serde_json::json!([
            {"kind":"direct","address":"192.168.1.7:4433"},
            {"kind":"direct","address":"192.168.1.8:4433"}
        ]),
        serde_json::json!([
            {"kind":"direct","address":"192.168.1.7:4433"},
            {"kind":"direct","address":"8.8.8.8:4433"}
        ]),
        serde_json::json!([
            {"kind":"direct","address":"192.168.1.7:4433"},
            {"kind":"relay","relay_id":"relay.local"}
        ]),
    ] {
        let mut raw = raw_advertisement();
        raw["hints"] = hints;
        assert!(MdnsAnnouncement::new(
            "node-one",
            "host-one",
            endpoint(),
            &advertisement(&raw),
            1000,
            1
        )
        .is_err());
    }
}

#[test]
fn valid_f0_above_the_txt_budget_fails_before_any_partial_packet_is_observable() {
    let mut raw = raw_advertisement();
    raw["offered_capabilities"] = serde_json::json!((0..16)
        .map(|index| format!("peer.c{index}{}", "a".repeat(95)))
        .collect::<Vec<_>>());
    let ad = advertisement(&raw);
    assert!(ad.encode().unwrap().len() > 1200);
    assert!(MdnsAnnouncement::new("node-one", "host-one", endpoint(), &ad, 1000, 1).is_err());
}

fn advertisement_with_canonical_length(length: usize) -> PeerAdvertisement {
    let mut raw = raw_advertisement();
    let mut capabilities: Vec<String> = (0..16).map(|index| format!("peer.c{index}")).collect();
    raw["offered_capabilities"] = serde_json::json!(&capabilities);
    let mut remaining = length - advertisement(&raw).encode().unwrap().len();
    for capability in &mut capabilities {
        let padding = remaining.min(95);
        capability.push_str(&"a".repeat(padding));
        remaining -= padding;
    }
    assert_eq!(remaining, 0);
    raw["offered_capabilities"] = serde_json::json!(capabilities);
    let ad = advertisement(&raw);
    assert_eq!(ad.encode().unwrap().len(), length);
    ad
}

#[test]
fn maximum_labels_and_exact_txt_budget_remain_one_complete_bounded_packet() {
    let ad = advertisement_with_canonical_length(1168);
    assert_eq!(encode_txt(&ad).unwrap().len(), 1200);
    let prepared =
        MdnsAnnouncement::new(&"a".repeat(63), &"z".repeat(63), endpoint(), &ad, 1000, 120)
            .unwrap();
    let packet = prepared.encode_at(1000).unwrap();
    assert_eq!(packet.len(), 1680);
    assert_eq!(records(&packet)[2].data.len(), 1200);
    assert!(matches!(
        &parse_response(&packet).unwrap()[0],
        MdnsEvent::Resolved { advertisement, .. } if *advertisement == ad.encode().unwrap()
    ));
}

#[test]
fn one_byte_beyond_txt_budget_is_not_trimmed_even_though_f0_itself_is_valid() {
    let ad = advertisement_with_canonical_length(1169);
    assert!(encode_txt(&ad).is_err());
    assert!(MdnsAnnouncement::new("node-one", "host-one", endpoint(), &ad, 1000, 1).is_err());
}

#[test]
fn errors_never_echo_untrusted_labels_or_fingerprint_claims() {
    let secret = "private-cookie-private-token\n";
    let result = MdnsAnnouncement::new(
        secret,
        "host-one",
        endpoint(),
        &advertisement(&raw_advertisement()),
        1000,
        1,
    );
    let error = match result {
        Ok(_) => panic!("invalid label must fail"),
        Err(error) => error,
    };
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains("private-cookie"));
    assert!(!rendered.contains("private-token"));
    assert!(!rendered.contains(&"b".repeat(64)));
}
