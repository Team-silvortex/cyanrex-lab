use super::*;

#[test]
fn header_and_all_four_resource_records_have_exact_uncompressed_bytes() {
    let rr = records(&announcement().encode_at(1000).unwrap());
    assert_eq!(rr[0].owner, SERVICE);
    assert_eq!(rr[0].kind, 12);
    assert_eq!(rr[0].class, 1, "the shared PTR must not set cache-flush");
    assert_eq!(rr[0].data, dns_name(INSTANCE));

    assert_eq!(rr[1].owner, INSTANCE);
    assert_eq!(rr[1].kind, 33);
    assert_eq!(rr[1].class, 0x8001);
    let mut expected_srv = vec![0, 0, 0, 0, 0x11, 0x51];
    expected_srv.extend(dns_name(HOST));
    assert_eq!(rr[1].data, expected_srv);

    assert_eq!(rr[2].owner, INSTANCE);
    assert_eq!(rr[2].kind, 16);
    assert_eq!(rr[2].class, 0x8001);
    assert_eq!(
        rr[2].data,
        encode_txt(&advertisement(&raw_advertisement())).unwrap()
    );
    let mut offset = 0;
    let mut fields = Vec::new();
    while offset < rr[2].data.len() {
        let length = rr[2].data[offset] as usize;
        offset += 1;
        fields.push(rr[2].data[offset..offset + length].to_vec());
        offset += length;
    }
    assert_eq!(fields[0], b"v=1");
    assert_eq!(fields[1], format!("c={}", fields.len() - 2).as_bytes());
    let mut f0 = Vec::new();
    for (index, field) in fields[2..].iter().enumerate() {
        assert_eq!(&field[..3], format!("d{index}=").as_bytes());
        assert!((1..=200).contains(&(field.len() - 3)));
        f0.extend_from_slice(&field[3..]);
    }
    assert_eq!(f0, advertisement(&raw_advertisement()).encode().unwrap());

    assert_eq!(rr[3].owner, HOST);
    assert_eq!(rr[3].kind, 1);
    assert_eq!(rr[3].class, 0x8001);
    assert_eq!(rr[3].data, [192, 168, 1, 7]);
    assert!(rr.iter().all(|record| record.ttl == 100));
}

#[test]
fn equal_inputs_produce_stable_complete_bytes_without_compression_or_padding() {
    let first = announcement().encode_at(1000).unwrap();
    assert_eq!(first, announcement().encode_at(1000).unwrap());
    let rr = records(&first);
    let expected_length = 12
        + rr.iter()
            .map(|record| dns_name(&record.owner).len() + 10 + record.data.len())
            .sum::<usize>();
    assert_eq!(first.len(), expected_length);
}

#[test]
fn instance_and_host_are_distinct_labels_not_derived_identity_claims() {
    let packet = MdnsAnnouncement::new(
        "teaching-room",
        "desktop-host",
        endpoint(),
        &advertisement(&raw_advertisement()),
        1000,
        1,
    )
    .unwrap()
    .encode_at(1000)
    .unwrap();
    let rr = records(&packet);
    assert_eq!(rr[0].data, dns_name("teaching-room._cyanrex._udp.local."));
    assert_eq!(rr[1].owner, "teaching-room._cyanrex._udp.local.");
    assert_eq!(rr[3].owner, "desktop-host.local.");
    assert!(!rr[1].owner.contains(&"b".repeat(64)));
}

#[test]
fn encode_retains_all_offered_and_required_capability_claims_without_selection() {
    let mut raw = raw_advertisement();
    raw["offered_capabilities"] = serde_json::json!(["payload.survey", "peer.identity"]);
    raw["required_capabilities"] = serde_json::json!(["peer.identity"]);
    let ad = advertisement(&raw);
    let prepared =
        MdnsAnnouncement::new("node-one", "host-one", endpoint(), &ad, 1000, 120).unwrap();
    let rr = records(&prepared.encode_at(1000).unwrap());
    assert_eq!(rr[2].data, encode_txt(&ad).unwrap());
}

#[test]
fn preparation_owns_frozen_advertisement_bytes_after_input_is_dropped() {
    let prepared = {
        let ad = advertisement(&raw_advertisement());
        MdnsAnnouncement::new("node-one", "host-one", endpoint(), &ad, 1000, 120).unwrap()
    };
    let rr = records(&prepared.encode_at(1000).unwrap());
    assert_eq!(
        rr[2].data,
        encode_txt(&advertisement(&raw_advertisement())).unwrap()
    );
}

#[test]
fn goodbye_preserves_every_frozen_owner_type_class_and_rdata_with_only_zero_ttls() {
    let prepared = announcement();
    let live = records(&prepared.encode_at(1000).unwrap());
    let goodbye = prepared.encode_goodbye().unwrap();
    let withdrawn = records(&goodbye);
    assert_eq!(live.len(), withdrawn.len());
    for (mut before, after) in live.into_iter().zip(withdrawn) {
        assert_eq!(after.ttl, 0);
        before.ttl = 0;
        assert_eq!(before, after);
    }
    let parsed = parse_response(&goodbye).unwrap();
    assert_eq!(parsed.len(), 1);
    assert!(matches!(&parsed[0], MdnsEvent::Removed { instance } if instance == "node-one"));
}

#[test]
fn packet_to_f1a_is_only_an_unverified_claim_with_separate_cache_lifetime() {
    let packet = announcement().encode_at(1000).unwrap();
    let event = parse_response(&packet).unwrap().remove(0);
    let MdnsEvent::Resolved {
        instance,
        advertisement: bytes,
        ttl,
        ..
    } = event
    else {
        panic!("one complete unverified event required")
    };
    let policy = DiscoveryPolicy::new(
        "a".repeat(64).parse().unwrap(),
        ProtocolRange::new(1, 3).unwrap(),
        vec!["peer.identity".parse().unwrap()],
        vec![],
    )
    .unwrap();
    let mut cache = LanCandidateCache::new(policy, vec![17], 1).unwrap();
    let source = LanSource::new(17, &instance).unwrap();
    assert_eq!(
        cache.observe(&source, &bytes, ttl, CacheTime::new(1000, 2).unwrap()),
        Ok(LanObservation::Added)
    );
    let snapshot = cache.snapshot(CacheTime::new(1001, 3).unwrap()).unwrap();
    assert_eq!(snapshot.len(), 1);
    let candidate = snapshot[0].candidate();
    assert_eq!(candidate.claimed_node().as_str(), "b".repeat(64));
    assert_eq!(
        candidate.claimed_authority().to_string(),
        "8b075b0b-8118-4c37-9224-337ba22f40c6"
    );
    assert_eq!(candidate.protocol(), 3);
    assert_eq!(candidate.expires_at(), 1100);
    assert_eq!(snapshot[0].deadline_monotonic(), 102);
    assert!(cache
        .snapshot(CacheTime::new(1100, 102).unwrap())
        .unwrap()
        .is_empty());
}
