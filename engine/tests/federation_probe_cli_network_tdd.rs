//! Explicit real probe CLI acceptance in disconnected owned namespaces; never use the host LAN.
#![cfg(target_os = "linux")]

use cyanrex_engine::services::federation_mdns::MDNS_GROUP;
use std::net::UdpSocket;

#[path = "federation_probe_cli_network/helpers.rs"]
mod helpers;
use helpers::*;

#[test]
#[ignore = "requires explicit --with-probe-cli anonymous netns runner, never host LAN"]
fn real_probe_cli_quiet_round_has_three_exact_probes_and_one_inert_observation() {
    network_case(3, || {
        let now = wall();
        let reference = prepared("10.23.0.1", now).encode_probe_at(now).unwrap();
        arm(0, "none", &[]);
        let output = Cli::start(&arguments(now)).finish();
        output.observation("no_conflict_observed");
        let received = finish(3, &reference);
        assert!(output.completed_ns - received[2] >= 240_000_000);
    });
}

fn conflict(after: u8, competing: bool) {
    network_case(u64::from(after), || {
        let now = wall();
        let reference = prepared("10.23.0.1", now).encode_probe_at(now).unwrap();
        let peer = prepared("10.23.0.2", now);
        let bytes = if competing {
            peer.encode_probe_at(now).unwrap()
        } else {
            peer.encode_at(now).unwrap()
        };
        let destination = if competing {
            MDNS_GROUP.to_string()
        } else {
            "10.23.0.1".to_owned()
        };
        arm(after, &destination, &bytes);
        let output = Cli::start(&arguments(now)).finish();
        output.observation(if competing {
            "competing_probe"
        } else {
            "existing_response"
        });
        finish(usize::from(after), &reference);
    });
}

#[test]
#[ignore = "requires explicit --with-probe-cli anonymous netns runner, never host LAN"]
fn real_probe_cli_unicast_response_after_first_probe_stops_without_retry() {
    conflict(1, false);
}

#[test]
#[ignore = "requires explicit --with-probe-cli anonymous netns runner, never host LAN"]
fn real_probe_cli_unicast_response_after_third_probe_prevents_quiet_success() {
    conflict(3, false);
}

#[test]
#[ignore = "requires explicit --with-probe-cli anonymous netns runner, never host LAN"]
fn real_probe_cli_competing_multicast_proposal_is_an_observation_without_retry() {
    conflict(1, true);
}

#[test]
#[ignore = "requires explicit --with-probe-cli anonymous netns runner, never host LAN"]
fn real_probe_cli_malformed_post_send_packet_fails_without_an_observation() {
    network_case(1, || {
        let now = wall();
        let reference = prepared("10.23.0.1", now).encode_probe_at(now).unwrap();
        arm(1, &MDNS_GROUP.to_string(), &[0; 11]);
        Cli::start(&arguments(now)).finish().rejected(74);
        finish(1, &reference);
    });
}

#[test]
#[ignore = "requires explicit --with-probe-cli anonymous netns runner, never host LAN"]
fn real_probe_cli_occupied_local_port_fails_before_any_probe() {
    network_case(0, || {
        let now = wall();
        let reference = prepared("10.23.0.1", now).encode_probe_at(now).unwrap();
        let occupant = UdpSocket::bind("10.23.0.1:5353").unwrap();
        arm(0, "none", &[]);
        Cli::start(&arguments(now)).finish().rejected(74);
        finish(0, &reference);
        drop(occupant);
    });
}

#[test]
#[ignore = "requires explicit --with-probe-cli anonymous netns runner, never host LAN"]
fn real_probe_cli_wrong_identity_and_expired_claim_never_send_or_echo_input() {
    network_case(0, || {
        let now = wall();
        let reference = prepared("10.23.0.1", now).encode_probe_at(now).unwrap();
        arm(0, "none", &[]);
        let mut wrong_identity = arguments(now);
        wrong_identity[2] = "1001".into();
        wrong_identity[22] = advertisement("10.23.0.1", now - 120);
        Cli::start(&wrong_identity).finish().rejected(77);
        // The same stale declaration is a preparation/input failure under the correct process.
        let mut expired = arguments(now);
        expired[22] = advertisement("10.23.0.1", now - 120);
        Cli::start(&expired).finish().rejected(64);
        finish(0, &reference);
    });
}

#[test]
#[ignore = "requires explicit --with-probe-cli anonymous netns runner, never host LAN"]
fn real_probe_cli_missing_interface_fails_without_a_probe_or_observation() {
    network_case(0, || {
        let now = wall();
        let reference = prepared("10.23.0.1", now).encode_probe_at(now).unwrap();
        let mut options = arguments(now);
        options[8] = "cli-missing".into();
        arm(0, "none", &[]);
        Cli::start(&options).finish().rejected(74);
        finish(0, &reference);
    });
}

fn interrupted(signal: i32) {
    network_case(1, || {
        let now = wall();
        let reference = prepared("10.23.0.1", now).encode_probe_at(now).unwrap();
        arm(0, "none", &[]);
        let sent = network_counts().0;
        let mut child = Cli::start(&arguments(now));
        child.signal_after_first_probe(sent, signal);
        child.finish().interrupted(signal);
        finish(1, &reference);
    });
}

#[test]
#[ignore = "requires explicit --with-probe-cli anonymous netns runner, never host LAN"]
fn real_probe_cli_sigint_after_first_probe_releases_both_channels_without_quiet_output() {
    interrupted(libc::SIGINT);
}

#[test]
#[ignore = "requires explicit --with-probe-cli anonymous netns runner, never host LAN"]
fn real_probe_cli_sigterm_after_first_probe_releases_both_channels_without_quiet_output() {
    interrupted(libc::SIGTERM);
}
