//! Bounded display of inert claims. No timestamp here is an effective DNS lease or permission.

use cyanrex_engine::{
    models::collaboration::{AuthorityId, QualifiedName},
    services::federation_discovery::{
        lan::{LanDiscoveredCandidate, MAX_LAN_CANDIDATES},
        ConnectionHint, MAX_CAPABILITIES, MAX_HINTS,
    },
};
use serde::Serialize;
use std::io::{self, Write};

pub const MAX_OUTPUT_BYTES: usize = 256 * 1024;

#[derive(Serialize)]
struct Snapshot<'a> {
    schema_version: u8,
    kind: &'static str,
    snapshot_only: bool,
    verified: bool,
    candidates: Vec<Candidate<'a>>,
}

#[derive(Serialize)]
struct Candidate<'a> {
    verified: bool,
    instance: &'a str,
    claimed_node_fingerprint: &'a str,
    claimed_authority_id: AuthorityId,
    negotiated_protocol: u16,
    compatible_capabilities: &'a [QualifiedName],
    connection_hints: Vec<String>,
    // Only the remote F0 declaration: deliberately not a DNS deadline or reusable lease.
    advertisement_expires_at: u64,
}

/// Complete JSON plus one newline, or a fixed failure with no partial output returned.
/// Input order and separate source observations are retained even when their claims match.
pub fn encode(candidates: &[LanDiscoveredCandidate]) -> Result<Vec<u8>, ()> {
    if candidates.len() > MAX_LAN_CANDIDATES {
        return Err(());
    }
    let mut peers = Vec::with_capacity(candidates.len());
    for observation in candidates {
        let candidate = observation.candidate();
        if candidate.capabilities().len() > MAX_CAPABILITIES
            || candidate.hints().is_empty()
            || candidate.hints().len() > MAX_HINTS
        {
            return Err(());
        }
        let connection_hints = candidate
            .hints()
            .iter()
            .map(|hint| match hint {
                ConnectionHint::Direct(std::net::SocketAddr::V4(address)) => {
                    Ok(address.to_string())
                }
                // This CLI's transport profile is IPv4 direct only; never silently drop hints.
                ConnectionHint::Direct(std::net::SocketAddr::V6(_)) | ConnectionHint::Relay(_) => {
                    Err(())
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        peers.push(Candidate {
            verified: false,
            instance: observation.source().instance(),
            claimed_node_fingerprint: candidate.claimed_node().as_str(),
            claimed_authority_id: candidate.claimed_authority(),
            negotiated_protocol: candidate.protocol(),
            compatible_capabilities: candidate.capabilities(),
            connection_hints,
            advertisement_expires_at: candidate.expires_at(),
        });
    }
    let snapshot = Snapshot {
        schema_version: 1,
        kind: "unverified_lan_snapshot",
        snapshot_only: true,
        verified: false,
        candidates: peers,
    };
    let mut output = BoundedOutput(Vec::new());
    serde_json::to_writer(&mut output, &snapshot).map_err(|_| ())?;
    output.write_all(b"\n").map_err(|_| ())?;
    Ok(output.0)
}

/// Reject excess bytes before reserving or appending them. Even the newline consumes the budget.
struct BoundedOutput(Vec<u8>);

impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let remaining = MAX_OUTPUT_BYTES - self.0.len();
        if bytes.len() > remaining {
            return Err(io::Error::other("discovery output exceeds its bound"));
        }
        let length = self.0.len() + bytes.len();
        if length > self.0.capacity() {
            // Geometric growth avoids one allocation per serializer fragment. The limit is a
            // power of two, so every requested capacity remains within the same fixed bound.
            let capacity = length.next_power_of_two().max(1024).min(MAX_OUTPUT_BYTES);
            self.0
                .try_reserve_exact(capacity - self.0.len())
                .map_err(|_| io::Error::other("discovery output unavailable"))?;
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_output_budget_is_allowed_but_one_more_byte_is_not_appended() {
        let mut output = BoundedOutput(Vec::new());
        output.write_all(&vec![b'a'; MAX_OUTPUT_BYTES]).unwrap();
        assert_eq!(output.0.len(), MAX_OUTPUT_BYTES);
        assert!(output.write_all(b"x").is_err());
        assert_eq!(output.0, vec![b'a'; MAX_OUTPUT_BYTES]);
    }

    #[test]
    fn oversized_first_chunk_is_rejected_without_allocating_output() {
        let mut output = BoundedOutput(Vec::new());
        assert!(output.write_all(&vec![b'a'; MAX_OUTPUT_BYTES + 1]).is_err());
        assert!(output.0.is_empty());
        assert_eq!(output.0.capacity(), 0);
    }

    #[test]
    fn incremental_writes_include_the_terminal_newline_in_the_same_budget() {
        let mut output = BoundedOutput(Vec::new());
        output.write_all(&vec![b'a'; MAX_OUTPUT_BYTES - 1]).unwrap();
        output.write_all(b"\n").unwrap();
        assert!(output.write_all(b"\n").is_err());
        assert_eq!(output.0.len(), MAX_OUTPUT_BYTES);
        assert_eq!(output.0.last(), Some(&b'\n'));
        output.flush().unwrap();
    }
}
