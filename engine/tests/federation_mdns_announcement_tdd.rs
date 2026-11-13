//! Pure F1d publication fixtures. Never sends packets or proves identity, ownership or authority.
#![cfg(target_os = "linux")]
use cyanrex_engine::services::{
    federation_discovery::{
        lan::{encode_txt, CacheTime, LanCandidateCache, LanObservation, LanSource},
        DiscoveryPolicy, PeerAdvertisement, ProtocolRange,
    },
    federation_mdns::{parse_response, MdnsAnnouncement, MdnsEvent, MAX_MDNS_PACKET_BYTES},
};
use std::net::{Ipv4Addr, SocketAddrV4};

const SERVICE: &str = "_cyanrex._udp.local.";
const INSTANCE: &str = "node-one._cyanrex._udp.local.";
const HOST: &str = "host-one.local.";

#[path = "federation_mdns_announcement/inputs.rs"]
mod inputs;
#[path = "federation_mdns_announcement/lease.rs"]
mod lease;
#[path = "federation_mdns_announcement/wire.rs"]
mod wire;

#[derive(Debug, PartialEq, Eq)]
struct Record {
    owner: String,
    kind: u16,
    class: u16,
    ttl: u32,
    data: Vec<u8>,
}

fn word(bytes: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

/// Independent encoder inspection: no parser implementation or DNS compression interpretation.
fn name(bytes: &[u8], offset: &mut usize) -> String {
    let mut result = String::new();
    loop {
        let length = bytes[*offset] as usize;
        *offset += 1;
        assert!(length <= 63, "announcements must not contain name pointers");
        if length == 0 {
            return result;
        }
        let label = &bytes[*offset..*offset + length];
        assert!(label.is_ascii());
        result.push_str(std::str::from_utf8(label).unwrap());
        result.push('.');
        *offset += length;
    }
}

fn records(bytes: &[u8]) -> Vec<Record> {
    assert_eq!(&bytes[..12], &[0, 0, 0x84, 0, 0, 0, 0, 4, 0, 0, 0, 0]);
    assert!(bytes.len() <= MAX_MDNS_PACKET_BYTES);
    let mut offset = 12;
    let mut records = Vec::new();
    for _ in 0..4 {
        let owner = name(bytes, &mut offset);
        let kind = word(bytes, offset);
        let class = word(bytes, offset + 2);
        let ttl = u32::from_be_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
        let length = word(bytes, offset + 8) as usize;
        offset += 10;
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
    assert_eq!(
        offset,
        bytes.len(),
        "no trailing or partial resource records"
    );
    records
}

fn dns_name(value: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    for label in value.trim_end_matches('.').split('.') {
        assert!((1..=63).contains(&label.len()));
        bytes.push(label.len() as u8);
        bytes.extend_from_slice(label.as_bytes());
    }
    bytes.push(0);
    bytes
}

fn endpoint() -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 7), 4433)
}

fn raw_advertisement() -> serde_json::Value {
    serde_json::json!({
        "schema_version":1,"node_fingerprint":"b".repeat(64),
        "authority_id":"8b075b0b-8118-4c37-9224-337ba22f40c6",
        "protocol":{"min":1,"max":3},"offered_capabilities":["peer.identity"],
        "required_capabilities":[],"issued_at":1000,"expires_at":1100,
        "hints":[{"kind":"direct","address":"192.168.1.7:4433"}]
    })
}

fn advertisement(raw: &serde_json::Value) -> PeerAdvertisement {
    PeerAdvertisement::parse(&serde_json::to_vec(raw).unwrap()).unwrap()
}

fn announcement() -> MdnsAnnouncement {
    MdnsAnnouncement::new(
        "node-one",
        "host-one",
        endpoint(),
        &advertisement(&raw_advertisement()),
        1000,
        120,
    )
    .unwrap()
}

#[test]
fn announcement_roundtrip_preserves_exact_f0_and_single_endpoint_without_network() {
    let bytes = announcement().encode_at(1000).unwrap();
    assert!(bytes.len() <= MAX_MDNS_PACKET_BYTES);
    let events = parse_response(&bytes).unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        MdnsEvent::Resolved {
            instance,
            advertisement: f0,
            addresses,
            port,
            ttl,
        } => {
            assert_eq!(instance, "node-one");
            assert_eq!(f0, &advertisement(&raw_advertisement()).encode().unwrap());
            assert_eq!(addresses, &[Ipv4Addr::new(192, 168, 1, 7)]);
            assert_eq!(*port, 4433);
            assert_eq!(*ttl, 100);
        }
        _ => panic!("the complete packet must yield one unverified observation"),
    }
}
