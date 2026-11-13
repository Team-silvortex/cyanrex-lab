use super::*;

#[test]
fn caller_supplied_time_is_positive_json_safe_wall_time_and_opaque_monotonic_ticks() {
    for unix in [0, 9_007_199_254_740_992, u64::MAX] {
        assert!(matches!(
            CacheTime::new(unix, 200),
            Err(LanDiscoveryError::InvalidTime)
        ));
    }
    assert!(CacheTime::new(1, 0).is_ok());
    assert!(CacheTime::new(9_007_199_254_740_991, u64::MAX).is_ok());
}

#[test]
fn ttl_is_nonzero_bounded_and_does_not_translate_goodbye_into_implicit_removal() {
    let mut cache = configured(1);
    let local = source(11, "node-one");
    add(&mut cache, &local);
    for ttl in [0, MAX_LAN_TTL_SECONDS + 1, u32::MAX] {
        assert!(matches!(
            cache.observe(&local, &raw(&advertisement()), ttl, time(1_011, 201)),
            Err(LanDiscoveryError::InvalidTtl)
        ));
    }
    assert_eq!(
        cache.snapshot(time(1_011, 201)).unwrap()[0].deadline_monotonic(),
        230
    );
}

#[test]
fn monotonic_ttl_expiry_is_strict_even_when_the_wall_clock_does_not_advance() {
    let mut cache = configured(1);
    add(&mut cache, &source(11, "node-one"));
    assert_eq!(cache.snapshot(time(1_010, 229)).unwrap().len(), 1);
    assert!(cache.snapshot(time(1_010, 230)).unwrap().is_empty());
}

#[test]
fn wall_ttl_expiry_is_strict_even_when_monotonic_elapsed_is_shorter() {
    let mut cache = configured(1);
    add(&mut cache, &source(11, "node-one"));
    assert_eq!(cache.snapshot(time(1_039, 210)).unwrap().len(), 1);
    assert!(cache.snapshot(time(1_040, 210)).unwrap().is_empty());
}

#[test]
fn advertisement_lease_bounds_a_longer_initial_dns_record_ttl() {
    let mut cache = configured(1);
    let local = source(11, "node-one");
    assert!(matches!(
        cache.observe(
            &local,
            &raw(&advertisement()),
            MAX_LAN_TTL_SECONDS,
            time(1_010, 200)
        ),
        Ok(LanObservation::Added)
    ));
    assert_eq!(
        cache.snapshot(time(1_010, 200)).unwrap()[0].deadline_monotonic(),
        290
    );
    assert_eq!(cache.snapshot(time(1_099, 289)).unwrap().len(), 1);
    assert!(cache.snapshot(time(1_100, 289)).unwrap().is_empty());
}

#[test]
fn lease_monotonic_deadline_also_expires_if_wall_time_stalls() {
    let mut cache = configured(1);
    assert!(matches!(
        cache.observe(
            &source(11, "node-one"),
            &raw(&advertisement()),
            120,
            time(1_010, 200)
        ),
        Ok(LanObservation::Added)
    ));
    assert!(cache.snapshot(time(1_010, 290)).unwrap().is_empty());
}

#[test]
fn repeated_broadcasts_cannot_refresh_initial_ttl_or_resurrect_an_expired_lease() {
    let mut cache = configured(1);
    let local = source(11, "node-one");
    add(&mut cache, &local);
    for (unix, mono) in [(1_011, 201), (1_020, 210), (1_029, 229)] {
        assert!(matches!(
            cache.observe(&local, &raw(&advertisement()), 120, time(unix, mono)),
            Ok(LanObservation::Duplicate)
        ));
        assert_eq!(
            cache.snapshot(time(unix, mono)).unwrap()[0].deadline_monotonic(),
            230
        );
    }
    assert!(cache.snapshot(time(1_030, 230)).unwrap().is_empty());
    assert!(matches!(
        cache.observe(&local, &raw(&advertisement()), 120, time(1_100, 300)),
        Err(LanDiscoveryError::RejectedAdvertisement(
            DiscoveryError::Expired
        ))
    ));
    assert!(cache.snapshot(time(1_100, 300)).unwrap().is_empty());
}

#[test]
fn stale_future_self_or_mandatory_capability_advertisements_are_still_f0_rejections() {
    for (field, replacement, expected) in [
        ("expires_at", json!(1_010), DiscoveryError::Expired),
        ("issued_at", json!(1_016), DiscoveryError::InvalidTime),
        (
            "node_fingerprint",
            json!("a".repeat(64)),
            DiscoveryError::SelfAdvertisement,
        ),
        (
            "offered_capabilities",
            json!(["remote.only"]),
            DiscoveryError::InvalidAdvertisement,
        ),
    ] {
        let mut cache = configured(1);
        let mut value = advertisement();
        value[field] = replacement;
        let error = match cache.observe(&source(11, "node-one"), &raw(&value), 30, time(1_010, 200))
        {
            Err(LanDiscoveryError::RejectedAdvertisement(error)) => error,
            _ => panic!("F0 rejection must be preserved"),
        };
        assert_eq!(error, expected);
        assert!(cache.snapshot(time(1_010, 200)).unwrap().is_empty());
    }
}

