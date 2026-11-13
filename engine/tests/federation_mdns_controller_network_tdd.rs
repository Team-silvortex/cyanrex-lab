//! Explicit bounded controller acceptance; never run on the host LAN.
#![cfg(target_os = "linux")]

use cyanrex_engine::services::federation_mdns::{
    MdnsConfig, MdnsError, MdnsProbeController, ProbeControllerState, ProbeRunObservation,
};
use std::{process::Command, sync::atomic::AtomicBool, thread, time::Duration};

#[path = "federation_mdns_controller_network/helpers.rs"]
mod helpers;
use helpers::*;

#[test]
#[ignore = "requires explicit --with-controller anonymous netns runner, never host LAN"]
fn constructing_and_dropping_a_ready_controller_has_no_network_effects() {
    network_case(0, || {
        let controller = MdnsProbeController::new(config(), local(wall())).unwrap();
        assert_eq!(controller.state(), ProbeControllerState::Ready);
        assert_eq!(controller.attempts_started(), 0);
        assert_eq!(controller.retry_after(), None);
        assert_no_listener();
        drop(controller);
    });
}

#[test]
#[ignore = "requires explicit --with-controller anonymous netns runner, never host LAN"]
fn a_conflict_cools_without_network_io_then_one_explicit_retry_finishes() {
    network_case(4, || {
        let now = wall();
        let reference = local(now).encode_probe_at(now).unwrap();
        let peer = announcement("10.23.0.2", now).encode_at(now).unwrap();
        let mut controller = MdnsProbeController::new(config(), local(now)).unwrap();
        arm(1, "10.23.0.1", &peer);
        assert_eq!(
            controller.attempt(&AtomicBool::new(false)),
            Ok(ProbeRunObservation::ExistingResponse)
        );
        let conflict_returned = monotonic_ns();
        assert_eq!(controller.state(), ProbeControllerState::CoolingDown);
        assert_eq!(controller.attempts_started(), 1);
        let remaining = controller.retry_after().unwrap();
        assert!(remaining > Duration::ZERO && remaining <= Duration::from_secs(5));
        assert_no_listener();
        let counts = network_counts();
        let descriptors = fd_count();
        assert_eq!(
            controller.attempt(&AtomicBool::new(false)),
            Err(MdnsError::CoolingDown)
        );
        assert_eq!(controller.state(), ProbeControllerState::CoolingDown);
        assert_eq!(controller.attempts_started(), 1);
        assert_eq!(network_counts(), counts);
        assert_eq!(fd_count(), descriptors);
        assert!(controller.retry_after().unwrap() <= remaining);
        finish(1, &reference);
        let remaining = controller.retry_after().unwrap();
        assert!(remaining <= Duration::from_secs(5));
        thread::sleep(remaining + Duration::from_millis(30));
        arm(0, "none", &[]);
        assert_eq!(
            controller.attempt(&AtomicBool::new(false)),
            Ok(ProbeRunObservation::NoConflictObserved)
        );
        assert_eq!(controller.state(), ProbeControllerState::Finished);
        assert_eq!(controller.attempts_started(), 2);
        assert_eq!(controller.retry_after(), None);
        assert_no_listener();
        let queries = finish(3, &reference);
        assert!(queries[0] - conflict_returned >= 5_000_000_000);
        let counts = network_counts();
        assert_eq!(
            controller.attempt(&AtomicBool::new(false)),
            Err(MdnsError::Closed)
        );
        assert_eq!(controller.attempts_started(), 2);
        assert_eq!(network_counts(), counts);
    });
}

#[test]
#[ignore = "requires explicit --with-controller anonymous netns runner, never host LAN"]
fn pre_cancellation_closes_without_starting_an_attempt() {
    network_case(0, || {
        let mut controller = MdnsProbeController::new(config(), local(wall())).unwrap();
        assert_eq!(
            controller.attempt(&AtomicBool::new(true)),
            Err(MdnsError::Cancelled)
        );
        assert_eq!(controller.state(), ProbeControllerState::Closed);
        assert_eq!(controller.attempts_started(), 0);
        assert_eq!(controller.retry_after(), None);
        assert_eq!(
            controller.attempt(&AtomicBool::new(false)),
            Err(MdnsError::Closed)
        );
    });
}

#[test]
#[ignore = "requires explicit --with-controller anonymous netns runner, never host LAN"]
fn an_expired_frozen_claim_closes_without_network_io() {
    network_case(0, || {
        let mut controller = MdnsProbeController::new(config(), local(wall() - 120)).unwrap();
        assert!(matches!(
            controller.attempt(&AtomicBool::new(false)),
            Err(MdnsError::RejectedAdvertisement(_))
        ));
        assert_eq!(controller.state(), ProbeControllerState::Closed);
        assert_eq!(controller.attempts_started(), 0);
        assert_eq!(controller.retry_after(), None);
    });
}

