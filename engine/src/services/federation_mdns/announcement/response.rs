//! Bounded frozen query-response preparation, never a live responder or permission to send.
use super::{
    query::QueryAnalysis, records::RecordKind, MdnsAnnouncement, MdnsQueryContext,
    MdnsQueryRecordSet, Packet, Result,
};

/// Complete prepared bytes can become stale immediately; no variant proves ownership or delivery.
#[derive(Debug, PartialEq, Eq)]
pub enum MdnsQueryResponse {
    NoMatchingQuestion,
    AllDirectAnswersSuppressed,
    RequiresContext(MdnsQueryContext),
    Packet(Vec<u8>),
}

impl MdnsAnnouncement {
    /// Reanalyze one raw query and prepare only frozen positive Answer and Additional records.
    /// Caller-supplied source metadata and time are not ingress proof or a reusable sending lease.
    /// Additional dependencies come only from surviving direct answers and obey the same KAS sample.
    /// No MTU policy, scheduling, interface validation, negative answer or I/O is supplied here.
    pub fn prepare_query_response(
        &self,
        bytes: &[u8],
        source_port: u16,
        multicast_destination: bool,
        now: u64,
    ) -> Result<MdnsQueryResponse> {
        let (requested, known_suppressed, ttl) =
            match self.analyze_query(bytes, source_port, multicast_destination, now)? {
                QueryAnalysis::RequiresContext(context) => {
                    return Ok(MdnsQueryResponse::RequiresContext(context));
                }
                QueryAnalysis::Multicast {
                    requested,
                    known_suppressed,
                    ttl,
                } => (requested, known_suppressed, ttl),
            };
        if requested.count() == 0 {
            return Ok(MdnsQueryResponse::NoMatchingQuestion);
        }
        let answers = requested.difference(known_suppressed);
        if answers.count() == 0 {
            return Ok(MdnsQueryResponse::AllDirectAnswersSuppressed);
        }
        let dependencies = MdnsQueryRecordSet {
            ptr: false,
            srv: answers.ptr,
            txt: answers.ptr,
            a: answers.ptr || answers.srv,
        };
        let additional = dependencies
            .difference(known_suppressed)
            .difference(answers);
        let mut packet = Packet(Vec::new());
        packet.append(&[0, 0, 0x84, 0, 0, 0])?;
        packet.append(&answers.count().to_be_bytes())?;
        packet.append(&[0, 0])?;
        packet.append(&additional.count().to_be_bytes())?;
        for selected in [answers, additional] {
            for kind in [
                RecordKind::Ptr,
                RecordKind::Srv,
                RecordKind::Txt,
                RecordKind::A,
            ] {
                if selected.contains(kind) {
                    let class = if kind == RecordKind::Ptr { 1 } else { 0x8001 };
                    self.frozen_record(&mut packet, kind, class, ttl)?;
                }
            }
        }
        Ok(MdnsQueryResponse::Packet(packet.0))
    }
}