#[test]
fn observing_a_wall_clock_rollback_clears_and_permanently_closes_the_cache() {
    let mut cache = configured(1);
    let local = source(11, "node-one");
    add(&mut cache, &local);
    assert!(matches!(
        cache.observe(&local, &raw(&advertisement()), 30, time(1_009, 201)),
        Err(LanDiscoveryError::ClockRollback)
    ));
    assert!(matches!(
        cache.snapshot(time(1_011, 202)),
        Err(LanDiscoveryError::Closed)
    ));
    assert!(matches!(
        cache.remove(&local, time(1_011, 202)),
        Err(LanDiscoveryError::Closed)
    ));
}

#[test]
fn snapshot_monotonic_rollback_cannot_return_or_revive_old_candidates() {
    let mut cache = configured(1);
    add(&mut cache, &source(11, "node-one"));
    assert!(matches!(
        cache.snapshot(time(1_011, 199)),
        Err(LanDiscoveryError::ClockRollback)
    ));
    assert!(matches!(
        cache.snapshot(time(1_012, 202)),
        Err(LanDiscoveryError::Closed)
    ));
}

#[test]
fn removal_clock_rollback_cannot_be_used_as_an_ordinary_source_delete() {
    let mut cache = configured(1);
    let local = source(11, "node-one");
    add(&mut cache, &local);
    assert!(matches!(
        cache.remove(&local, time(1_009, 200)),
        Err(LanDiscoveryError::ClockRollback)
    ));
    assert!(matches!(
        cache.observe(&local, &raw(&advertisement()), 30, time(1_011, 201)),
        Err(LanDiscoveryError::Closed)
    ));
}

#[test]
fn snapshot_time_watermark_applies_to_later_observation_and_removal() {
    let mut cache = configured(1);
    let local = source(11, "node-one");
    add(&mut cache, &local);
    assert_eq!(cache.snapshot(time(1_020, 210)).unwrap().len(), 1);
    assert!(matches!(
        cache.remove(&local, time(1_019, 211)),
        Err(LanDiscoveryError::ClockRollback)
    ));
    assert!(matches!(
        cache.snapshot(time(1_021, 212)),
        Err(LanDiscoveryError::Closed)
    ));
}

#[test]
fn deadline_overflow_is_not_an_insertion_or_permission_to_roll_back_observed_time() {
    let mut cache = configured(2);
    add(&mut cache, &source(11, "node-one"));
    assert!(matches!(
        cache.observe(
            &source(11, "node-two"),
            &raw(&advertisement()),
            30,
            time(1_011, u64::MAX)
        ),
        Err(LanDiscoveryError::InvalidTime)
    ));
    assert!(cache.snapshot(time(1_011, u64::MAX)).unwrap().is_empty());
    assert!(matches!(
        cache.snapshot(time(1_011, 201)),
        Err(LanDiscoveryError::ClockRollback)
    ));
    assert!(matches!(
        cache.snapshot(time(1_012, u64::MAX)),
        Err(LanDiscoveryError::Closed)
    ));
}

#[test]
fn an_unconstructible_time_sample_cannot_mutate_an_existing_cache() {
    let mut cache = configured(1);
    add(&mut cache, &source(11, "node-one"));
    for unix in [0, 9_007_199_254_740_992, u64::MAX] {
        assert!(matches!(
            CacheTime::new(unix, u64::MAX),
            Err(LanDiscoveryError::InvalidTime)
        ));
    }
    assert_eq!(
        cache.snapshot(time(1_011, 201)).unwrap()[0].deadline_monotonic(),
        230
    );
}

#[test]
fn rejected_event_samples_still_advance_clock_watermarks_without_reviving_old_time() {
    for case in 0..4 {
        let mut cache = configured(1);
        let local = source(11, "node-one");
        add(&mut cache, &local);
        match case {
            0 => assert!(matches!(
                cache.observe(&local, b"not-json", 30, time(1_020, 210)),
                Err(LanDiscoveryError::RejectedAdvertisement(
                    DiscoveryError::InvalidAdvertisement
                ))
            )),
            1 => assert!(matches!(
                cache.observe(&local, &raw(&advertisement()), 0, time(1_020, 210)),
                Err(LanDiscoveryError::InvalidTtl)
            )),
            2 => assert!(matches!(
                cache.observe(
                    &source(33, "node-one"),
                    &raw(&advertisement()),
                    30,
                    time(1_020, 210)
                ),
                Err(LanDiscoveryError::InvalidSource)
            )),
            3 => assert!(matches!(
                cache.remove(&source(33, "node-one"), time(1_020, 210)),
                Err(LanDiscoveryError::InvalidSource)
            )),
            _ => unreachable!(),
        }
        assert!(matches!(
            cache.snapshot(time(1_019, 211)),
            Err(LanDiscoveryError::ClockRollback)
        ));
        assert!(matches!(
            cache.snapshot(time(1_021, 212)),
            Err(LanDiscoveryError::Closed)
        ));
    }
}

