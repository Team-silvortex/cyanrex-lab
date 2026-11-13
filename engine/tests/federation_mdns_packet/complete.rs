use super::*;

#[test]
fn complete_single_packet_resolves_only_the_fixed_service_and_canonical_f0_bytes() {
    let events = parse_response(&packet(&records())).unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        MdnsEvent::Resolved {
            instance,
            advertisement,
            addresses,
            port,
            ttl,
        } => {
            assert_eq!(instance, "node-one");
            assert_eq!(*port, 4433);
            assert_eq!(*ttl, 70);
            assert_eq!(addresses, &[Ipv4Addr::new(192, 168, 1, 7)]);
            let peer = PeerAdvertisement::parse(advertisement).unwrap();
            assert_eq!(peer.encode().unwrap(), *advertisement);
        }
        _ => panic!("complete live records must resolve"),
    }
}

#[test]
fn compressed_names_follow_only_bounded_prior_occurrences() {
    let plain = parse_response(&packet(&records())).unwrap();
    let packed = parse_response(&compressed()).unwrap();
    assert_eq!(format!("{plain:?}"), format!("{packed:?}"));
}

#[test]
fn ascii_dns_case_and_cache_flush_bit_do_not_change_service_identity() {
    let mut rr = records();
    for record in &mut rr {
        record.owner.make_ascii_uppercase();
        record.class = 0x8001;
    }
    rr[0].data = dns_name("NODE-ONE._CYANREX._UDP.LOCAL.");
    rr[1].data = srv("NODE-ONE.LOCAL.", 4433);
    let events = parse_response(&packet(&rr)).unwrap();
    assert!(matches!(&events[0], MdnsEvent::Resolved { instance, .. } if instance == "node-one"));
}

#[test]
fn records_may_arrive_in_any_section_or_order_within_one_packet() {
    let mut rr = records();
    rr.reverse();
    let mut bytes = packet(&rr);
    bytes[6..8].copy_from_slice(&1_u16.to_be_bytes());
    bytes[8..10].copy_from_slice(&1_u16.to_be_bytes());
    bytes[10..12].copy_from_slice(&2_u16.to_be_bytes());
    assert!(matches!(
        &parse_response(&bytes).unwrap()[0],
        MdnsEvent::Resolved { ttl: 70, .. }
    ));
}

#[test]
fn eight_bounded_questions_do_not_replace_the_complete_response_chain() {
    let original = packet(&records());
    let mut bytes = original[..12].to_vec();
    bytes[4..6].copy_from_slice(&8_u16.to_be_bytes());
    for _ in 0..8 {
        bytes.extend(dns_name(SERVICE));
        bytes.extend([0, 12, 0, 1]);
    }
    bytes.extend_from_slice(&original[12..]);
    assert_eq!(parse_response(&bytes).unwrap().len(), 1);
}

#[test]
fn sixty_four_total_records_are_allowed_but_one_more_is_not_ignored() {
    let mut rr = records();
    for _ in 4..MAX_MDNS_RECORDS {
        rr.push(Record {
            owner: "other.local.".into(),
            kind: 65000,
            class: 1,
            ttl: 30,
            data: vec![],
        });
    }
    assert_eq!(parse_response(&packet(&rr)).unwrap().len(), 1);
    rr.push(rr.last().unwrap().clone());
    assert_invalid(&packet(&rr));
}

#[test]
fn incomplete_packets_do_not_assemble_candidates_across_calls() {
    let original = records();
    for missing in 0..original.len() {
        let mut rr = original.clone();
        rr.remove(missing);
        assert!(parse_response(&packet(&rr)).unwrap().is_empty());
    }
    assert!(parse_response(&packet(&original[..2])).unwrap().is_empty());
    assert!(parse_response(&packet(&original[2..])).unwrap().is_empty());
}

#[test]
fn unrelated_service_records_are_bounded_skipped_not_profile_validated() {
    let mut rr = records();
    for kind in [1, 12, 16, 33, 65000] {
        rr.push(Record {
            owner: "printer._ipp._tcp.local.".into(),
            kind,
            class: 1,
            ttl: 60,
            data: vec![0xff, 0, 0xff],
        });
    }
    assert_eq!(parse_response(&packet(&rr)).unwrap().len(), 1);
}

#[test]
fn non_in_records_are_ignored_after_bounds_without_producing_partial_candidates() {
    for index in 0..4 {
        let mut rr = records();
        rr[index].class = 3;
        assert!(parse_response(&packet(&rr)).unwrap().is_empty());
    }
}

#[test]
fn same_name_non_in_shadow_records_cannot_poison_the_complete_in_chain() {
    let mut rr = records();
    for original in records() {
        let mut shadow = original;
        shadow.class = 0x8003;
        shadow.data = vec![0xff, 0];
        rr.push(shadow);
    }
    assert_eq!(parse_response(&packet(&rr)).unwrap().len(), 1);
}

