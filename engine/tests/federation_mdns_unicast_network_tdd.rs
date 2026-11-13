//! Explicit receive-only unicast acceptance in two owned anonymous network namespaces.
//! Never run this ignored target against the host LAN, or infer name availability from silence.
#![cfg(target_os = "linux")]

use cyanrex_engine::services::federation_mdns::{
    MdnsError, MdnsSocket, MdnsUnicastReceiver, ProbeObservation, MDNS_GROUP, MDNS_PORT,
};
use std::{net::Ipv4Addr, time::Instant};

#[path = "federation_mdns_unicast_network/binding.rs"]
mod binding;
#[path = "federation_mdns_unicast_network/helpers.rs"]
mod helpers;
use helpers::*;

#[test]
#[ignore = "requires explicit --with-unicast anonymous netns runner, never host LAN"]
fn real_unicast_bytes_reach_only_conservative_f1e_observation() {
    network_case(false, || {
        let mut receiver = unicast("crx0", "10.23.0.1");
        let announcement = announcement("peer", "peer-host");
        for (bytes, expected) in [
            (
                announcement.encode_at(1000).unwrap(),
                ProbeObservation::ExistingResponse,
            ),
            (
                announcement.encode_probe_at(1000).unwrap(),
                ProbeObservation::CompetingProbe,
            ),
        ] {
            send("10.23.0.2", MDNS_PORT, 255, "10.23.0.1", &bytes);
            let packet = receiver.receive().unwrap().unwrap();
            assert_eq!(packet.bytes, bytes);
            assert_eq!(packet.source, Ipv4Addr::new(10, 23, 0, 2));
            assert_eq!(
                announcement.observe_probe_packet(&packet.bytes),
                Ok(expected)
            );
        }
    });
}

#[test]
#[ignore = "requires explicit --with-unicast anonymous netns runner, never host LAN"]
fn multicast_then_unicast_preserve_both_exact_destination_boundaries() {
    network_case(false, || {
        let interface = verified("crx0", "10.23.0.1");
        let multicast = MdnsSocket::open(&interface).unwrap();
        let unicast = MdnsUnicastReceiver::open(&interface).unwrap();
        isolated_destinations(multicast, unicast);
    });
}

#[test]
#[ignore = "requires explicit --with-unicast anonymous netns runner, never host LAN"]
fn unicast_then_multicast_preserve_both_exact_destination_boundaries() {
    network_case(false, || {
        let interface = verified("crx0", "10.23.0.1");
        let unicast = MdnsUnicastReceiver::open(&interface).unwrap();
        let multicast = MdnsSocket::open(&interface).unwrap();
        isolated_destinations(multicast, unicast);
    });
}

fn isolated_destinations(mut multicast: MdnsSocket, mut unicast: MdnsUnicastReceiver) {
    send(
        "10.23.0.2",
        MDNS_PORT,
        255,
        &MDNS_GROUP.to_string(),
        b"group",
    );
    assert_eq!(multicast.receive().unwrap().unwrap().bytes, b"group");
    no_packet(unicast.receive());
    send("10.23.0.2", MDNS_PORT, 255, "10.23.0.1", b"direct");
    assert_eq!(unicast.receive().unwrap().unwrap().bytes, b"direct");
    no_packet(multicast.receive());
}

#[test]
#[ignore = "requires explicit --with-unicast anonymous netns runner, never host LAN"]
fn different_exact_local_addresses_do_not_steal_each_others_unicast() {
    network_case(false, || {
        let mut selected = unicast("crx0", "10.23.0.1");
        let mut other = unicast("crx2", "10.24.0.1");
        send("10.24.0.2", MDNS_PORT, 255, "10.24.0.1", b"other");
        assert_eq!(other.receive().unwrap().unwrap().bytes, b"other");
        no_packet(selected.receive());
        send("10.23.0.2", MDNS_PORT, 255, "10.23.0.1", b"selected");
        assert_eq!(selected.receive().unwrap().unwrap().bytes, b"selected");
        no_packet(other.receive());
    });
}

#[test]
#[ignore = "requires explicit --with-unicast anonymous netns runner, never host LAN"]
fn wrong_ttl_port_foreign_source_empty_and_truncated_never_become_observations() {
    network_case(false, || {
        let mut receiver = unicast("crx0", "10.23.0.1");
        for (ip, port, ttl, bytes) in [
            ("10.23.0.2", MDNS_PORT, 254, b"ttl".to_vec()),
            ("10.23.0.2", 5354, 255, b"port".to_vec()),
            ("10.24.0.2", MDNS_PORT, 255, b"foreign".to_vec()),
            ("10.23.0.2", MDNS_PORT, 255, Vec::new()),
            ("10.23.0.2", MDNS_PORT, 255, vec![0; 5000]),
        ] {
            send(ip, port, ttl, "10.23.0.1", &bytes);
            let rejected = receiver.receive();
            if ip == "10.24.0.2" {
                // The kernel may reject a foreign-subnet source before UDP delivery.
                no_packet(rejected);
            } else {
                // Same-subnet delivery must reach the receiver's strict validation,
                // not merely disappear or time out before the positive control.
                assert!(matches!(rejected, Err(MdnsError::InvalidPacket)));
            }
            send("10.23.0.2", MDNS_PORT, 255, "10.23.0.1", b"control");
            assert_eq!(receiver.receive().unwrap().unwrap().bytes, b"control");
        }
    });
}

#[test]
#[ignore = "requires explicit --with-unicast anonymous netns runner, never host LAN"]
fn existing_local_and_wildcard_binds_with_every_reuse_mode_fail_closed() {
    network_case(false, || binding::existing_ipv4_binds_are_exclusive());
}

#[test]
#[ignore = "requires explicit --with-unicast anonymous netns runner, never host LAN"]
fn existing_dual_stack_wildcards_with_every_reuse_mode_fail_closed() {
    network_case(false, || binding::existing_dual_stack_binds_are_exclusive());
}

#[test]
#[ignore = "requires explicit --with-unicast anonymous netns runner, never host LAN"]
fn receiver_rejects_reverse_bind_sharing_and_drop_releases_the_exact_port() {
    network_case(false, || {
        binding::receiver_first_is_exclusive_and_releases()
    });
}

#[test]
#[ignore = "requires explicit --with-unicast anonymous netns runner, never host LAN"]
fn empty_receive_window_is_bounded_and_closes_without_sending() {
    network_case(false, || {
        let mut receiver = unicast("crx0", "10.23.0.1");
        let started = Instant::now();
        assert!(receiver.receive().unwrap().is_none());
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
    });
}

#[test]
#[ignore = "requires explicit anonymous NET_ADMIN mutation fixture, never host LAN"]
fn selected_address_removal_invalidates_the_unicast_receiver_without_fallback() {
    network_case(true, || {
        let interface = verified("crx0", "10.23.0.1");
        let mut receiver = MdnsUnicastReceiver::open(&interface).unwrap();
        assert!(std::process::Command::new("ip")
            .args(["address", "del", "10.23.0.1/24", "dev", "crx0"])
            .status()
            .unwrap()
            .success());
        assert!(matches!(
            receiver.receive(),
            Err(MdnsError::InterfaceChanged)
        ));
        assert!(matches!(
            MdnsUnicastReceiver::open(&interface),
            Err(MdnsError::InterfaceChanged)
        ));
    });
}