#[test]
fn fixed_monotonic_second_event_limit_rejects_extra_updates_and_resets_next_second() {
    let mut cache = configured(1);
    let local = source(11, "node-one");
    add(&mut cache, &local);
    for _ in 1..MAX_LAN_EVENTS_PER_SECOND {
        assert!(matches!(
            cache.observe(&local, &raw(&advertisement()), 30, time(1_010, 200)),
            Ok(LanObservation::Duplicate)
        ));
    }
    assert!(matches!(
        cache.observe(
            &source(11, "node-two"),
            &raw(&advertisement()),
            30,
            time(1_010, 200)
        ),
        Err(LanDiscoveryError::RateLimited)
    ));
    assert!(matches!(
        cache.remove(&local, time(1_010, 200)),
        Err(LanDiscoveryError::RateLimited)
    ));
    assert_eq!(cache.snapshot(time(1_010, 200)).unwrap().len(), 1);
    assert!(matches!(
        cache.observe(&local, &raw(&advertisement()), 30, time(1_010, 201)),
        Ok(LanObservation::Duplicate)
    ));
    assert!(cache.remove(&local, time(1_010, 201)).unwrap());
}

#[test]
fn invalid_raw_events_still_consume_the_rate_window_but_never_create_candidates() {
    let mut cache = configured(1);
    let local = source(11, "node-one");
    for _ in 0..MAX_LAN_EVENTS_PER_SECOND {
        assert!(matches!(
            cache.observe(&local, b"not-json", 30, time(1_010, 200)),
            Err(LanDiscoveryError::RejectedAdvertisement(
                DiscoveryError::InvalidAdvertisement
            ))
        ));
    }
    assert!(matches!(
        cache.observe(&local, &raw(&advertisement()), 30, time(1_010, 200)),
        Err(LanDiscoveryError::RateLimited)
    ));
    assert!(cache.snapshot(time(1_010, 200)).unwrap().is_empty());
    assert!(matches!(
        cache.observe(&local, &raw(&advertisement()), 30, time(1_010, 201)),
        Ok(LanObservation::Added)
    ));
}

#[test]
fn incompatible_advertisements_and_missing_deletes_share_the_event_budget() {
    let mut cache = configured(1);
    let local = source(11, "node-one");
    let mut incompatible = advertisement();
    incompatible["protocol"] = json!({"min": 4, "max": 4});
    for index in 0..MAX_LAN_EVENTS_PER_SECOND {
        if index % 2 == 0 {
            assert!(matches!(
                cache.observe(&local, &raw(&incompatible), 30, time(1_010, 200)),
                Err(LanDiscoveryError::RejectedAdvertisement(
                    DiscoveryError::UnsupportedProtocol
                ))
            ));
        } else {
            assert!(!cache.remove(&local, time(1_010, 200)).unwrap());
        }
    }
    assert!(matches!(
        cache.observe(&local, &raw(&advertisement()), 30, time(1_010, 200)),
        Err(LanDiscoveryError::RateLimited)
    ));
}

#[test]
fn invalid_ttl_and_unapproved_interface_events_cannot_bypass_the_budget() {
    let mut cache = configured(1);
    for index in 0..MAX_LAN_EVENTS_PER_SECOND {
        if index % 2 == 0 {
            assert!(matches!(
                cache.observe(
                    &source(11, "node-one"),
                    &raw(&advertisement()),
                    0,
                    time(1_010, 200)
                ),
                Err(LanDiscoveryError::InvalidTtl)
            ));
        } else {
            assert!(matches!(
                cache.remove(&source(33, "node-one"), time(1_010, 200)),
                Err(LanDiscoveryError::InvalidSource)
            ));
        }
    }
    assert!(matches!(
        cache.remove(&source(11, "node-one"), time(1_010, 200)),
        Err(LanDiscoveryError::RateLimited)
    ));
}

#[test]
fn snapshots_do_not_spend_event_admission_or_refresh_expiration() {
    let mut cache = configured(1);
    let local = source(11, "node-one");
    add(&mut cache, &local);
    for _ in 0..MAX_LAN_EVENTS_PER_SECOND * 2 {
        assert_eq!(
            cache.snapshot(time(1_010, 200)).unwrap()[0].deadline_monotonic(),
            230
        );
    }
    for _ in 1..MAX_LAN_EVENTS_PER_SECOND {
        assert!(matches!(
            cache.observe(&local, &raw(&advertisement()), 30, time(1_010, 200)),
            Ok(LanObservation::Duplicate)
        ));
    }
    assert!(matches!(
        cache.remove(&local, time(1_010, 200)),
        Err(LanDiscoveryError::RateLimited)
    ));
}
