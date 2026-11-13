//! Pure per-packet evidence, never a name-availability result, winner or announcement permit.
//! All bytes are untrusted. A future adapter must supply complete live ingress and its provenance.
use super::{MdnsAnnouncement, MdnsError, Packet, Result};
use crate::services::federation_mdns::wire::{self, Section};

/// Conservative claims only; errors and absence must never become permission to use a name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeObservation {
    NoRelevantClaim,
    ExistingResponse,
    CompetingProbe,
}

impl MdnsAnnouncement {
    /// Prepare ANY/QU questions for both names and all three frozen unique Authority records.
    /// No PTR or cache-flush bits: probing proposes data, not an assertion of name ownership.
    pub fn encode_probe_at(&self, now: u64) -> Result<Vec<u8>> {
        let ttl = self.ttl_at(now)?;
        let mut packet = Packet(Vec::new());
        packet.append(&[0, 0, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0])?;
        for owner in [&self.instance, &self.host] {
            packet.append(owner)?;
            packet.append(&255_u16.to_be_bytes())?;
            packet.append(&0x8001_u16.to_be_bytes())?;
        }
        self.unique_records(&mut packet, 1, ttl)?;
        Ok(packet.0)
    }

    /// Inspect one complete bounded message, independently of candidate/TXT compatibility.
    /// Any same-name IN response record is conservative evidence, even identical, unknown or TTL0.
    /// Query Answer/Additional records are not authoritative probe proposals. Related Authority
    /// records must match an IN question; a malformed proposal fails, never returns absence.
    /// No semantic RDATA comparison, tiebreak, cache, timer, source proof or success is supplied.
    pub fn observe_probe_packet(&self, bytes: &[u8]) -> Result<ProbeObservation> {
        let message = wire::read_message(bytes)?;
        if message.id != 0 || !matches!(message.flags, 0 | 0x8000 | 0x8400) {
            return Err(MdnsError::InvalidPacket);
        }
        let protected = [
            wire::complete_name(&self.instance, 0, self.instance.len())?,
            wire::complete_name(&self.host, 0, self.host.len())?,
        ];
        let relevant =
            |record: &wire::Record| record.class() == 1 && protected.contains(&record.owner);
        if message.flags & 0x8000 != 0 {
            return Ok(if message.records.iter().any(relevant) {
                ProbeObservation::ExistingResponse
            } else {
                ProbeObservation::NoRelevantClaim
            });
        }
        let mut competing = false;
        for record in message
            .records
            .iter()
            .filter(|record| record.section == Section::Authority && relevant(record))
        {
            if !message.questions.iter().any(|question| {
                question.name == record.owner
                    && question.class() == 1
                    && (question.kind == 255 || question.kind == record.kind)
            }) {
                return Err(MdnsError::InvalidPacket);
            }
            competing = true;
        }
        Ok(if competing {
            ProbeObservation::CompetingProbe
        } else {
            ProbeObservation::NoRelevantClaim
        })
    }
}
