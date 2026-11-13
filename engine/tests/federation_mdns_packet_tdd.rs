//! Pure bounded DNS packet fixtures; no multicast sockets, clocks, DB or peer authority.
#![cfg(target_os = "linux")]
use cyanrex_engine::services::{
    federation_discovery::{lan::encode_txt, PeerAdvertisement},
    federation_mdns::{
        browse_query, parse_response, MdnsError, MdnsEvent, MAX_MDNS_PACKET_BYTES, MAX_MDNS_RECORDS,
    },
};
use std::net::Ipv4Addr;

#[path = "federation_mdns_packet/complete.rs"]
mod complete;
#[path = "federation_mdns_packet/malformed.rs"]
mod malformed;

const SERVICE: &str = "_cyanrex._udp.local.";
const INSTANCE: &str = "node-one._cyanrex._udp.local.";
const HOST: &str = "node-one.local.";

#[derive(Clone)]
struct Record {
    owner: String,
    kind: u16,
    class: u16,
    ttl: u32,
    data: Vec<u8>,
}

fn dns_name(value: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    for label in value.trim_end_matches('.').split('.') {
        bytes.push(label.len() as u8);
        bytes.extend(label.as_bytes());
    }
    bytes.push(0);
    bytes
}

fn txt() -> Vec<u8> {
    let value = serde_json::json!({
        "schema_version":1,"node_fingerprint":"b".repeat(64),
        "authority_id":"8b075b0b-8118-4c37-9224-337ba22f40c6",
        "protocol":{"min":1,"max":3},"offered_capabilities":["peer.identity"],
        "required_capabilities":[],"issued_at":1000,"expires_at":1100,
        "hints":[{"kind":"direct","address":"192.168.1.7:4433"}]
    });
    encode_txt(&PeerAdvertisement::parse(&serde_json::to_vec(&value).unwrap()).unwrap()).unwrap()
}

fn srv(target: &str, port: u16) -> Vec<u8> {
    let mut bytes = vec![0, 0, 0, 0];
    bytes.extend(port.to_be_bytes());
    bytes.extend(dns_name(target));
    bytes
}

fn records() -> Vec<Record> {
    vec![
        Record {
            owner: SERVICE.into(),
            kind: 12,
            class: 1,
            ttl: 100,
            data: dns_name(INSTANCE),
        },
        Record {
            owner: INSTANCE.into(),
            kind: 33,
            class: 0x8001,
            ttl: 90,
            data: srv(HOST, 4433),
        },
        Record {
            owner: INSTANCE.into(),
            kind: 16,
            class: 0x8001,
            ttl: 80,
            data: txt(),
        },
        Record {
            owner: HOST.into(),
            kind: 1,
            class: 0x8001,
            ttl: 70,
            data: vec![192, 168, 1, 7],
        },
    ]
}

fn append_rr(bytes: &mut Vec<u8>, owner: &[u8], kind: u16, class: u16, ttl: u32, data: &[u8]) {
    bytes.extend(owner);
    bytes.extend(kind.to_be_bytes());
    bytes.extend(class.to_be_bytes());
    bytes.extend(ttl.to_be_bytes());
    bytes.extend((data.len() as u16).to_be_bytes());
    bytes.extend(data);
}

fn packet(records: &[Record]) -> Vec<u8> {
    let mut bytes = vec![0, 0, 0x84, 0, 0, 0];
    bytes.extend((records.len() as u16).to_be_bytes());
    bytes.extend([0; 4]);
    for rr in records {
        append_rr(
            &mut bytes,
            &dns_name(&rr.owner),
            rr.kind,
            rr.class,
            rr.ttl,
            &rr.data,
        );
    }
    bytes
}

fn pointer(offset: usize) -> Vec<u8> {
    assert!(offset < 0x4000);
    (0xc000 | offset as u16).to_be_bytes().to_vec()
}

fn compressed() -> Vec<u8> {
    compressed_offsets().0
}

fn compressed_offsets() -> (Vec<u8>, [usize; 2]) {
    let mut bytes = vec![0, 0, 0x84, 0, 0, 1, 0, 4, 0, 0, 0, 0];
    let service_offset = bytes.len();
    bytes.extend(dns_name(SERVICE));
    bytes.extend([0, 12, 0, 1]);
    let instance_offset = bytes.len() + 2 + 10;
    let ptr_length_offset = bytes.len() + 2 + 8;
    let mut instance = vec![8];
    instance.extend(b"node-one");
    instance.extend(pointer(service_offset));
    append_rr(&mut bytes, &pointer(service_offset), 12, 1, 100, &instance);
    let host_offset = bytes.len() + 2 + 10 + 6;
    let srv_length_offset = bytes.len() + 2 + 8;
    let mut srv_data = vec![0, 0, 0, 0, 0x11, 0x51, 8];
    srv_data.extend(b"node-one");
    srv_data.extend(pointer(service_offset + 14));
    append_rr(
        &mut bytes,
        &pointer(instance_offset),
        33,
        0x8001,
        90,
        &srv_data,
    );
    append_rr(
        &mut bytes,
        &pointer(instance_offset),
        16,
        0x8001,
        80,
        &txt(),
    );
    append_rr(
        &mut bytes,
        &pointer(host_offset),
        1,
        0x8001,
        70,
        &[192, 168, 1, 7],
    );
    (bytes, [ptr_length_offset, srv_length_offset])
}

fn assert_invalid(bytes: &[u8]) {
    assert!(matches!(
        parse_response(bytes),
        Err(MdnsError::InvalidPacket)
    ));
}

#[test]
fn browse_query_is_one_id_zero_ptr_in_question_for_the_fixed_local_service() {
    assert_eq!(MAX_MDNS_PACKET_BYTES, 4_096);
    assert_eq!(MAX_MDNS_RECORDS, 64);
    let mut expected = vec![0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    expected.extend(b"\x08_cyanrex\x04_udp\x05local\0\0\x0c\0\x01");
    assert_eq!(browse_query(), expected);
}
