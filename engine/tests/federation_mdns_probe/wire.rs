use super::*;

#[test]
fn probe_has_two_any_qu_questions_and_three_in_authority_records_without_ptr() {
    let bytes = announcement().encode_probe_at(1000).unwrap();
    let rr = probe_records(&bytes);
    assert_eq!(rr.iter().map(|rr| rr.kind).collect::<Vec<_>>(), [33, 16, 1]);
    assert_eq!(
        rr.iter().map(|rr| rr.owner.as_str()).collect::<Vec<_>>(),
        [INSTANCE, INSTANCE, HOST]
    );
    assert!(rr.iter().all(|rr| rr.class == 1 && rr.ttl == 100));
    assert!(!rr.iter().any(|rr| rr.kind == 12));
}

#[test]
fn probe_rdata_is_the_complete_frozen_srv_txt_and_ipv4_not_selected_capabilities() {
    let rr = probe_records(&announcement().encode_probe_at(1000).unwrap());
    let mut expected_srv = vec![0, 0, 0, 0, 0x11, 0x51];
    expected_srv.extend(dns_name(HOST));
    assert_eq!(rr[0].data, expected_srv);
    assert_eq!(rr[1].data, encode_txt(&advertisement()).unwrap());
    assert_eq!(rr[2].data, [192, 168, 1, 7]);
}

#[test]
fn probe_at_uses_the_same_frozen_rdata_as_live_announcement() {
    let prepared = announcement();
    let bytes = prepared.encode_at(1000).unwrap();
    let mut offset = 12;
    let mut records = Vec::new();
    for _ in 0..4 {
        let owner = read_name(&bytes, &mut offset);
        let kind = word(&bytes, offset);
        let class = word(&bytes, offset + 2) & 0x7fff;
        let ttl = u32::from_be_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
        let length = word(&bytes, offset + 8) as usize;
        offset += 10;
        let data = bytes[offset..offset + length].to_vec();
        offset += length;
        if kind != 12 {
            records.push(Record {
                owner,
                kind,
                class,
                ttl,
                data,
            });
        }
    }
    assert_eq!(
        probe_records(&prepared.encode_probe_at(1000).unwrap()),
        records
    );
}

#[test]
fn probe_ttls_shrink_by_fresh_advertisement_remaining_time_without_changing_data() {
    let prepared = announcement();
    let early = probe_records(&prepared.encode_probe_at(1000).unwrap());
    let late = probe_records(&prepared.encode_probe_at(1099).unwrap());
    for (mut before, after) in early.into_iter().zip(late) {
        assert_eq!(before.ttl, 100);
        assert_eq!(after.ttl, 1);
        before.ttl = 1;
        assert_eq!(before, after);
    }
}

#[test]
fn probe_ttls_respect_the_requested_cap_not_the_maximum_advertisement_lifetime() {
    let prepared = MdnsAnnouncement::new(
        "node-one",
        "host-one",
        SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 7), 4433),
        &advertisement(),
        1000,
        7,
    )
    .unwrap();
    for (time, ttl) in [(1000, 7), (1098, 2), (1099, 1)] {
        assert!(probe_records(&prepared.encode_probe_at(time).unwrap())
            .iter()
            .all(|rr| rr.ttl == ttl));
    }
}

#[test]
fn probe_rejects_time_before_preparation_expiry_and_non_json_safe_times() {
    let prepared = announcement();
    for now in [0, 999, 1100, 1101, 9_007_199_254_740_992, u64::MAX] {
        assert!(prepared.encode_probe_at(now).is_err(), "invalid time {now}");
    }
    assert_eq!(prepared.encode_probe_at(999), Err(MdnsError::ClockInvalid));
}

#[test]
fn repeated_encoding_does_not_claim_a_rollback_watermark_or_renew_f0() {
    let prepared = announcement();
    let first = prepared.encode_probe_at(1000).unwrap();
    assert_eq!(first, prepared.encode_probe_at(1000).unwrap());
    assert!(probe_records(&prepared.encode_probe_at(1099).unwrap())
        .iter()
        .all(|rr| rr.ttl == 1));
    assert!(probe_records(&prepared.encode_probe_at(1001).unwrap())
        .iter()
        .all(|rr| rr.ttl == 99));
    assert_eq!(first, prepared.encode_probe_at(1000).unwrap());
    assert!(prepared.encode_probe_at(1100).is_err());
}

#[test]
fn observing_identical_goodbye_is_still_only_an_existing_claim_not_name_availability() {
    let prepared = announcement();
    assert_eq!(
        prepared.observe_probe_packet(&prepared.encode_goodbye().unwrap()),
        Ok(ProbeObservation::ExistingResponse)
    );
}
