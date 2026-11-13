//! Pure frozen response preparation; no socket, ownership, MTU or publication permission.
#![cfg(target_os = "linux")]
use cyanrex_engine::services::{
    federation_discovery::{lan::encode_txt, PeerAdvertisement},
    federation_mdns::{
        browse_query, MdnsAnnouncement, MdnsError, MdnsQueryContext, MdnsQueryPlan,
        MdnsQueryRecordSet, MdnsQueryResponse,
    },
};

const SERVICE: &str = "_cyanrex._udp.local.";
const INSTANCE: &str = "node-one._cyanrex._udp.local.";
const HOST: &str = "host-one.local.";

#[path = "federation_mdns_query_response/selection.rs"]
mod selection;
#[path = "federation_mdns_query_response/time_and_wire.rs"]
mod time_and_wire;
#[path = "federation_mdns_query_response/validation.rs"]
mod validation;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Record {
    owner: Vec<u8>,
    kind: u16,
    class: u16,
    ttl: u32,
    data: Vec<u8>,
}

fn name(value: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    for label in value.trim_end_matches('.').split('.') {
        bytes.push(label.len() as u8);
        bytes.extend(label.as_bytes());
    }
    bytes.push(0);
    bytes
}
fn advertisement() -> PeerAdvertisement {
    let raw = serde_json::json!({
        "schema_version":1,"node_fingerprint":"b".repeat(64),
        "authority_id":"8b075b0b-8118-4c37-9224-337ba22f40c6",
        "protocol":{"min":1,"max":3},"offered_capabilities":["peer.identity"],
        "required_capabilities":[],"issued_at":1000,"expires_at":1100,
        "hints":[{"kind":"direct","address":"192.168.1.7:4433"}]
    });
    PeerAdvertisement::parse(&serde_json::to_vec(&raw).unwrap()).unwrap()
}
fn announcement(ttl: u32) -> MdnsAnnouncement {
    MdnsAnnouncement::new(
        "node-one",
        "host-one",
        "192.168.1.7:4433".parse().unwrap(),
        &advertisement(),
        1000,
        ttl,
    )
    .unwrap()
}
fn local(kind: u16, ttl: u32) -> Record {
    let (owner, data) = match kind {
        12 => (SERVICE, name(INSTANCE)),
        33 => {
            let mut data = vec![0, 0, 0, 0];
            data.extend(4433_u16.to_be_bytes());
            data.extend(name(HOST));
            (INSTANCE, data)
        }
        16 => (INSTANCE, encode_txt(&advertisement()).unwrap()),
        1 => (HOST, vec![192, 168, 1, 7]),
        _ => panic!("four frozen tuples only"),
    };
    Record {
        owner: name(owner),
        kind,
        class: 1,
        ttl,
        data,
    }
}
fn append(bytes: &mut Vec<u8>, rr: &Record) {
    bytes.extend(&rr.owner);
    bytes.extend(rr.kind.to_be_bytes());
    bytes.extend(rr.class.to_be_bytes());
    bytes.extend(rr.ttl.to_be_bytes());
    bytes.extend((rr.data.len() as u16).to_be_bytes());
    bytes.extend(&rr.data);
}
fn query(questions: &[(&str, u16, u16)], known: &[Record]) -> Vec<u8> {
    let mut bytes = vec![0, 0, 0, 0];
    for count in [questions.len(), known.len(), 0, 0] {
        bytes.extend((count as u16).to_be_bytes());
    }
    for (owner, kind, class) in questions {
        bytes.extend(name(owner));
        bytes.extend(kind.to_be_bytes());
        bytes.extend(class.to_be_bytes());
    }
    for rr in known {
        append(&mut bytes, rr);
    }
    bytes
}
fn prepare(bytes: &[u8], now: u64) -> Result<MdnsQueryResponse, MdnsError> {
    announcement(100).prepare_query_response(bytes, 5353, true, now)
}
fn packet(bytes: &[u8], now: u64) -> Vec<u8> {
    match prepare(bytes, now).unwrap() {
        MdnsQueryResponse::Packet(bytes) => bytes,
        other => panic!("expected prepared bytes, got {other:?}"),
    }
}

// Intentionally independent uncompressed inspector: compressed input must never be copied out.
fn take_name(bytes: &[u8], offset: &mut usize) -> Vec<u8> {
    let start = *offset;
    loop {
        let length = bytes[*offset] as usize;
        assert!(length <= 63, "output must use frozen uncompressed names");
        *offset += 1;
        if length == 0 {
            break;
        }
        *offset += length;
        assert!(*offset <= bytes.len());
    }
    bytes[start..*offset].to_vec()
}
fn word(bytes: &[u8], offset: &mut usize) -> u16 {
    let value = u16::from_be_bytes(bytes[*offset..*offset + 2].try_into().unwrap());
    *offset += 2;
    value
}
fn inspect(bytes: &[u8]) -> (Vec<Record>, Vec<Record>) {
    assert!(bytes.len() <= 4096);
    assert_eq!(&bytes[..6], &[0, 0, 0x84, 0, 0, 0]);
    assert_eq!(&bytes[8..10], &[0, 0]);
    let answers = u16::from_be_bytes(bytes[6..8].try_into().unwrap()) as usize;
    let additional = u16::from_be_bytes(bytes[10..12].try_into().unwrap()) as usize;
    assert!(answers > 0 && answers + additional <= 4);
    let mut records = Vec::new();
    let mut offset = 12;
    for _ in 0..answers + additional {
        let owner = take_name(bytes, &mut offset);
        let kind = word(bytes, &mut offset);
        let class = word(bytes, &mut offset);
        let ttl = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let length = word(bytes, &mut offset) as usize;
        let data = bytes[offset..offset + length].to_vec();
        offset += length;
        records.push(Record {
            owner,
            kind,
            class,
            ttl,
            data,
        });
    }
    assert_eq!(offset, bytes.len(), "no suffix or incorrect section counts");
    let extra = records.split_off(answers);
    (records, extra)
}
fn expected(kinds: &[u16], ttl: u32) -> Vec<Record> {
    kinds
        .iter()
        .map(|kind| {
            let mut rr = local(*kind, ttl);
            rr.class = if *kind == 12 { 1 } else { 0x8001 };
            rr
        })
        .collect()
}
fn check(bytes: &[u8], now: u64, answers: &[u16], additional: &[u16], ttl: u32) {
    let output = packet(bytes, now);
    assert_eq!(
        inspect(&output),
        (expected(answers, ttl), expected(additional, ttl))
    );
}

#[test]
fn ptr_query_prepares_one_answer_and_three_frozen_additional_records() {
    check(&browse_query(), 1000, &[12], &[33, 16, 1], 100);
}

#[test]
fn all_direct_answers_suppressed_never_creates_an_additional_only_packet() {
    let input = query(&[(SERVICE, 12, 1)], &[local(12, 50)]);
    assert_eq!(
        prepare(&input, 1000),
        Ok(MdnsQueryResponse::AllDirectAnswersSuppressed)
    );
}

#[test]
fn unrelated_question_is_distinct_from_suppressed_direct_answers() {
    let input = query(&[("other.local.", 1, 1)], &[]);
    assert_eq!(
        prepare(&input, 1000),
        Ok(MdnsQueryResponse::NoMatchingQuestion)
    );
}
