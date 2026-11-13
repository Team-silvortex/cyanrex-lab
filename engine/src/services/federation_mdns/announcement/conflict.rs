//! Pure response-record comparison, never proof of identity, name ownership or sending authority.
use super::{
    records::{FrozenRecords, MatchRecord, RecordKind},
    MdnsAnnouncement, MdnsError, Result,
};
use crate::services::federation_mdns::wire;

/// Only the three frozen unique IN tuples are compared; absence confers no publication permit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseConflictObservation {
    NoConflictingRecord,
    ConflictingRecord,
}

impl MdnsAnnouncement {
    /// Inspect one complete response with standard opcode and no error, independently of probe policy.
    /// ID and unrelated header bits are ignored; complete packet framing is still required.
    /// Validate every known IN A/PTR/TXT/SRV record before returning the accumulated observation.
    /// Unknown types remain bounded opaque data. Identical records and shared PTRs do not conflict;
    /// TTL zero and cache-flush neither excuse differing data nor establish another owner's claim.
    /// NoConflictingRecord does not exclude unknown/cross-type conflicts or required TTL rescue.
    /// Zero-length TXT is accepted. Query handling, source/time checks and all live action are separate.
    pub fn observe_response_conflict(&self, bytes: &[u8]) -> Result<ResponseConflictObservation> {
        let message = wire::read_message(bytes)?;
        if message.flags & 0x8000 == 0 || message.flags & 0x7800 != 0 || message.flags & 0x000f != 0
        {
            return Err(MdnsError::InvalidPacket);
        }
        let frozen = FrozenRecords::new(self)?;
        let mut conflicting = false;
        for record in &message.records {
            if let MatchRecord::Different(kind) = frozen.match_record(bytes, record)? {
                // Shared PTR differences are never conflicts for the three unique frozen tuples.
                conflicting |= kind != RecordKind::Ptr;
            }
        }
        Ok(if conflicting {
            ResponseConflictObservation::ConflictingRecord
        } else {
            ResponseConflictObservation::NoConflictingRecord
        })
    }
}
