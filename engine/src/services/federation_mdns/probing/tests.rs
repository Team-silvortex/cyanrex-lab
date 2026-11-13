use super::*;
use crate::services::federation_discovery::PeerAdvertisement;

fn announcement(name: &str) -> MdnsAnnouncement {
    let value = serde_json::json!({
        "schema_version":1,"node_fingerprint":"b".repeat(64),
        "authority_id":"8b075b0b-8118-4c37-9224-337ba22f40c6",
        "protocol":{"min":1,"max":1},"offered_capabilities":[],
        "required_capabilities":[],"issued_at":1000,"expires_at":1100,
        "hints":[{"kind":"direct","address":"10.23.0.1:4433"}]
    });
    let claim = PeerAdvertisement::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
    MdnsAnnouncement::new(
        name,
        name,
        "10.23.0.1:4433".parse().unwrap(),
        &claim,
        1000,
        60,
    )
    .unwrap()
}

#[test]
fn submillisecond_send_completion_cannot_shorten_the_next_send_or_final_wait() {
    let last = Duration::from_nanos(123_999_999);
    assert_eq!(
        remaining_wait(Some(last), Duration::from_millis(373)),
        Ok(Duration::from_nanos(999_999))
    );
    assert_eq!(
        remaining_wait(Some(last), Duration::from_nanos(373_999_998)),
        Ok(Duration::from_nanos(1))
    );
    assert_eq!(
        remaining_wait(Some(last), Duration::from_nanos(373_999_999)),
        Ok(Duration::ZERO)
    );
    assert_eq!(
        remaining_wait(Some(last), Duration::from_secs(1)),
        Ok(Duration::ZERO)
    );
}

#[test]
fn no_previous_send_needs_no_extra_interval_and_time_reversal_fails() {
    assert_eq!(remaining_wait(None, Duration::ZERO), Ok(Duration::ZERO));
    assert_eq!(
        remaining_wait(Some(Duration::from_secs(1)), Duration::ZERO),
        Err(MdnsError::ClockInvalid)
    );
}

#[test]
fn pre_first_send_packets_are_not_responses_to_this_attempt() {
    let a = announcement("owned");
    for channel in [Channel::Multicast, Channel::Unicast] {
        for bytes in [
            vec![],
            a.encode_at(1000).unwrap(),
            a.encode_probe_at(1000).unwrap(),
        ] {
            assert_eq!(classify(&a, channel, 0, &bytes), Ok(None));
        }
    }
}

#[test]
fn matching_response_after_any_successful_send_stops_either_channel() {
    let a = announcement("owned");
    for sent in 1..=3 {
        for channel in [Channel::Multicast, Channel::Unicast] {
            assert_eq!(
                classify(&a, channel, sent, &a.encode_at(1000).unwrap()),
                Ok(Some(ProbeRunObservation::ExistingResponse))
            );
        }
    }
}

#[test]
fn unrelated_responses_do_not_become_conflicts_or_authority() {
    let a = announcement("owned");
    let other = announcement("unrelated").encode_at(1000).unwrap();
    for channel in [Channel::Multicast, Channel::Unicast] {
        assert_eq!(classify(&a, channel, 1, &other), Ok(None));
    }
}

#[test]
fn only_multicast_can_carry_a_competing_proposal() {
    let a = announcement("owned");
    let bytes = a.encode_probe_at(1000).unwrap();
    assert_eq!(
        classify(&a, Channel::Multicast, 1, &bytes),
        Ok(Some(ProbeRunObservation::CompetingProbe))
    );
    assert_eq!(
        classify(&a, Channel::Unicast, 1, &bytes),
        Err(MdnsError::InvalidPacket)
    );
}

#[test]
fn malformed_or_unsupported_post_send_packets_fail_instead_of_becoming_absence() {
    let a = announcement("owned");
    for channel in [Channel::Multicast, Channel::Unicast] {
        for bytes in [
            vec![],
            vec![0; 11],
            {
                let mut b = a.encode_at(1000).unwrap();
                b.push(0);
                b
            },
            {
                let mut b = a.encode_at(1000).unwrap();
                b[1] = 1;
                b
            },
        ] {
            assert_eq!(
                classify(&a, channel, 1, &bytes),
                Err(MdnsError::InvalidPacket)
            );
        }
    }
}

