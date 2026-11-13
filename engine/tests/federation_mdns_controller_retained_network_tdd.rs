//! Explicit controller-to-retained acceptance in owned disconnected namespaces, never the host LAN.
#![cfg(target_os = "linux")]

use cyanrex_engine::services::federation_mdns::{
    MdnsError, MdnsProbeController, ProbeControllerState, ProbeRunObservation,
    RetainedProbeOutcome, MDNS_GROUP,
};
use std::{
    net::UdpSocket,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

#[path = "federation_mdns_controller_retained_network/helpers.rs"]
mod helpers;
use helpers::*;

// A live handle borrows the controller; inspect its terminal state only after dropping the handle.
macro_rules! quiet_context {
    ($controller:expr, $cancelled:expr) => {{
        let started = Instant::now();
        match $controller.attempt_retained($cancelled).unwrap() {
            RetainedProbeOutcome::Quiet(context) => {
                assert!(started.elapsed() < Duration::from_millis(2250));
                context
            }
            _ => panic!("the isolated controller fixture returned a conflict"),
        }
    }};
}

fn assert_finished(controller: &mut MdnsProbeController, attempts: u8) {
    assert_eq!(controller.state(), ProbeControllerState::Finished);
    assert_eq!(controller.attempts_started(), attempts);
    assert_eq!(controller.retry_after(), None);
    let counts = network_counts();
    assert!(matches!(
        controller.attempt_retained(&AtomicBool::new(false)),
        Err(MdnsError::Closed)
    ));
    assert_eq!(controller.attempts_started(), attempts);
    assert_eq!(network_counts(), counts);
}

#[test]
#[ignore = "requires explicit --with-controller-retained anonymous netns runner, never host LAN"]
fn conflict_cooldown_then_quiet_retains_the_same_channels_without_another_send() {
    controller_case(4, || {
        let now = wall();
        let reference = local(now).encode_probe_at(now).unwrap();
        let peer = announcement("10.23.0.2", now).encode_at(now).unwrap();
        let mut controller = MdnsProbeController::new(config(), local(now)).unwrap();
        let cancelled = AtomicBool::new(false);
        let descriptors = fd_count();
        arm(1, "10.23.0.1", &peer);
        assert!(matches!(
            controller.attempt_retained(&cancelled),
            Ok(RetainedProbeOutcome::ExistingResponse)
        ));
        let conflict_returned = monotonic_ns();
        assert_released(descriptors);
        assert_eq!(controller.state(), ProbeControllerState::CoolingDown);
        assert_eq!(controller.attempts_started(), 1);
        let counts = network_counts();
        assert!(matches!(
            controller.attempt_retained(&cancelled),
            Err(MdnsError::CoolingDown)
        ));
        assert_eq!(controller.attempts_started(), 1);
        assert_eq!(network_counts(), counts);
        assert_released(descriptors);
        finish(1, &reference);
        let remaining = controller.retry_after().unwrap();
        assert!(remaining <= Duration::from_secs(5));
        thread::sleep(remaining + Duration::from_millis(30));
        arm(0, "none", &[]);
        let mut context = quiet_context!(controller, &cancelled);
        assert_retained(descriptors);
        let channels = channel_identities();
        let counts = network_counts();
        for _ in 0..2 {
            assert_eq!(
                context.review(&cancelled),
                Ok(ProbeRunObservation::NoConflictObserved)
            );
            assert_eq!(channel_identities(), channels, "review reopened a channel");
            assert_eq!(network_counts(), counts);
        }
        context.close();
        assert_released(descriptors);
        assert_eq!(context.review(&cancelled), Err(MdnsError::Closed));
        drop(context);
        assert_finished(&mut controller, 2);
        finish_after(3, &reference, conflict_returned);
    });
}

fn late_conflict(destination: &str, competing: bool) {
    controller_case(3, || {
        let now = wall();
        let reference = local(now).encode_probe_at(now).unwrap();
        let mut controller = MdnsProbeController::new(config(), local(now)).unwrap();
        let cancelled = AtomicBool::new(false);
        let descriptors = fd_count();
        arm(0, "none", &[]);
        let mut context = quiet_context!(controller, &cancelled);
        let peer = announcement("10.23.0.2", now);
        let bytes = if competing {
            peer.encode_probe_at(now).unwrap()
        } else {
            peer.encode_at(now).unwrap()
        };
        send(destination, &bytes);
        assert_eq!(
            context.review(&cancelled),
            Ok(if competing {
                ProbeRunObservation::CompetingProbe
            } else {
                ProbeRunObservation::ExistingResponse
            })
        );
        assert_released(descriptors);
        assert_eq!(context.review(&cancelled), Err(MdnsError::Closed));
        drop(context);
        assert_finished(&mut controller, 1);
        finish(3, &reference);
    });
}

#[test]
#[ignore = "requires explicit --with-controller-retained anonymous netns runner, never host LAN"]
fn a_queued_late_multicast_response_closes_the_controller_retained_handle() {
    late_conflict(&MDNS_GROUP.to_string(), false);
}

#[test]
#[ignore = "requires explicit --with-controller-retained anonymous netns runner, never host LAN"]
fn a_queued_late_unicast_response_closes_the_controller_retained_handle() {
    late_conflict("10.23.0.1", false);
}

#[test]
#[ignore = "requires explicit --with-controller-retained anonymous netns runner, never host LAN"]
fn a_queued_late_multicast_proposal_closes_the_controller_retained_handle() {
    late_conflict(&MDNS_GROUP.to_string(), true);
}

#[test]
#[ignore = "requires explicit --with-controller-retained anonymous netns runner, never host LAN"]
fn cancellation_after_controller_handoff_closes_permanently_without_retry() {
    controller_case(3, || {
        let now = wall();
        let reference = local(now).encode_probe_at(now).unwrap();
        let mut controller = MdnsProbeController::new(config(), local(now)).unwrap();
        let cancelled = AtomicBool::new(false);
        let descriptors = fd_count();
        arm(0, "none", &[]);
        let mut context = quiet_context!(controller, &cancelled);
        assert_retained(descriptors);
        cancelled.store(true, Ordering::Release);
        assert_eq!(context.review(&cancelled), Err(MdnsError::Cancelled));
        assert_released(descriptors);
        cancelled.store(false, Ordering::Release);
        assert_eq!(context.review(&cancelled), Err(MdnsError::Closed));
        drop(context);
        assert_finished(&mut controller, 1);
        finish(3, &reference);
    });
}

#[test]
#[ignore = "requires explicit --with-controller-retained anonymous netns runner, never host LAN"]
fn controller_reviews_do_not_renew_the_fixed_quiet_window() {
    controller_case(3, || {
        let now = wall();
        let reference = local(now).encode_probe_at(now).unwrap();
        let mut controller = MdnsProbeController::new(config(), local(now)).unwrap();
        let cancelled = AtomicBool::new(false);
        let descriptors = fd_count();
        arm(0, "none", &[]);
        let mut context = quiet_context!(controller, &cancelled);
        thread::sleep(Duration::from_millis(120));
        assert_eq!(
            context.review(&cancelled),
            Ok(ProbeRunObservation::NoConflictObserved)
        );
        thread::sleep(Duration::from_millis(150));
        assert_retained(descriptors);
        assert_eq!(context.review(&cancelled), Err(MdnsError::DeadlineExceeded));
        assert_released(descriptors);
        assert_eq!(context.review(&cancelled), Err(MdnsError::Closed));
        drop(context);
        assert_finished(&mut controller, 1);
        finish(3, &reference);
    });
}

#[test]
#[ignore = "requires explicit --with-controller-retained anonymous netns runner, never host LAN"]
fn dropping_a_controller_quiet_handle_releases_both_channels_without_revival() {
    controller_case(3, || {
        let now = wall();
        let reference = local(now).encode_probe_at(now).unwrap();
        let mut controller = MdnsProbeController::new(config(), local(now)).unwrap();
        let cancelled = AtomicBool::new(false);
        let descriptors = fd_count();
        arm(0, "none", &[]);
        let context = quiet_context!(controller, &cancelled);
        assert_retained(descriptors);
        drop(context);
        assert_released(descriptors);
        assert_finished(&mut controller, 1);
        finish(3, &reference);
    });
}

#[test]
#[ignore = "requires explicit --with-controller-retained anonymous netns runner, never host LAN"]
fn pre_cancellation_and_port_contention_never_send_or_retry() {
    controller_case(0, || {
        let now = wall();
        let reference = local(now).encode_probe_at(now).unwrap();
        let descriptors = fd_count();
        arm(0, "none", &[]);
        let mut cancelled = MdnsProbeController::new(config(), local(now)).unwrap();
        assert!(matches!(
            cancelled.attempt_retained(&AtomicBool::new(true)),
            Err(MdnsError::Cancelled)
        ));
        assert_eq!(cancelled.state(), ProbeControllerState::Closed);
        assert_eq!(cancelled.attempts_started(), 0);
        assert_released(descriptors);
        let mut blocked = MdnsProbeController::new(config(), local(now)).unwrap();
        let occupant = UdpSocket::bind("10.23.0.1:5353").unwrap();
        assert!(matches!(
            blocked.attempt_retained(&AtomicBool::new(false)),
            Err(MdnsError::SocketUnavailable)
        ));
        assert_eq!(blocked.state(), ProbeControllerState::Closed);
        assert_eq!(blocked.attempts_started(), 1);
        drop(occupant);
        assert_released(descriptors);
        for controller in [&mut cancelled, &mut blocked] {
            assert!(matches!(
                controller.attempt_retained(&AtomicBool::new(false)),
                Err(MdnsError::Closed)
            ));
        }
        finish(0, &reference);
    });
}

#[test]
#[ignore = "requires explicit --with-controller-retained anonymous netns runner, never host LAN"]
fn a_malformed_queued_packet_closes_the_controller_retained_handle() {
    controller_case(3, || {
        let now = wall();
        let reference = local(now).encode_probe_at(now).unwrap();
        let mut controller = MdnsProbeController::new(config(), local(now)).unwrap();
        let cancelled = AtomicBool::new(false);
        let descriptors = fd_count();
        arm(0, "none", &[]);
        let mut context = quiet_context!(controller, &cancelled);
        send(&MDNS_GROUP.to_string(), &[0; 11]);
        assert_eq!(context.review(&cancelled), Err(MdnsError::InvalidPacket));
        assert_released(descriptors);
        assert_eq!(context.review(&cancelled), Err(MdnsError::Closed));
        drop(context);
        assert_finished(&mut controller, 1);
        finish(3, &reference);
    });
}

#[test]
#[ignore = "requires explicit --with-controller-retained NET_ADMIN owned-netns mutation, never host LAN"]
fn a_renamed_controller_retained_interface_closes_and_releases_original_channels() {
    controller_mutation_case(3, || {
        let now = wall();
        let reference = local(now).encode_probe_at(now).unwrap();
        let mut controller = MdnsProbeController::new(config(), local(now)).unwrap();
        let cancelled = AtomicBool::new(false);
        let descriptors = fd_count();
        arm(0, "none", &[]);
        let mut context = quiet_context!(controller, &cancelled);
        let quiet_at = Instant::now();
        assert_retained(descriptors);
        let output = std::process::Command::new("ip")
            .args(["link", "set", "dev", "crx0", "name", "crx4"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(quiet_at.elapsed() < Duration::from_millis(250));
        assert_eq!(context.review(&cancelled), Err(MdnsError::InterfaceChanged));
        assert_released(descriptors);
        assert_eq!(context.review(&cancelled), Err(MdnsError::Closed));
        drop(context);
        assert_finished(&mut controller, 1);
        finish(3, &reference);
        // Only this owned namespace is changed; its runner restores crx4 to crx0 afterwards.
    });
}
