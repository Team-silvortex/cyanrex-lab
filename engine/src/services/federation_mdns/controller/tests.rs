use super::*;
use crate::services::federation_discovery::PeerAdvertisement;

fn announcement() -> MdnsAnnouncement {
    let value = serde_json::json!({
        "schema_version":1,"node_fingerprint":"b".repeat(64),
        "authority_id":"8b075b0b-8118-4c37-9224-337ba22f40c6",
        "protocol":{"min":1,"max":1},"offered_capabilities":[],
        "required_capabilities":[],"issued_at":1000,"expires_at":1100,
        "hints":[{"kind":"direct","address":"10.23.0.1:4433"}]
    });
    let claim = PeerAdvertisement::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
    MdnsAnnouncement::new(
        "node-one",
        "host-one",
        "10.23.0.1:4433".parse().unwrap(),
        &claim,
        1000,
        60,
    )
    .unwrap()
}

fn config(address: &str, timeout: Duration) -> MdnsConfig {
    MdnsConfig::new("missing", address.parse().unwrap(), timeout).unwrap()
}

#[test]
fn construction_only_prepares_frozen_inputs_without_clock_or_interface_work() {
    // The wall timestamp is deliberately historical and interface deliberately missing.
    let controller =
        MdnsProbeController::new(config("10.23.0.1", Duration::from_secs(2)), announcement())
            .unwrap();
    assert_eq!(controller.state(), ProbeControllerState::Ready);
    assert_eq!(controller.attempts_started(), 0);
    assert_eq!(controller.retry_after(), None);
    assert!(
        controller.origin.is_none()
            && controller.lifetime.is_none()
            && controller.interface.is_none()
    );
}

#[test]
fn mismatching_frozen_source_is_rejected_before_any_clock_or_interface_work() {
    assert!(matches!(
        MdnsProbeController::new(config("10.23.0.2", Duration::from_secs(2)), announcement()),
        Err(MdnsError::InvalidConfig)
    ));
}

#[test]
fn controller_timeout_reuses_the_one_attempt_profile_instead_of_relaxing_it() {
    for timeout in [Duration::from_millis(999), Duration::from_millis(2001)] {
        assert!(matches!(
            MdnsProbeController::new(config("10.23.0.1", timeout), announcement()),
            Err(MdnsError::InvalidConfig)
        ));
    }
}

#[test]
fn initial_cancellation_is_terminal_before_expired_claims_or_interface_lookups() {
    let mut controller =
        MdnsProbeController::new(config("10.23.0.1", Duration::from_secs(2)), announcement())
            .unwrap();
    assert_eq!(
        controller.attempt(&AtomicBool::new(true)),
        Err(MdnsError::Cancelled)
    );
    assert_eq!(controller.state(), ProbeControllerState::Closed);
    assert_eq!(controller.attempts_started(), 0);
    assert!(
        controller.origin.is_none()
            && controller.lifetime.is_none()
            && controller.interface.is_none()
    );
    assert_eq!(
        controller.attempt(&AtomicBool::new(false)),
        Err(MdnsError::Closed)
    );
}

#[test]
fn expired_claims_close_before_a_missing_interface_can_be_looked_up() {
    let mut controller =
        MdnsProbeController::new(config("10.23.0.1", Duration::from_secs(2)), announcement())
            .unwrap();
    assert!(matches!(
        controller.attempt(&AtomicBool::new(false)),
        Err(MdnsError::RejectedAdvertisement(_))
    ));
    assert_eq!(controller.state(), ProbeControllerState::Closed);
    assert_eq!(controller.attempts_started(), 0);
    assert!(controller.interface.is_none());
    assert_eq!(
        controller.attempt(&AtomicBool::new(false)),
        Err(MdnsError::Closed)
    );
}
