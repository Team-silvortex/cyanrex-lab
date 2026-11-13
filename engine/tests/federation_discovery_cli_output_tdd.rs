#![cfg(target_os = "linux")]
//! The CLI emits inert unverified snapshots, not reusable leases or connection authority.

#[path = "../src/bin/cyanrex-discovery/output.rs"]
mod output;

use cyanrex_engine::{
    models::collaboration::QualifiedName,
    services::federation_discovery::{
        lan::{CacheTime, LanCandidateCache, LanDiscoveredCandidate, LanSource},
        DiscoveryPolicy, ProtocolRange,
    },
};
use serde_json::{json, Value};

const AUTHORITY: &str = "8b075b0b-8118-4c37-9224-337ba22f40c6";

fn names(values: &[String]) -> Vec<QualifiedName> {
    values.iter().map(|value| value.parse().unwrap()).collect()
}

fn candidates(
    instances: &[&str],
    hints: Value,
    capabilities: &[String],
) -> Vec<LanDiscoveredCandidate> {
    let policy = DiscoveryPolicy::new(
        "a".repeat(64).parse().unwrap(),
        ProtocolRange::new(1, 3).unwrap(),
        names(capabilities),
        vec![],
    )
    .unwrap()
    .with_relays(vec!["lab.home".parse().unwrap()])
    .unwrap();
    let mut cache = LanCandidateCache::new(policy, vec![11], 64).unwrap();
    let advertisement = serde_json::to_vec(&json!({
        "schema_version": 1,
        "node_fingerprint": "b".repeat(64),
        "authority_id": AUTHORITY,
        "protocol": {"min": 2, "max": 4},
        "offered_capabilities": capabilities,
        "required_capabilities": [],
        "issued_at": 1_000,
        "expires_at": 1_100,
        "hints": hints
    }))
    .unwrap();
    for instance in instances {
        cache
            .observe(
                &LanSource::new(11, instance).unwrap(),
                &advertisement,
                30,
                CacheTime::new(1_010, 200).unwrap(),
            )
            .unwrap();
    }
    cache.snapshot(CacheTime::new(1_010, 200).unwrap()).unwrap()
}

fn direct() -> Value {
    json!([{"kind": "direct", "address": "192.168.1.7:4433"}])
}

fn decode(bytes: &[u8]) -> Value {
    assert_eq!(bytes.last(), Some(&b'\n'));
    assert_eq!(bytes.iter().filter(|byte| **byte == b'\n').count(), 1);
    serde_json::from_slice(bytes).unwrap()
}

#[test]
fn output_budget_and_empty_snapshot_have_an_exact_versioned_schema() {
    assert_eq!(output::MAX_OUTPUT_BYTES, 256 * 1024);
    assert_eq!(
        decode(&output::encode(&[]).unwrap()),
        json!({
            "schema_version": 1,
            "kind": "unverified_lan_snapshot",
            "snapshot_only": true,
            "verified": false,
            "candidates": []
        })
    );
}

#[test]
fn all_peer_fields_are_claims_or_compatibility_not_verified_authority() {
    let peers = candidates(&["peer"], direct(), &["cyanrex.task.v1".to_owned()]);
    let value = decode(&output::encode(&peers).unwrap());
    assert_eq!(
        value["candidates"],
        json!([{
            "verified": false,
            "instance": "peer",
            "claimed_node_fingerprint": "b".repeat(64),
            "claimed_authority_id": AUTHORITY,
            "negotiated_protocol": 3,
            "compatible_capabilities": ["cyanrex.task.v1"],
            "connection_hints": ["192.168.1.7:4433"],
            "advertisement_expires_at": 1_100
        }])
    );
    assert_eq!(value.as_object().unwrap().len(), 5);
}

#[test]
fn advertisement_expiry_is_not_replaced_with_an_epoch_or_effective_dns_deadline() {
    let peers = candidates(&["peer"], direct(), &[]);
    assert_eq!(peers[0].deadline_monotonic(), 230);
    let bytes = output::encode(&peers).unwrap();
    let value = decode(&bytes);
    assert_eq!(value["candidates"][0]["advertisement_expires_at"], 1_100);
    let object = value["candidates"][0].as_object().unwrap();
    for forbidden in [
        "epoch",
        "interface_epoch",
        "deadline_monotonic",
        "expires_at",
        "ttl",
        "lease",
        "raw",
        "advertisement",
        "txt",
        "authority_id",
        "node_fingerprint",
        "error",
    ] {
        assert!(
            !object.contains_key(forbidden),
            "unexpected field {forbidden}"
        );
    }
    assert_eq!(object.len(), 8);
}

