//! Pure single-packet direct-answer selection, never provenance, publication or sending authority.
use super::{
    records::{FrozenRecords, MatchRecord, RecordKind},
    MdnsAnnouncement, MdnsError, Result,
};
use crate::services::federation_mdns::wire::{self, Section};

/// Only the four frozen direct records; this does not select DNS-SD additional records.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MdnsQueryRecordSet {
    pub ptr: bool,
    pub srv: bool,
    pub txt: bool,
    pub a: bool,
}

impl MdnsQueryRecordSet {
    fn insert(&mut self, kind: RecordKind) {
        match kind {
            RecordKind::Ptr => self.ptr = true,
            RecordKind::Srv => self.srv = true,
            RecordKind::Txt => self.txt = true,
            RecordKind::A => self.a = true,
        }
    }

    fn intersect(self, other: Self) -> Self {
        Self {
            ptr: self.ptr && other.ptr,
            srv: self.srv && other.srv,
            txt: self.txt && other.txt,
            a: self.a && other.a,
        }
    }

    pub(super) fn difference(self, other: Self) -> Self {
        Self {
            ptr: self.ptr && !other.ptr,
            srv: self.srv && !other.srv,
            txt: self.txt && !other.txt,
            a: self.a && !other.a,
        }
    }

    pub(super) fn contains(self, kind: RecordKind) -> bool {
        match kind {
            RecordKind::Ptr => self.ptr,
            RecordKind::Srv => self.srv,
            RecordKind::Txt => self.txt,
            RecordKind::A => self.a,
        }
    }

    pub(super) fn count(self) -> u16 {
        u16::from(self.ptr) + u16::from(self.srv) + u16::from(self.txt) + u16::from(self.a)
    }
}

/// Any set flag needs processing outside this single multicast QM packet model.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MdnsQueryContext {
    pub truncated: bool,
    pub unicast_question: bool,
    pub authority_records: bool,
    pub legacy_port: bool,
    pub direct_unicast: bool,
    pub no_questions: bool,
}

/// Empty requested and fully suppressed requested sets are distinct observations, not send permits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdnsQueryPlan {
    Multicast {
        requested: MdnsQueryRecordSet,
        suppressed: MdnsQueryRecordSet,
    },
    RequiresContext(MdnsQueryContext),
}

/// One validated sample, retaining suppression for all frozen records, not only direct questions.
pub(super) enum QueryAnalysis {
    Multicast {
        requested: MdnsQueryRecordSet,
        known_suppressed: MdnsQueryRecordSet,
        ttl: u32,
    },
    RequiresContext(MdnsQueryContext),
}

impl MdnsAnnouncement {
    /// Plan only direct QM IN/ANY answers to one complete bounded query at a supplied wall sample.
    /// Source port and destination are caller declarations, not checked live ingress metadata.
    /// All known IN data is validated before a context result; unknown/non-IN data stays opaque.
    /// Known-answer suppression uses only equal Answer records at half the current bounded TTL.
    /// A later sender still needs ownership, fresh clocks/encoding and actual egress validation.
    pub fn plan_query(
        &self,
        bytes: &[u8],
        source_port: u16,
        multicast_destination: bool,
        now: u64,
    ) -> Result<MdnsQueryPlan> {
        Ok(
            match self.analyze_query(bytes, source_port, multicast_destination, now)? {
                QueryAnalysis::RequiresContext(context) => MdnsQueryPlan::RequiresContext(context),
                QueryAnalysis::Multicast {
                    requested,
                    known_suppressed,
                    ..
                } => MdnsQueryPlan::Multicast {
                    requested,
                    suppressed: known_suppressed.intersect(requested),
                },
            },
        )
    }

    pub(super) fn analyze_query(
        &self,
        bytes: &[u8],
        source_port: u16,
        multicast_destination: bool,
        now: u64,
    ) -> Result<QueryAnalysis> {
        if source_port == 0 {
            return Err(MdnsError::InvalidConfig);
        }
        let message = wire::read_message(bytes)?;
        if message.flags & 0x8000 != 0 || message.flags & 0x7800 != 0 || message.flags & 0x000f != 0
        {
            return Err(MdnsError::InvalidPacket);
        }
        let ttl = self.ttl_at(now)?;
        let frozen = FrozenRecords::new(self)?;
        let mut known_suppressed = MdnsQueryRecordSet::default();
        let mut invalid_answer_flush = false;
        for record in &message.records {
            let comparison = frozen.match_record(bytes, record)?;
            if record.section == Section::Answer {
                // Strict query profile only: receive-side F1m still ignores cache-flush in equality.
                invalid_answer_flush |= record.class() == 1
                    && RecordKind::from_code(record.kind).is_some()
                    && record.raw_class & 0x8000 != 0;
                if u64::from(record.ttl) * 2 >= u64::from(ttl) {
                    if let MatchRecord::Equal(kind) = comparison {
                        known_suppressed.insert(kind);
                    }
                }
            }
        }
        if invalid_answer_flush {
            return Err(MdnsError::InvalidPacket);
        }
        let context = MdnsQueryContext {
            truncated: message.flags & 0x0200 != 0,
            unicast_question: message.questions.iter().any(|q| q.raw_class & 0x8000 != 0),
            authority_records: message
                .records
                .iter()
                .any(|r| r.section == Section::Authority),
            legacy_port: source_port != 5353,
            direct_unicast: !multicast_destination,
            no_questions: message.questions.is_empty(),
        };
        if context != MdnsQueryContext::default() {
            return Ok(QueryAnalysis::RequiresContext(context));
        }
        let mut requested = MdnsQueryRecordSet::default();
        for question in message
            .questions
            .iter()
            .filter(|q| matches!(q.class(), 1 | 255))
        {
            for kind in [
                RecordKind::Ptr,
                RecordKind::Srv,
                RecordKind::Txt,
                RecordKind::A,
            ] {
                if question.name == *frozen.owner(kind)
                    && (question.kind == 255 || question.kind == kind.code())
                {
                    requested.insert(kind);
                }
            }
        }
        Ok(QueryAnalysis::Multicast {
            requested,
            known_suppressed,
            ttl,
        })
    }
}