#[test]
fn cancellation_precedes_clocks_entropy_interfaces_or_socket_work() {
    let a = announcement("owned");
    let config = MdnsConfig::new(
        "missing",
        "10.23.0.1".parse().unwrap(),
        Duration::from_secs(2),
    )
    .unwrap();
    assert_eq!(
        probe_once(&config, &a, &AtomicBool::new(true)),
        Err(MdnsError::Cancelled)
    );
}

#[test]
fn internal_controller_guard_can_reject_before_clock_entropy_or_interface_work() {
    let a = announcement("owned");
    let config = MdnsConfig::new(
        "missing",
        "10.23.0.1".parse().unwrap(),
        Duration::from_secs(2),
    )
    .unwrap();
    let mut checks = 0;
    let result = probe_guarded(&config, &a, &AtomicBool::new(false), &mut || {
        checks += 1;
        Err(MdnsError::DeadlineExceeded)
    });
    assert_eq!(result, Err(MdnsError::DeadlineExceeded));
    assert_eq!(checks, 1);
}

#[test]
fn initial_cancellation_also_precedes_the_internal_controller_guard() {
    let a = announcement("owned");
    let config = MdnsConfig::new(
        "missing",
        "10.23.0.1".parse().unwrap(),
        Duration::from_secs(2),
    )
    .unwrap();
    let mut checks = 0;
    let result = probe_guarded(&config, &a, &AtomicBool::new(true), &mut || {
        checks += 1;
        Err(MdnsError::DeadlineExceeded)
    });
    assert_eq!(result, Err(MdnsError::Cancelled));
    assert_eq!(checks, 0);
}

#[test]
fn frozen_announcement_rejects_wrong_probe_source_and_preparation_clock() {
    let a = announcement("owned");
    assert_eq!(
        a.probe_remaining("10.23.0.2".parse().unwrap(), 1000),
        Err(MdnsError::InvalidConfig)
    );
    assert_eq!(
        a.probe_remaining("10.23.0.1".parse().unwrap(), 999),
        Err(MdnsError::ClockInvalid)
    );
}

#[test]
fn retained_entry_cancellation_precedes_expired_claim_and_interface_work() {
    let config = MdnsConfig::new(
        "missing",
        "10.23.0.1".parse().unwrap(),
        Duration::from_secs(2),
    )
    .unwrap();
    assert!(matches!(
        probe_retained_once(&config, &announcement("owned"), &AtomicBool::new(true)),
        Err(MdnsError::Cancelled)
    ));
}

#[test]
fn retained_entry_source_mismatch_is_rejected_before_interface_work() {
    let config = MdnsConfig::new(
        "missing",
        "10.23.0.2".parse().unwrap(),
        Duration::from_secs(2),
    )
    .unwrap();
    assert!(matches!(
        probe_retained_once(&config, &announcement("owned"), &AtomicBool::new(false)),
        Err(MdnsError::InvalidConfig)
    ));
}

#[test]
fn retained_entry_timeout_bounds_are_pure_preflight_not_an_extended_attempt() {
    for timeout in [Duration::from_millis(999), Duration::from_millis(2001)] {
        let config = MdnsConfig::new("missing", "10.23.0.1".parse().unwrap(), timeout).unwrap();
        assert!(matches!(
            probe_retained_once(&config, &announcement("owned"), &AtomicBool::new(false)),
            Err(MdnsError::InvalidConfig)
        ));
    }
}

#[test]
fn retained_entry_expiry_does_not_fall_through_to_missing_interface() {
    let config = MdnsConfig::new(
        "missing",
        "10.23.0.1".parse().unwrap(),
        Duration::from_secs(2),
    )
    .unwrap();
    assert!(matches!(
        probe_retained_once(&config, &announcement("owned"), &AtomicBool::new(false)),
        Err(MdnsError::RejectedAdvertisement(_))
    ));
}