#[test]
fn snapshot_order_and_distinct_sources_survive_identical_claims() {
    let mut peers = candidates(&["z-peer", "a-peer", "m-peer"], direct(), &[]);
    // Repeated visible fields must not become an implicit source-identity merge either.
    peers.push(peers[0].clone());
    let value = decode(&output::encode(&peers).unwrap());
    let instances: Vec<_> = value["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|peer| peer["instance"].as_str().unwrap())
        .collect();
    assert_eq!(instances, ["z-peer", "a-peer", "m-peer", "z-peer"]);
}

#[test]
fn capability_and_ipv4_hint_order_are_preserved_without_grants_or_urls() {
    let peers = candidates(
        &["peer"],
        json!([
            {"kind":"direct", "address":"192.168.1.7:4434"},
            {"kind":"direct", "address":"192.168.1.7:4433"}
        ]),
        &["zeta.payload".to_owned(), "alpha.payload".to_owned()],
    );
    let value = decode(&output::encode(&peers).unwrap());
    assert_eq!(
        value["candidates"][0]["compatible_capabilities"],
        json!(["zeta.payload", "alpha.payload"])
    );
    assert_eq!(
        value["candidates"][0]["connection_hints"],
        json!(["192.168.1.7:4434", "192.168.1.7:4433"])
    );
}

#[test]
fn ipv6_and_relay_hints_reject_the_entire_output_instead_of_silent_filtering() {
    for hints in [
        json!([{"kind":"direct", "address":"[fd12:3456::7]:4433"}]),
        json!([{"kind":"relay", "relay_id":"lab.home"}]),
        json!([
            {"kind":"direct", "address":"192.168.1.7:4433"},
            {"kind":"relay", "relay_id":"lab.home"}
        ]),
    ] {
        let mut peers = candidates(&["valid"], direct(), &[]);
        peers.extend(candidates(&["outside-profile"], hints, &[]));
        assert_eq!(output::encode(&peers), Err(()));
    }
}

#[test]
fn sixty_four_candidates_are_not_deduplicated_but_sixty_five_are_rejected() {
    let labels: Vec<_> = (0..64).map(|index| format!("peer-{index}")).collect();
    let refs: Vec<_> = labels.iter().map(String::as_str).collect();
    let mut peers = candidates(&refs, direct(), &[]);
    let value = decode(&output::encode(&peers).unwrap());
    assert_eq!(value["candidates"].as_array().unwrap().len(), 64);
    peers.push(peers[0].clone());
    assert_eq!(output::encode(&peers), Err(()));
}

#[test]
fn maximal_supported_fields_are_complete_within_the_output_byte_budget() {
    let capabilities: Vec<_> = (0..16)
        .map(|index| format!("feature.{}{:02}", "a".repeat(118), index))
        .collect();
    assert!(capabilities.iter().all(|name| name.len() == 128));
    let hints = Value::Array(
        (4433..4441)
            .map(|port| json!({"kind":"direct","address":format!("192.168.1.7:{port}")}))
            .collect(),
    );
    let labels: Vec<_> = (0..64)
        .map(|index| format!("{}{:02}", "p".repeat(61), index))
        .collect();
    let refs: Vec<_> = labels.iter().map(String::as_str).collect();
    let bytes = output::encode(&candidates(&refs, hints, &capabilities)).unwrap();
    assert!(bytes.len() <= output::MAX_OUTPUT_BYTES);
    let value = decode(&bytes);
    let peers = value["candidates"].as_array().unwrap();
    assert_eq!(peers.len(), 64);
    for (index, peer) in peers.iter().enumerate() {
        assert_eq!(peer["instance"], labels[index]);
        assert_eq!(peer["compatible_capabilities"], json!(capabilities));
        assert_eq!(peer["connection_hints"].as_array().unwrap().len(), 8);
    }
}
