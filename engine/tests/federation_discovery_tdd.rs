//! F0 discovery is untrusted data and pure selection, never peer authentication or a grant.

use cyanrex_engine::{
    models::collaboration::{AuthorityId, QualifiedName},
    services::federation_discovery::{
        ConnectionHint, DiscoveryError, DiscoveryPolicy, NodeFingerprint, PeerAdvertisement,
        ProtocolRange, MAX_ADVERTISEMENT_BYTES, MAX_CAPABILITIES, MAX_FUTURE_SKEW_SECONDS,
        MAX_HINTS, MAX_LEASE_SECONDS,
    },
};
use serde_json::{json, Value};

#[path = "federation_discovery/addresses.rs"]
mod addresses;
#[path = "federation_discovery/selection.rs"]
mod selection;
#[path = "federation_discovery/wire.rs"]
mod wire;

const AUTHORITY: &str = "8b075b0b-8118-4c37-9224-337ba22f40c6";
const NOW: u64 = 1_010;
const TASK: &str = "cyanrex.task.v1";
const EXTRA: &str = "cyanrex.extra.v1";

fn name(value: &str) -> QualifiedName {
    value.parse().unwrap()
}

fn fingerprint(digit: char) -> NodeFingerprint {
    digit.to_string().repeat(64).parse().unwrap()
}

fn range(min: u16, max: u16) -> ProtocolRange {
    ProtocolRange::new(min, max).unwrap()
}

fn policy() -> DiscoveryPolicy {
    DiscoveryPolicy::new(
        fingerprint('a'),
        range(1, 3),
        vec![name(TASK), name(EXTRA)],
        vec![name(TASK)],
    )
    .unwrap()
}

fn advertisement() -> Value {
    json!({
        "schema_version": 1,
        "node_fingerprint": "b".repeat(64),
        "authority_id": AUTHORITY,
        "protocol": {"min": 2, "max": 4},
        "offered_capabilities": [TASK, "cyanrex.optional.v1"],
        "required_capabilities": [TASK],
        "issued_at": 1_000,
        "expires_at": 1_100,
        "hints": [{"kind": "direct", "address": "192.168.1.7:4433"}]
    })
}

fn parse(value: &Value) -> PeerAdvertisement {
    PeerAdvertisement::parse(&serde_json::to_vec(value).unwrap()).unwrap()
}

fn invalid_wire(value: &Value) {
    assert!(matches!(
        PeerAdvertisement::parse(&serde_json::to_vec(value).unwrap()),
        Err(DiscoveryError::InvalidAdvertisement)
    ));
}

fn direct(address: &str) -> ConnectionHint {
    ConnectionHint::Direct(address.parse().unwrap())
}

#[test]
fn f0_exports_fixed_wire_and_resource_limits_without_product_version_coupling() {
    assert_eq!(MAX_ADVERTISEMENT_BYTES, 4_096);
    assert_eq!(MAX_HINTS, 8);
    assert_eq!(MAX_CAPABILITIES, 16);
    assert_eq!(MAX_LEASE_SECONDS, 300);
    assert_eq!(MAX_FUTURE_SKEW_SECONDS, 5);
}

#[test]
fn node_fingerprint_is_only_a_canonical_claim_not_an_allocated_identity() {
    let value = "abcdef0123456789".repeat(4);
    assert_eq!(value.parse::<NodeFingerprint>().unwrap().to_string(), value);
    for invalid in [
        "".to_string(),
        "b".repeat(63),
        "b".repeat(65),
        "B".repeat(64),
        "g".repeat(64),
        format!(" {}", "b".repeat(64)),
        format!("{}\n", "b".repeat(64)),
    ] {
        assert!(invalid.parse::<NodeFingerprint>().is_err());
    }
    assert_eq!(fingerprint('0').to_string(), "0".repeat(64));
}

#[test]
fn protocol_ranges_require_nonzero_ordered_endpoints() {
    assert!(ProtocolRange::new(1, u16::MAX).is_ok());
    assert!(ProtocolRange::new(7, 7).is_ok());
    for (min, max) in [(0, 0), (0, 1), (4, 3)] {
        assert!(matches!(
            ProtocolRange::new(min, max),
            Err(DiscoveryError::InvalidPolicy)
        ));
    }
}
