//! F1a is bounded LAN discovery data intake, not a socket, pairing or authority service.

use cyanrex_engine::{
    models::collaboration::QualifiedName,
    services::federation_discovery::{
        lan::{
            CacheTime, LanCandidateCache, LanDiscoveryError, LanObservation, LanSource,
            MAX_LAN_CANDIDATES, MAX_LAN_EVENTS_PER_SECOND, MAX_LAN_INTERFACES, MAX_LAN_TTL_SECONDS,
        },
        DiscoveryError, DiscoveryPolicy, NodeFingerprint, ProtocolRange,
    },
};
use serde_json::{json, Value};

#[path = "federation_lan/cache.rs"]
mod cache;
#[path = "federation_lan/clock.rs"]
mod clock;
#[path = "federation_lan/source.rs"]
mod source;

const AUTHORITY: &str = "8b075b0b-8118-4c37-9224-337ba22f40c6";
const OTHER_AUTHORITY: &str = "267c6b0f-182c-4980-a925-cdc967327c57";
const TASK: &str = "cyanrex.task.v1";

fn fingerprint(digit: char) -> NodeFingerprint {
    digit.to_string().repeat(64).parse().unwrap()
}

fn name(value: &str) -> QualifiedName {
    value.parse().unwrap()
}

fn policy() -> DiscoveryPolicy {
    DiscoveryPolicy::new(
        fingerprint('a'),
        ProtocolRange::new(1, 3).unwrap(),
        vec![name(TASK)],
        vec![name(TASK)],
    )
    .unwrap()
}

fn configured(capacity: usize) -> LanCandidateCache {
    LanCandidateCache::new(policy(), vec![11, 22], capacity).unwrap()
}

fn source(epoch: u64, instance: &str) -> LanSource {
    LanSource::new(epoch, instance).unwrap()
}

fn time(unix: u64, monotonic: u64) -> CacheTime {
    CacheTime::new(unix, monotonic).unwrap()
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

fn raw(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

fn add(cache: &mut LanCandidateCache, source: &LanSource) {
    assert!(matches!(
        cache.observe(source, &raw(&advertisement()), 30, time(1_010, 200)),
        Ok(LanObservation::Added)
    ));
}

#[test]
fn lan_data_intake_has_fixed_bounded_configuration_not_a_live_listener() {
    assert_eq!(MAX_LAN_CANDIDATES, 64);
    assert_eq!(MAX_LAN_INTERFACES, 8);
    assert_eq!(MAX_LAN_TTL_SECONDS, 120);
    assert_eq!(MAX_LAN_EVENTS_PER_SECOND, 64);
}