#[test]
#[ignore = "requires explicit --with-controller anonymous netns runner, never host LAN"]
fn a_socket_error_closes_without_retrying_or_sending() {
    network_case(0, || {
        let mut controller = MdnsProbeController::new(config(), local(wall())).unwrap();
        let occupant = occupy_local_port();
        assert_eq!(
            controller.attempt(&AtomicBool::new(false)),
            Err(MdnsError::SocketUnavailable)
        );
        assert_eq!(controller.state(), ProbeControllerState::Closed);
        assert_eq!(controller.attempts_started(), 1);
        assert_eq!(controller.retry_after(), None);
        drop(occupant);
        assert_no_listener();
        assert_eq!(
            controller.attempt(&AtomicBool::new(false)),
            Err(MdnsError::Closed)
        );
    });
}

#[test]
#[ignore = "requires explicit --with-controller anonymous netns runner, never host LAN"]
fn invalid_constructor_bounds_and_source_mismatch_never_touch_an_interface() {
    network_case(0, || {
        for timeout in [Duration::from_millis(999), Duration::from_millis(2001)] {
            let config = MdnsConfig::new("missing", "10.23.0.1".parse().unwrap(), timeout).unwrap();
            assert!(matches!(
                MdnsProbeController::new(config, local(wall())),
                Err(MdnsError::InvalidConfig)
            ));
        }
        let config = MdnsConfig::new(
            "missing",
            "10.23.0.2".parse().unwrap(),
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(matches!(
            MdnsProbeController::new(config, local(wall())),
            Err(MdnsError::InvalidConfig)
        ));
    });
}

#[test]
#[ignore = "requires explicit --with-controller anonymous netns runner, never host LAN"]
fn cancellation_while_cooling_is_terminal_without_an_additional_probe() {
    network_case(1, || {
        let now = wall();
        let reference = local(now).encode_probe_at(now).unwrap();
        let peer = announcement("10.23.0.2", now).encode_at(now).unwrap();
        let mut controller = MdnsProbeController::new(config(), local(now)).unwrap();
        arm(1, "10.23.0.1", &peer);
        assert_eq!(
            controller.attempt(&AtomicBool::new(false)),
            Ok(ProbeRunObservation::ExistingResponse)
        );
        finish(1, &reference);
        assert_eq!(controller.state(), ProbeControllerState::CoolingDown);
        let counts = network_counts();
        assert_eq!(
            controller.attempt(&AtomicBool::new(true)),
            Err(MdnsError::Cancelled)
        );
        assert_eq!(controller.state(), ProbeControllerState::Closed);
        assert_eq!(controller.attempts_started(), 1);
        assert_eq!(controller.retry_after(), None);
        assert_eq!(network_counts(), counts);
        assert_no_listener();
    });
}

#[test]
#[ignore = "requires explicit --with-controller owned-netns NET_ADMIN mutation runner, never host LAN"]
fn a_renamed_pinned_interface_is_not_looked_up_during_cooldown_or_replaced_after_it() {
    network_mutation_case(1, || {
        let now = wall();
        let reference = local(now).encode_probe_at(now).unwrap();
        let peer = announcement("10.23.0.2", now).encode_at(now).unwrap();
        let mut controller = MdnsProbeController::new(config(), local(now)).unwrap();
        arm(1, "10.23.0.1", &peer);
        assert_eq!(
            controller.attempt(&AtomicBool::new(false)),
            Ok(ProbeRunObservation::ExistingResponse)
        );
        assert_no_listener();
        assert!(Command::new("ip")
            .args(["link", "set", "dev", "crx0", "name", "crx4"])
            .status()
            .unwrap()
            .success());
        let counts = network_counts();
        let descriptors = fd_count();
        assert_eq!(
            controller.attempt(&AtomicBool::new(false)),
            Err(MdnsError::CoolingDown)
        );
        assert_eq!(controller.state(), ProbeControllerState::CoolingDown);
        assert_eq!(controller.attempts_started(), 1);
        assert_eq!(network_counts(), counts);
        assert_eq!(fd_count(), descriptors);
        finish(1, &reference);
        let remaining = controller.retry_after().unwrap();
        assert!(remaining <= Duration::from_secs(5));
        thread::sleep(remaining + Duration::from_millis(30));
        assert_eq!(
            controller.attempt(&AtomicBool::new(false)),
            Err(MdnsError::InterfaceChanged)
        );
        assert_eq!(controller.state(), ProbeControllerState::Closed);
        // Admission counts the second attempt even though its pinned-interface check fails
        // before the probe runtime opens a socket. The entire case still sends exactly one UDP.
        assert_eq!(controller.attempts_started(), 2);
        assert_eq!(controller.retry_after(), None);
        assert_eq!(network_counts(), counts);
        assert_no_listener();
        assert_eq!(
            controller.attempt(&AtomicBool::new(false)),
            Err(MdnsError::Closed)
        );
    });
}
