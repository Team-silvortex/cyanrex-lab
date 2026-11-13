//! Retained quiet observations in owned anonymous network namespaces; never use the host LAN.
#![cfg(target_os = "linux")]

use cyanrex_engine::services::federation_mdns::{
    probe_retained_once, MdnsConfig, MdnsError, ProbeRunObservation, RetainedProbeOutcome,
    MDNS_GROUP,
};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

#[path = "federation_mdns_retained_network/helpers.rs"]
mod helpers;
use helpers::*;

// Keep the context inferred: it need not be Clone, Debug, comparable, or expose its sockets.
macro_rules! quiet_context {
    ($configuration:expr, $prepared:expr, $cancelled:expr) => {{
        let started = Instant::now();
        let outcome = probe_retained_once($configuration, $prepared, $cancelled).unwrap();
        assert!(started.elapsed() < Duration::from_millis(2250));
        match outcome {
            RetainedProbeOutcome::Quiet(context) => context,
            _ => panic!("the isolated quiet fixture returned a conflict"),
        }
    }};
}

#[test]
#[ignore = "requires explicit --with-retained anonymous netns runner, never host LAN"]
fn quiet_retains_two_exclusive_channels_and_review_or_close_never_sends_again() {
    network_case(3, || {
        let now = wall();
        let prepared = local(now);
        let configuration = config();
        let cancelled = AtomicBool::new(false);
        let descriptors = fd_count();
        arm(0, "none", &[]);
        let mut context = quiet_context!(&configuration, &prepared, &cancelled);
        assert_retained(descriptors);
        let channels = channel_identities();
        let counters = network_counts();
        for _ in 0..2 {
            assert_eq!(
                context.review(&cancelled),
                Ok(ProbeRunObservation::NoConflictObserved)
            );
            assert_retained(descriptors);
            assert_eq!(channel_identities(), channels, "review reopened a channel");
            assert_eq!(network_counts(), counters);
        }
        context.close();
        assert_released(descriptors);
        context.close();
        assert_eq!(context.review(&cancelled), Err(MdnsError::Closed));
        assert_released(descriptors);
        finish(3, &prepared.encode_probe_at(now).unwrap());
    });
}

#[test]
#[ignore = "requires explicit --with-retained anonymous netns runner, never host LAN"]
fn dropping_a_quiet_context_releases_both_channels_without_a_fourth_probe() {
    network_case(3, || {
        let now = wall();
        let prepared = local(now);
        let configuration = config();
        let descriptors = fd_count();
        arm(0, "none", &[]);
        let context = quiet_context!(&configuration, &prepared, &AtomicBool::new(false));
        assert_retained(descriptors);
        drop(context);
        assert_released(descriptors);
        finish(3, &prepared.encode_probe_at(now).unwrap());
    });
}

fn late_conflict(destination: &str, competing: bool) {
    network_case(3, || {
        let now = wall();
        let prepared = local(now);
        let configuration = config();
        let cancelled = AtomicBool::new(false);
        let descriptors = fd_count();
        arm(0, "none", &[]);
        let mut context = quiet_context!(&configuration, &prepared, &cancelled);
        assert_retained(descriptors);
        let peer = announcement("10.23.0.2", now);
        let bytes = if competing {
            peer.encode_probe_at(now).unwrap()
        } else {
            peer.encode_at(now).unwrap()
        };
        // The existing direct-send RPC remains available while capture is active. The return
        // acknowledges the peer send; no review runs until the packet is queued on our veth.
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
        assert_released(descriptors);
        finish(3, &prepared.encode_probe_at(now).unwrap());
    });
}

#[test]
#[ignore = "requires explicit --with-retained anonymous netns runner, never host LAN"]
fn a_queued_late_multicast_response_permanently_closes_the_context() {
    late_conflict(&MDNS_GROUP.to_string(), false);
}

#[test]
#[ignore = "requires explicit --with-retained anonymous netns runner, never host LAN"]
fn a_queued_late_unicast_response_permanently_closes_the_context() {
    late_conflict("10.23.0.1", false);
}

#[test]
#[ignore = "requires explicit --with-retained anonymous netns runner, never host LAN"]
fn a_queued_late_multicast_proposal_permanently_closes_the_context() {
    late_conflict(&MDNS_GROUP.to_string(), true);
}

#[test]
#[ignore = "requires explicit --with-retained anonymous netns runner, never host LAN"]
fn cancellation_after_quiet_drops_both_channels_and_cannot_be_cleared() {
    network_case(3, || {
        let now = wall();
        let prepared = local(now);
        let configuration = config();
        let cancelled = AtomicBool::new(false);
        let descriptors = fd_count();
        arm(0, "none", &[]);
        let mut context = quiet_context!(&configuration, &prepared, &cancelled);
        assert_retained(descriptors);
        cancelled.store(true, Ordering::Release);
        assert_eq!(context.review(&cancelled), Err(MdnsError::Cancelled));
        assert_released(descriptors);
        cancelled.store(false, Ordering::Release);
        assert_eq!(context.review(&cancelled), Err(MdnsError::Closed));
        assert_released(descriptors);
        finish(3, &prepared.encode_probe_at(now).unwrap());
    });
}

