//! Explicit one-shot probing in owned anonymous network namespaces; never use the host LAN.
#![cfg(target_os = "linux")]

use cyanrex_engine::services::federation_mdns::{MdnsError, ProbeRunObservation, MDNS_GROUP};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[path = "federation_mdns_probing_network/helpers.rs"]
mod helpers;
use helpers::*;

#[test]
#[ignore = "requires explicit --with-probe anonymous netns runner, never host LAN"]
fn a_complete_quiet_round_is_not_an_announcement_or_ownership_permit() {
    network_case(3, || {
        let now = wall();
        let prepared = local(now);
        arm(0, "none", &[]);
        let (result, ended) = run(&prepared, &AtomicBool::new(false));
        assert_eq!(result, Ok(ProbeRunObservation::NoConflictObserved));
        let queries = finish(3, &prepared.encode_probe_at(now).unwrap());
        assert!(ended - queries[2]["ns"].as_u64().unwrap() >= 240_000_000);
    });
}

fn defense(after: u8, destination: &str, competing: bool) {
    network_case(u64::from(after), || {
        let now = wall();
        let prepared = local(now);
        let peer = announcement("node-one", "host-one", "10.23.0.2", now);
        let bytes = if competing {
            peer.encode_probe_at(now).unwrap()
        } else {
            peer.encode_at(now).unwrap()
        };
        arm(after, destination, &bytes);
        let (result, _) = run(&prepared, &AtomicBool::new(false));
        assert_eq!(
            result,
            Ok(if competing {
                ProbeRunObservation::CompetingProbe
            } else {
                ProbeRunObservation::ExistingResponse
            })
        );
        finish(usize::from(after), &prepared.encode_probe_at(now).unwrap());
    });
}

#[test]
#[ignore = "requires explicit --with-probe anonymous netns runner, never host LAN"]
fn qu_unicast_defense_after_first_probe_stops_all_further_sends() {
    defense(1, "10.23.0.1", false);
}

#[test]
#[ignore = "requires explicit --with-probe anonymous netns runner, never host LAN"]
fn qu_unicast_defense_after_second_probe_stops_the_third_send() {
    defense(2, "10.23.0.1", false);
}

#[test]
#[ignore = "requires explicit --with-probe anonymous netns runner, never host LAN"]
fn qu_unicast_defense_after_third_probe_prevents_a_quiet_completion() {
    defense(3, "10.23.0.1", false);
}

#[test]
#[ignore = "requires explicit --with-probe anonymous netns runner, never host LAN"]
fn multicast_defense_stops_the_round_without_announcement() {
    defense(1, &MDNS_GROUP.to_string(), false);
}

#[test]
#[ignore = "requires explicit --with-probe anonymous netns runner, never host LAN"]
fn a_competing_multicast_proposal_stops_without_a_tiebreak_or_retry() {
    defense(1, &MDNS_GROUP.to_string(), true);
}

fn unrelated(destination: &str) {
    network_case(3, || {
        let now = wall();
        let prepared = local(now);
        let other = announcement("unrelated", "unrelated-host", "10.23.0.2", now);
        arm(1, destination, &other.encode_at(now).unwrap());
        let (result, ended) = run(&prepared, &AtomicBool::new(false));
        assert_eq!(result, Ok(ProbeRunObservation::NoConflictObserved));
        let queries = finish(3, &prepared.encode_probe_at(now).unwrap());
        assert!(ended - queries[2]["ns"].as_u64().unwrap() >= 240_000_000);
    });
}

#[test]
#[ignore = "requires explicit --with-probe anonymous netns runner, never host LAN"]
fn unrelated_multicast_records_do_not_become_conflicts() {
    unrelated(&MDNS_GROUP.to_string());
}

#[test]
#[ignore = "requires explicit --with-probe anonymous netns runner, never host LAN"]
fn unrelated_unicast_records_do_not_become_conflicts() {
    unrelated("10.23.0.1");
}

#[test]
#[ignore = "requires explicit --with-probe anonymous netns runner, never host LAN"]
fn an_occupied_local_response_port_fails_before_any_probe_is_sent() {
    network_case(0, || {
        let now = wall();
        let prepared = local(now);
        let occupant = occupy_local_port();
        arm(0, "none", &[]);
        assert_eq!(
            run(&prepared, &AtomicBool::new(false)).0,
            Err(MdnsError::SocketUnavailable)
        );
        finish(0, &prepared.encode_probe_at(now).unwrap());
        drop(occupant);
    });
}

#[test]
#[ignore = "requires explicit --with-probe anonymous netns runner, never host LAN"]
fn pre_cancelled_round_has_no_network_effects_or_open_descriptors() {
    network_case(0, || {
        let now = wall();
        let prepared = local(now);
        arm(0, "none", &[]);
        assert_eq!(
            run(&prepared, &AtomicBool::new(true)).0,
            Err(MdnsError::Cancelled)
        );
        finish(0, &prepared.encode_probe_at(now).unwrap());
    });
}

#[test]
#[ignore = "requires explicit --with-probe anonymous netns runner, never host LAN"]
fn cancellation_after_first_probe_closes_both_channels_without_a_second_send() {
    network_case(1, || {
        let now = wall();
        let prepared = local(now);
        let cancelled = Arc::new(AtomicBool::new(false));
        arm(0, "none", &[]);
        let udp_before = network_counts().0;
        let signal = cancelled.clone();
        let cancelling = thread::spawn(move || {
            let started = Instant::now();
            while started.elapsed() < Duration::from_secs(2) {
                let sent = network_counts().0 - udp_before;
                if sent != 0 {
                    signal.store(true, Ordering::Release);
                    return sent;
                }
                thread::sleep(Duration::from_millis(2));
            }
            0
        });
        assert_eq!(run(&prepared, &cancelled).0, Err(MdnsError::Cancelled));
        assert_eq!(cancelling.join().unwrap(), 1);
        finish(1, &prepared.encode_probe_at(now).unwrap());
    });
}

#[test]
#[ignore = "requires explicit --with-probe anonymous netns runner, never host LAN"]
fn a_frozen_but_now_expired_advertisement_never_sends_a_probe() {
    network_case(0, || {
        let past = wall() - 120;
        let prepared = local(past);
        arm(0, "none", &[]);
        assert!(matches!(
            run(&prepared, &AtomicBool::new(false)).0,
            Err(MdnsError::RejectedAdvertisement(_))
        ));
        finish(0, &prepared.encode_probe_at(past).unwrap());
    });
}

#[test]
#[ignore = "requires explicit --with-probe anonymous netns runner, never host LAN"]
fn a_valid_proposal_larger_than_the_selected_mtu_fails_without_fragmenting() {
    network_case(0, || {
        let output = std::process::Command::new("ip")
            .args(["-j", "link", "show", "dev", "crx0"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let links: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(links.as_array().unwrap().len(), 1);
        assert_eq!(links[0]["ifname"], "crx0");
        assert_eq!(links[0]["mtu"], 1500);
        let now = wall();
        let prepared = oversized_for_fixture_mtu(now);
        arm(0, "none", &[]);
        assert_eq!(
            run(&prepared, &AtomicBool::new(false)).0,
            Err(MdnsError::TooLarge)
        );
        finish(0, &prepared.encode_probe_at(now).unwrap());
    });
}