#[test]
fn separate_instances_may_share_one_host_address_without_becoming_duplicates() {
    let mut rr = records();
    let second = "node-two._cyanrex._udp.local.";
    let mut ptr = rr[0].clone();
    ptr.data = dns_name(second);
    rr.push(ptr);
    for original in [rr[1].clone(), rr[2].clone()] {
        let mut record = original;
        record.owner = second.into();
        rr.push(record);
    }
    let events = parse_response(&packet(&rr)).unwrap();
    assert_eq!(events.len(), 2);
    for (event, expected) in events.iter().zip(["node-one", "node-two"]) {
        assert!(
            matches!(event, MdnsEvent::Resolved { instance, addresses, .. }
            if instance == expected && addresses == &[Ipv4Addr::new(192,168,1,7)])
        );
    }
    rr.push(rr.last().unwrap().clone());
    assert_invalid(&packet(&rr));
}

#[test]
fn ttl_is_the_minimum_necessary_record_value_capped_to_the_lan_bound() {
    for index in 0..4 {
        let mut rr = records();
        for record in &mut rr {
            record.ttl = 999;
        }
        rr[index].ttl = 7;
        assert!(matches!(
            &parse_response(&packet(&rr)).unwrap()[0],
            MdnsEvent::Resolved { ttl: 7, .. }
        ));
    }
    let mut rr = records();
    for record in &mut rr {
        record.ttl = u32::MAX;
    }
    assert!(matches!(
        &parse_response(&packet(&rr)).unwrap()[0],
        MdnsEvent::Resolved { ttl: 120, .. }
    ));
}

#[test]
fn a_zero_ttl_in_any_necessary_record_removes_only_that_instance() {
    for index in 0..4 {
        let mut rr = records();
        rr[index].ttl = 0;
        assert!(
            matches!(&parse_response(&packet(&rr)).unwrap()[0], MdnsEvent::Removed { instance } if instance == "node-one")
        );
    }
}

#[test]
fn bare_ptr_goodbye_is_a_precise_remove_not_a_live_or_authorized_candidate() {
    let mut goodbye = records()[0].clone();
    goodbye.ttl = 0;
    let events = parse_response(&packet(&[goodbye])).unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(&events[0], MdnsEvent::Removed { instance } if instance == "node-one"));
}

#[test]
fn up_to_eight_unique_addresses_keep_packet_order_and_minimum_ttl() {
    let mut rr = records();
    for index in 8..=14 {
        let mut a = rr[3].clone();
        a.data[3] = index;
        a.ttl = 9;
        rr.push(a);
    }
    match &parse_response(&packet(&rr)).unwrap()[0] {
        MdnsEvent::Resolved { addresses, ttl, .. } => {
            assert_eq!(addresses.len(), 8);
            assert_eq!(*ttl, 9);
            assert_eq!(addresses[0], Ipv4Addr::new(192, 168, 1, 7));
            assert_eq!(addresses[7], Ipv4Addr::new(192, 168, 1, 14));
        }
        _ => panic!("complete set must resolve"),
    }
}

#[test]
fn eight_events_are_bounded_and_names_do_not_claim_node_fingerprints() {
    let mut rr = Vec::new();
    for index in 0..8 {
        let mut ptr = records()[0].clone();
        ptr.ttl = 0;
        ptr.data = dns_name(&format!("node-{index}.{SERVICE}"));
        rr.push(ptr);
    }
    let events = parse_response(&packet(&rr)).unwrap();
    assert_eq!(events.len(), 8);
    assert!(matches!(&events[7], MdnsEvent::Removed { instance } if instance == "node-7"));
    let mut ninth = rr[0].clone();
    ninth.data = dns_name(&format!("node-8.{SERVICE}"));
    rr.push(ninth);
    assert_invalid(&packet(&rr));
}

#[test]
fn unknown_records_fill_the_exact_packet_budget_without_trailing_garbage() {
    let mut rr = records();
    rr.push(Record {
        owner: "other.local.".into(),
        kind: 65000,
        class: 1,
        ttl: 60,
        data: vec![],
    });
    let padding = MAX_MDNS_PACKET_BYTES - packet(&rr).len();
    rr.last_mut().unwrap().data.resize(padding, 0xff);
    let exact = packet(&rr);
    assert_eq!(exact.len(), MAX_MDNS_PACKET_BYTES);
    assert_eq!(parse_response(&exact).unwrap().len(), 1);
    let mut oversized = exact;
    oversized.push(0);
    assert!(matches!(
        parse_response(&oversized),
        Err(MdnsError::TooLarge)
    ));
}