#[test]
#[ignore = "requires explicit --with-retained anonymous netns runner, never host LAN"]
fn repeated_quiet_review_cannot_renew_the_original_250ms_review_deadline() {
    network_case(3, || {
        let now = wall();
        let prepared = local(now);
        let configuration = config();
        let cancelled = AtomicBool::new(false);
        let descriptors = fd_count();
        arm(0, "none", &[]);
        let mut context = quiet_context!(&configuration, &prepared, &cancelled);
        let quiet_at = Instant::now();
        thread::sleep(Duration::from_millis(120));
        assert_eq!(
            context.review(&cancelled),
            Ok(ProbeRunObservation::NoConflictObserved)
        );
        thread::sleep(Duration::from_millis(150));
        assert!(quiet_at.elapsed() >= Duration::from_millis(270));
        // No background task watches an idle handle. Its FDs remain owned until review, close,
        // or Drop even though the fixed deadline has expired; no publication right is implied.
        assert_retained(descriptors);
        assert_eq!(context.review(&cancelled), Err(MdnsError::DeadlineExceeded));
        assert_released(descriptors);
        assert_eq!(context.review(&cancelled), Err(MdnsError::Closed));
        finish(3, &prepared.encode_probe_at(now).unwrap());
    });
}

#[test]
#[ignore = "requires explicit --with-retained anonymous netns runner, never host LAN"]
fn a_queued_malformed_packet_fails_closed_and_releases_both_channels() {
    network_case(3, || {
        let now = wall();
        let prepared = local(now);
        let configuration = config();
        let cancelled = AtomicBool::new(false);
        let descriptors = fd_count();
        arm(0, "none", &[]);
        let mut context = quiet_context!(&configuration, &prepared, &cancelled);
        send(&MDNS_GROUP.to_string(), &[0; 11]);
        assert_eq!(context.review(&cancelled), Err(MdnsError::InvalidPacket));
        assert_released(descriptors);
        assert_eq!(context.review(&cancelled), Err(MdnsError::Closed));
        finish(3, &prepared.encode_probe_at(now).unwrap());
    });
}

#[test]
#[ignore = "requires explicit --with-retained NET_ADMIN owned-netns mutation, never host LAN"]
fn a_renamed_retained_interface_fails_review_and_releases_the_original_channels() {
    network_mutation_case(3, || {
        let now = wall();
        let prepared = local(now);
        let configuration = config();
        let cancelled = AtomicBool::new(false);
        let descriptors = fd_count();
        arm(0, "none", &[]);
        let mut context = quiet_context!(&configuration, &prepared, &cancelled);
        let quiet_at = Instant::now();
        assert_retained(descriptors);
        // Only the selected fixture inside this verified owned namespace is renamed. The
        // namespace-owning runner restores its exact name before the older mutation cases.
        let output = std::process::Command::new("ip")
            .args(["link", "set", "dev", "crx0", "name", "crx4"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(quiet_at.elapsed() < Duration::from_millis(250));
        assert_eq!(context.review(&cancelled), Err(MdnsError::InterfaceChanged));
        assert_released(descriptors);
        assert_eq!(context.review(&cancelled), Err(MdnsError::Closed));
        assert_released(descriptors);
        finish(3, &prepared.encode_probe_at(now).unwrap());
    });
}

#[test]
#[ignore = "requires explicit --with-retained anonymous netns runner, never host LAN"]
fn actual_response_or_proposal_conflicts_return_no_retained_handle() {
    network_case(2, || {
        let now = wall();
        let prepared = local(now);
        let peer = announcement("10.23.0.2", now);
        let descriptors = fd_count();
        for competing in [false, true] {
            let bytes = if competing {
                peer.encode_probe_at(now).unwrap()
            } else {
                peer.encode_at(now).unwrap()
            };
            arm(1, &MDNS_GROUP.to_string(), &bytes);
            let result = probe_retained_once(&config(), &prepared, &AtomicBool::new(false));
            assert!(if competing {
                matches!(result, Ok(RetainedProbeOutcome::CompetingProbe))
            } else {
                matches!(result, Ok(RetainedProbeOutcome::ExistingResponse))
            });
            assert_released(descriptors);
            finish(1, &prepared.encode_probe_at(now).unwrap());
        }
    });
}

#[test]
#[ignore = "requires explicit --with-retained anonymous netns runner, never host LAN"]
fn source_mismatch_invalid_timeout_and_pre_cancellation_open_no_sockets_or_send() {
    network_case(0, || {
        let now = wall();
        let prepared = local(now);
        let descriptors = fd_count();
        arm(0, "none", &[]);
        let mismatch = announcement("10.23.0.2", now);
        assert!(matches!(
            probe_retained_once(&config(), &mismatch, &AtomicBool::new(false)),
            Err(MdnsError::InvalidConfig)
        ));
        assert_released(descriptors);
        let invalid_timeout =
            MdnsConfig::new("crx0", "10.23.0.1".parse().unwrap(), Duration::from_secs(3)).unwrap();
        assert!(matches!(
            probe_retained_once(&invalid_timeout, &prepared, &AtomicBool::new(false)),
            Err(MdnsError::InvalidConfig)
        ));
        assert_released(descriptors);
        let absent = MdnsConfig::new(
            "missing0",
            "10.23.0.1".parse().unwrap(),
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(matches!(
            probe_retained_once(&absent, &prepared, &AtomicBool::new(true)),
            Err(MdnsError::Cancelled)
        ));
        assert_released(descriptors);
        finish(0, &prepared.encode_probe_at(now).unwrap());
    });
}
