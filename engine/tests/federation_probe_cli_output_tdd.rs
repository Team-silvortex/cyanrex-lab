#![cfg(target_os = "linux")]
//! Fixed probe observations never imply identity, ownership or permission.

#[path = "../src/bin/cyanrex-discovery/probe_output.rs"]
mod probe_output;

use cyanrex_engine::services::federation_mdns::ProbeRunObservation;
use serde_json::{json, Value};

#[test]
fn every_observation_has_the_exact_inert_schema_and_one_newline() {
    assert_eq!(probe_output::MAX_OUTPUT_BYTES, 512);
    for (observation, text) in [
        (
            ProbeRunObservation::NoConflictObserved,
            "no_conflict_observed",
        ),
        (ProbeRunObservation::ExistingResponse, "existing_response"),
        (ProbeRunObservation::CompetingProbe, "competing_probe"),
    ] {
        let bytes = probe_output::encode(observation).unwrap();
        assert!(bytes.len() <= probe_output::MAX_OUTPUT_BYTES);
        assert_eq!(bytes.last(), Some(&b'\n'));
        assert_eq!(bytes.iter().filter(|byte| **byte == b'\n').count(), 1);
        assert_eq!(
            serde_json::from_slice::<Value>(&bytes).unwrap(),
            json!({
                "schema_version":1,"kind":"unverified_lan_probe_observation",
                "observation_only":true,"verified":false,"name_ownership_verified":false,
                "publication_permitted":false,"announcement_sent":false,"observation":text
            })
        );
    }
}

#[test]
fn output_contains_no_candidate_or_reusable_lease_and_no_fixed_probe_count() {
    let bytes = probe_output::encode(ProbeRunObservation::NoConflictObserved).unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    let object = value.as_object().unwrap();
    assert_eq!(object.len(), 8);
    for field in [
        "candidates",
        "identity",
        "owner",
        "available",
        "lease",
        "expires_at",
        "deadline",
        "node_fingerprint",
        "authority_id",
        "probes_sent",
        "raw",
        "advertisement",
        "error",
    ] {
        assert!(!object.contains_key(field));
    }
}
