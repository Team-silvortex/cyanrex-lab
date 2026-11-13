//! Fixed historical observations only, never name ownership, identity or publication authority.
use cyanrex_engine::services::federation_mdns::ProbeRunObservation;
use serde::Serialize;

pub const MAX_OUTPUT_BYTES: usize = 512;

#[derive(Serialize)]
struct Observation {
    schema_version: u8,
    kind: &'static str,
    observation_only: bool,
    verified: bool,
    name_ownership_verified: bool,
    publication_permitted: bool,
    announcement_sent: bool,
    observation: &'static str,
}

/// Buffer one complete fixed-schema JSON plus newline before any stdout write.
pub fn encode(observation: ProbeRunObservation) -> Result<Vec<u8>, ()> {
    let observation = match observation {
        ProbeRunObservation::NoConflictObserved => "no_conflict_observed",
        ProbeRunObservation::ExistingResponse => "existing_response",
        ProbeRunObservation::CompetingProbe => "competing_probe",
    };
    let value = Observation {
        schema_version: 1,
        kind: "unverified_lan_probe_observation",
        observation_only: true,
        verified: false,
        name_ownership_verified: false,
        publication_permitted: false,
        announcement_sent: false,
        observation,
    };
    // All fields are fixed literals and this closed enum: no input bytes reach the serializer.
    let mut bytes = serde_json::to_vec(&value).map_err(|_| ())?;
    if bytes.len() >= MAX_OUTPUT_BYTES {
        return Err(());
    }
    bytes.push(b'\n');
    Ok(bytes)
}
