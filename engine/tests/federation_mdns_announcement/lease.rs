use super::*;

const MAX_SAFE_TIME: u64 = 9_007_199_254_740_991;

fn ttls(prepared: &MdnsAnnouncement, now: u64) -> Vec<u32> {
    records(&prepared.encode_at(now).unwrap())
        .into_iter()
        .map(|record| record.ttl)
        .collect()
}

#[test]
fn requested_ttl_is_bounded_at_construction_not_silently_coerced() {
    let ad = advertisement(&raw_advertisement());
    for ttl in [0, 121, u32::MAX] {
        assert!(MdnsAnnouncement::new("node-one", "host-one", endpoint(), &ad, 1000, ttl).is_err());
    }
    for ttl in [1, 120] {
        let prepared =
            MdnsAnnouncement::new("node-one", "host-one", endpoint(), &ad, 1000, ttl).unwrap();
        assert_eq!(ttls(&prepared, 1000), vec![ttl.min(100); 4]);
    }
}

#[test]
fn all_record_ttls_are_capped_by_requested_ttl_and_current_remaining_f0_lease() {
    let mut raw = raw_advertisement();
    raw["expires_at"] = 1300.into();
    let prepared = MdnsAnnouncement::new(
        "node-one",
        "host-one",
        endpoint(),
        &advertisement(&raw),
        1000,
        120,
    )
    .unwrap();
    assert_eq!(ttls(&prepared, 1000), vec![120; 4]);
    assert_eq!(ttls(&prepared, 1180), vec![120; 4]);
    assert_eq!(ttls(&prepared, 1299), vec![1; 4]);
}

#[test]
fn construction_requires_strict_current_issued_time_without_five_second_future_skew() {
    let ad = advertisement(&raw_advertisement());
    assert!(MdnsAnnouncement::new("node-one", "host-one", endpoint(), &ad, 999, 1).is_err());
    assert!(MdnsAnnouncement::new("node-one", "host-one", endpoint(), &ad, 995, 1).is_err());
    let prepared = MdnsAnnouncement::new("node-one", "host-one", endpoint(), &ad, 1000, 1).unwrap();
    assert_eq!(ttls(&prepared, 1000), vec![1; 4]);
}

#[test]
fn zero_unsafe_or_expired_preparation_times_do_not_make_a_live_announcement() {
    let ad = advertisement(&raw_advertisement());
    for now in [0, 1100, 1101, MAX_SAFE_TIME + 1, u64::MAX] {
        assert!(MdnsAnnouncement::new("node-one", "host-one", endpoint(), &ad, now, 1).is_err());
    }
}

#[test]
fn json_safe_time_boundary_and_one_second_remaining_lease_are_accepted_exactly() {
    let mut raw = raw_advertisement();
    raw["issued_at"] = (MAX_SAFE_TIME - 1).into();
    raw["expires_at"] = MAX_SAFE_TIME.into();
    let prepared = MdnsAnnouncement::new(
        "node-one",
        "host-one",
        endpoint(),
        &advertisement(&raw),
        MAX_SAFE_TIME - 1,
        120,
    )
    .unwrap();
    assert_eq!(ttls(&prepared, MAX_SAFE_TIME - 1), vec![1; 4]);
    assert!(prepared.encode_at(MAX_SAFE_TIME).is_err());
}

#[test]
fn encode_rejects_before_preparation_or_expired_times_instead_of_reusing_old_ttl() {
    let prepared = announcement();
    for now in [0, 999, 1100, 1101, MAX_SAFE_TIME + 1, u64::MAX] {
        assert!(prepared.encode_at(now).is_err());
    }
    assert_eq!(ttls(&prepared, 1099), vec![1; 4]);
}

#[test]
fn later_encode_shrinks_ttl_without_mutating_frozen_f0_issued_or_expiry_claims() {
    let prepared = announcement();
    let early = records(&prepared.encode_at(1000).unwrap());
    let late = records(&prepared.encode_at(1099).unwrap());
    for (before, after) in early.iter().zip(&late) {
        assert_eq!(before.owner, after.owner);
        assert_eq!(before.kind, after.kind);
        assert_eq!(before.class, after.class);
        assert_eq!(before.data, after.data);
        assert_eq!(before.ttl, 100);
        assert_eq!(after.ttl, 1);
    }
    assert_eq!(
        late[2].data,
        encode_txt(&advertisement(&raw_advertisement())).unwrap()
    );
}

#[test]
fn encode_has_only_a_prepared_time_floor_not_a_continuous_rollback_watermark() {
    let prepared = announcement();
    assert_eq!(ttls(&prepared, 1099), vec![1; 4]);
    assert_eq!(ttls(&prepared, 1001), vec![99; 4]);
    assert!(prepared.encode_at(999).is_err());
}

#[test]
fn goodbye_is_time_independent_even_after_live_encoding_rejects_expiry() {
    let prepared = announcement();
    let first = prepared.encode_goodbye().unwrap();
    assert!(prepared.encode_at(1100).is_err());
    assert!(prepared.encode_at(u64::MAX).is_err());
    assert_eq!(prepared.encode_goodbye().unwrap(), first);
    assert_eq!(
        records(&first)
            .iter()
            .map(|record| record.ttl)
            .collect::<Vec<_>>(),
        [0; 4]
    );
}
