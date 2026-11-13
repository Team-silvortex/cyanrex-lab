//! Pure F1e preparation and observation fixtures; never send or prove a name is available.
#![cfg(target_os = "linux")]
use cyanrex_engine::services::{
    federation_discovery::{lan::encode_txt, PeerAdvertisement},
    federation_mdns::{
        MdnsAnnouncement, MdnsError, ProbeObservation, MAX_MDNS_PACKET_BYTES, MAX_MDNS_RECORDS,
    },
};
use std::net::{Ipv4Addr, SocketAddrV4};

const SERVICE: &str = "_cyanrex._udp.local.";
const INSTANCE: &str = "node-one._cyanrex._udp.local.";
const HOST: &str = "host-one.local.";

#[path = "federation_mdns_probe/malformed.rs"]
mod malformed;
#[path = "federation_mdns_probe/observation.rs"]
mod observation;
#[path = "federation_mdns_probe/wire.rs"]
mod wire;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Record {
    owner: String,
    kind: u16,
    class: u16,
    ttl: u32,
    data: Vec<u8>,
}

#[derive(Clone)]
struct Question {
    owner: String,
    kind: u16,
    class: u16,
}

fn dns_name(value: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    for label in value.trim_end_matches('.').split('.') {
        assert!((1..=63).contains(&label.len()));
        bytes.push(label.len() as u8);
        bytes.extend(label.as_bytes());
    }
    bytes.push(0);
    bytes
}

fn question(owner: &str, kind: u16, class: u16) -> Question {
    Question {
        owner: owner.into(),
        kind,
        class,
    }
}

fn record(owner: &str, kind: u16) -> Record {
    Record {
        owner: owner.into(),
        kind,
        class: 1,
        ttl: 100,
        data: vec![192, 168, 1, 7],
    }
}

fn append_rr(bytes: &mut Vec<u8>, owner: &[u8], rr: &Record) {
    bytes.extend(owner);
    bytes.extend(rr.kind.to_be_bytes());
    bytes.extend(rr.class.to_be_bytes());
    bytes.extend(rr.ttl.to_be_bytes());
    bytes.extend((rr.data.len() as u16).to_be_bytes());
    bytes.extend(&rr.data);
}

fn packet(
    flags: u16,
    questions: &[Question],
    answers: &[Record],
    authority: &[Record],
    additional: &[Record],
) -> Vec<u8> {
    let mut bytes = vec![0, 0];
    bytes.extend(flags.to_be_bytes());
    for count in [
        questions.len(),
        answers.len(),
        authority.len(),
        additional.len(),
    ] {
        bytes.extend((count as u16).to_be_bytes());
    }
    for q in questions {
        bytes.extend(dns_name(&q.owner));
        bytes.extend(q.kind.to_be_bytes());
        bytes.extend(q.class.to_be_bytes());
    }
    for rr in answers.iter().chain(authority).chain(additional) {
        append_rr(&mut bytes, &dns_name(&rr.owner), rr);
    }
    bytes
}

fn observe(bytes: &[u8]) -> Result<ProbeObservation, MdnsError> {
    announcement().observe_probe_packet(bytes)
}

fn assert_invalid(bytes: &[u8]) {
    assert_eq!(observe(bytes), Err(MdnsError::InvalidPacket));
}

fn word(bytes: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

/// Independent, uncompressed inspection of our encoder, not a product parser roundtrip.
fn read_name(bytes: &[u8], offset: &mut usize) -> String {
    let mut result = String::new();
    loop {
        let length = bytes[*offset] as usize;
        *offset += 1;
        assert!(length <= 63, "this encoder emits no compression pointers");
        if length == 0 {
            return result;
        }
        result.push_str(std::str::from_utf8(&bytes[*offset..*offset + length]).unwrap());
        result.push('.');
        *offset += length;
    }
}

fn probe_records(bytes: &[u8]) -> Vec<Record> {
    assert_eq!(&bytes[..12], &[0, 0, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0]);
    assert!(bytes.len() <= MAX_MDNS_PACKET_BYTES);
    let mut offset = 12;
    for expected in [INSTANCE, HOST] {
        assert_eq!(read_name(bytes, &mut offset), expected);
        assert_eq!(word(bytes, offset), 255);
        assert_eq!(word(bytes, offset + 2), 0x8001);
        offset += 4;
    }
    let mut result = Vec::new();
    for _ in 0..3 {
        let owner = read_name(bytes, &mut offset);
        let kind = word(bytes, offset);
        let class = word(bytes, offset + 2);
        let ttl = u32::from_be_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
        let length = word(bytes, offset + 8) as usize;
        offset += 10;
        let data = bytes[offset..offset + length].to_vec();
        offset += length;
        result.push(Record {
            owner,
            kind,
            class,
            ttl,
            data,
        });
    }
    assert_eq!(offset, bytes.len(), "no prefix, padding or omitted records");
    result
}

fn advertisement() -> PeerAdvertisement {
    let value = serde_json::json!({
        "schema_version":1,"node_fingerprint":"b".repeat(64),
        "authority_id":"8b075b0b-8118-4c37-9224-337ba22f40c6",
        "protocol":{"min":1,"max":3},"offered_capabilities":["peer.identity"],
        "required_capabilities":[],"issued_at":1000,"expires_at":1100,
        "hints":[{"kind":"direct","address":"192.168.1.7:4433"}]
    });
    PeerAdvertisement::parse(&serde_json::to_vec(&value).unwrap()).unwrap()
}

fn announcement() -> MdnsAnnouncement {
    MdnsAnnouncement::new(
        "node-one",
        "host-one",
        SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 7), 4433),
        &advertisement(),
        1000,
        120,
    )
    .unwrap()
}

#[test]
fn own_probe_is_competition_not_an_available_name_or_a_tiebreak_win() {
    let local = announcement();
    assert_eq!(
        local.observe_probe_packet(&local.encode_probe_at(1000).unwrap()),
        Ok(ProbeObservation::CompetingProbe)
    );
}
