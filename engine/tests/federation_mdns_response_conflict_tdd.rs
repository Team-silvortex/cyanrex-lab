//! Pure F1m record comparison; no live ownership, source proof or sender is constructed.
#![cfg(target_os = "linux")]
use cyanrex_engine::services::{
    federation_discovery::{lan::encode_txt, PeerAdvertisement},
    federation_mdns::{
        MdnsAnnouncement, MdnsError, ProbeObservation, ResponseConflictObservation as Observation,
        MAX_MDNS_PACKET_BYTES, MAX_MDNS_RECORDS,
    },
};

#[path = "federation_mdns_response_conflict/compression.rs"]
mod compression;
#[path = "federation_mdns_response_conflict/malformed.rs"]
mod malformed;

const INSTANCE: &str = "node-one._cyanrex._udp.local.";
const HOST: &str = "host-one.local.";
const SERVICE: &str = "_cyanrex._udp.local.";

#[derive(Clone)]
struct Record {
    owner: Vec<u8>,
    kind: u16,
    class: u16,
    ttl: u32,
    data: Vec<u8>,
}

fn name(value: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    for label in value
        .trim_end_matches('.')
        .split('.')
        .filter(|s| !s.is_empty())
    {
        bytes.push(label.len() as u8);
        bytes.extend(label.as_bytes());
    }
    bytes.push(0);
    bytes
}

fn record(owner: &str, kind: u16, data: Vec<u8>) -> Record {
    Record {
        owner: name(owner),
        kind,
        class: 1,
        ttl: 100,
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

fn packet(flags: u16, answers: &[Record], authority: &[Record], additional: &[Record]) -> Vec<u8> {
    let mut bytes = vec![0, 0];
    bytes.extend(flags.to_be_bytes());
    for count in [0, answers.len(), authority.len(), additional.len()] {
        bytes.extend((count as u16).to_be_bytes());
    }
    for rr in answers.iter().chain(authority).chain(additional) {
        append(&mut bytes, rr);
    }
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

fn announcement() -> MdnsAnnouncement {
    MdnsAnnouncement::new(
        "node-one",
        "host-one",
        "192.168.1.7:4433".parse().unwrap(),
        &advertisement(),
        1000,
        120,
    )
    .unwrap()
}

fn srv(priority: u16, weight: u16, port: u16, target: &str) -> Vec<u8> {
    let mut data = Vec::new();
    for word in [priority, weight, port] {
        data.extend(word.to_be_bytes());
    }
    data.extend(name(target));
    data
}

fn local(kind: u16) -> Record {
    match kind {
        1 => record(HOST, 1, vec![192, 168, 1, 7]),
        16 => record(INSTANCE, 16, encode_txt(&advertisement()).unwrap()),
        33 => record(INSTANCE, 33, srv(0, 0, 4433, HOST)),
        _ => panic!("fixture only owns three unique tuples"),
    }
}

fn observe(bytes: &[u8]) -> Result<Observation, MdnsError> {
    announcement().observe_response_conflict(bytes)
}

fn assert_invalid(bytes: &[u8]) {
    assert_eq!(observe(bytes), Err(MdnsError::InvalidPacket));
}

#[test]
fn an_empty_profile_response_has_no_compared_conflicting_record() {
    assert_eq!(
        observe(&packet(0x8400, &[], &[], &[])),
        Ok(Observation::NoConflictingRecord)
    );
}

#[test]
fn identical_complete_frozen_records_are_not_conflicting() {
    assert_eq!(
        observe(&announcement().encode_at(1000).unwrap()),
        Ok(Observation::NoConflictingRecord)
    );
}

#[test]
fn changed_a_rdata_is_conflicting() {
    let rr = record(HOST, 1, vec![192, 168, 1, 8]);
    assert_eq!(
        observe(&packet(0x8400, &[rr], &[], &[])),
        Ok(Observation::ConflictingRecord)
    );
}

#[test]
fn identical_unique_records_can_appear_individually_without_a_ptr() {
    for kind in [1, 16, 33] {
        assert_eq!(
            observe(&packet(0x8400, &[local(kind)], &[], &[])),
            Ok(Observation::NoConflictingRecord)
        );
    }
}

#[test]
fn both_profile_response_headers_accept_identical_data() {
    for flags in [0x8000, 0x8400] {
        assert_eq!(
            observe(&packet(flags, &[local(1), local(16), local(33)], &[], &[])),
            Ok(Observation::NoConflictingRecord)
        );
    }
}

#[test]
fn identical_goodbye_data_is_not_a_record_conflict_or_a_withdrawal_receipt() {
    assert_eq!(
        observe(&announcement().encode_goodbye().unwrap()),
        Ok(Observation::NoConflictingRecord)
    );
}

#[test]
fn old_probe_observation_stays_conservative_for_identical_live_and_goodbye_records() {
    let ad = announcement();
    for bytes in [ad.encode_at(1000).unwrap(), ad.encode_goodbye().unwrap()] {
        assert_eq!(
            ad.observe_probe_packet(&bytes),
            Ok(ProbeObservation::ExistingResponse)
        );
        assert_eq!(
            ad.observe_response_conflict(&bytes),
            Ok(Observation::NoConflictingRecord)
        );
    }
}

#[test]
fn nonzero_srv_priority_is_valid_but_conflicting() {
    for priority in [1, u16::MAX] {
        let rr = record(INSTANCE, 33, srv(priority, 0, 4433, HOST));
        assert_eq!(
            observe(&packet(0x8400, &[rr], &[], &[])),
            Ok(Observation::ConflictingRecord)
        );
    }
}

#[test]
fn nonzero_srv_weight_is_valid_but_conflicting() {
    for weight in [1, u16::MAX] {
        let rr = record(INSTANCE, 33, srv(0, weight, 4433, HOST));
        assert_eq!(
            observe(&packet(0x8400, &[rr], &[], &[])),
            Ok(Observation::ConflictingRecord)
        );
    }
}

#[test]
fn zero_or_changed_srv_port_is_not_hidden_by_candidate_admission() {
    for port in [0, 4434, u16::MAX] {
        let rr = record(INSTANCE, 33, srv(0, 0, port, HOST));
        assert_eq!(
            observe(&packet(0x8400, &[rr], &[], &[])),
            Ok(Observation::ConflictingRecord)
        );
    }
}

#[test]
fn root_external_and_non_ldh_srv_targets_are_valid_but_different() {
    for target in [
        ".",
        "elsewhere.example.",
        "_other.local.",
        "other host.local.",
    ] {
        let rr = record(INSTANCE, 33, srv(0, 0, 4433, target));
        assert_eq!(
            observe(&packet(0x8400, &[rr], &[], &[])),
            Ok(Observation::ConflictingRecord)
        );
    }
}

#[test]
fn any_changed_ipv4_bytes_are_conflicting_without_private_address_filtering() {
    for data in [
        vec![0; 4],
        vec![8, 8, 8, 8],
        vec![224, 0, 0, 251],
        vec![255; 4],
    ] {
        assert_eq!(
            observe(&packet(0x8400, &[record(HOST, 1, data)], &[], &[])),
            Ok(Observation::ConflictingRecord)
        );
    }
}

#[test]
fn well_formed_non_f0_and_binary_txt_are_compared_without_application_decoding() {
    for data in [vec![0], vec![3, b'x', b'=', b'1'], vec![3, 0, 0xff, 0xc0]] {
        assert_eq!(
            observe(&packet(0x8400, &[record(INSTANCE, 16, data)], &[], &[])),
            Ok(Observation::ConflictingRecord)
        );
    }
}

#[test]
fn txt_key_case_is_not_dns_name_case_folding() {
    let mut rr = local(16);
    assert_eq!(rr.data[1], b'v');
    rr.data[1] = b'V';
    assert_eq!(
        observe(&packet(0x8400, &[rr], &[], &[])),
        Ok(Observation::ConflictingRecord)
    );
}

#[test]
fn txt_field_order_is_not_normalized_even_if_f0_would_decode_equivalently() {
    let mut rr = local(16);
    let first_end = 1 + rr.data[0] as usize;
    rr.data.rotate_left(first_end);
    assert_eq!(
        observe(&packet(0x8400, &[rr], &[], &[])),
        Ok(Observation::ConflictingRecord)
    );
}

#[test]
fn full_255_byte_txt_string_is_valid_and_compared_as_raw_rdata() {
    let mut data = vec![255];
    data.extend([0xff; 255]);
    assert_eq!(
        observe(&packet(0x8400, &[record(INSTANCE, 16, data)], &[], &[])),
        Ok(Observation::ConflictingRecord)
    );
}

#[test]
fn different_shared_ptr_targets_are_not_conflicting_unique_records() {
    for owner in [SERVICE, HOST, INSTANCE] {
        assert_eq!(
            observe(&packet(
                0x8400,
                &[record(owner, 12, name("other._cyanrex._udp.local."))],
                &[],
                &[]
            )),
            Ok(Observation::NoConflictingRecord)
        );
    }
}

#[test]
fn matching_owner_but_different_rrtype_is_outside_the_three_frozen_unique_tuples() {
    for rr in [
        record(INSTANCE, 1, vec![192, 168, 1, 8]),
        record(HOST, 16, vec![0]),
        record(HOST, 33, srv(0, 0, 1, ".")),
    ] {
        assert_eq!(
            observe(&packet(0x8400, &[rr], &[], &[])),
            Ok(Observation::NoConflictingRecord)
        );
    }
}

#[test]
fn unknown_types_are_bounded_opaque_not_cross_type_name_conflicts() {
    for kind in [5, 28, 47, 65_000] {
        for owner in [HOST, INSTANCE] {
            assert_eq!(
                observe(&packet(
                    0x8400,
                    &[record(owner, kind, vec![0xc0, 0xff])],
                    &[],
                    &[]
                )),
                Ok(Observation::NoConflictingRecord)
            );
        }
    }
}

#[test]
fn non_in_records_are_not_interpreted_as_in_rdata_or_compared() {
    for class in [0, 3, 255, 0x8003] {
        for kind in [1, 12, 16, 33] {
            let mut rr = record(HOST, kind, vec![]);
            rr.class = class;
            assert_eq!(
                observe(&packet(0x8400, &[rr], &[], &[])),
                Ok(Observation::NoConflictingRecord)
            );
        }
    }
}

#[test]
fn only_exact_dns_owner_tuples_are_compared() {
    for owner in [
        "other.local.",
        "host-one.example.local.",
        "xhost-one.local.",
        "host-one.local.example.",
        SERVICE,
    ] {
        assert_eq!(
            observe(&packet(
                0x8400,
                &[record(owner, 1, vec![192, 168, 1, 8])],
                &[],
                &[]
            )),
            Ok(Observation::NoConflictingRecord)
        );
    }
}

#[test]
fn each_resource_record_section_can_supply_a_conflict() {
    for kind in [1, 16, 33] {
        let mut rr = local(kind);
        if kind == 33 {
            rr.data[5] ^= 1;
        } else {
            *rr.data.last_mut().unwrap() ^= 1;
        }
        for section in 0..3 {
            let mut sections: [Vec<Record>; 3] = std::array::from_fn(|_| Vec::new());
            sections[section].push(rr.clone());
            assert_eq!(
                observe(&packet(0x8400, &sections[0], &sections[1], &sections[2])),
                Ok(Observation::ConflictingRecord)
            );
        }
    }
}

#[test]
fn ttl_and_cache_flush_do_not_change_record_equality() {
    for kind in [1, 16, 33] {
        for class in [1, 0x8001] {
            for ttl in [0, 1, u32::MAX] {
                let mut rr = local(kind);
                rr.class = class;
                rr.ttl = ttl;
                assert_eq!(
                    observe(&packet(0x8400, &[rr.clone()], &[], &[])),
                    Ok(Observation::NoConflictingRecord)
                );
                if kind == 33 {
                    rr.data[5] ^= 1;
                } else {
                    *rr.data.last_mut().unwrap() ^= 1;
                }
                assert_eq!(
                    observe(&packet(0x8400, &[rr], &[], &[])),
                    Ok(Observation::ConflictingRecord)
                );
            }
        }
    }
}

#[test]
fn duplicate_identical_records_are_not_a_conflict() {
    for kind in [1, 16, 33] {
        let rr = local(kind);
        assert_eq!(
            observe(&packet(
                0x8400,
                &[rr.clone(), rr.clone()],
                &[rr.clone()],
                &[rr]
            )),
            Ok(Observation::NoConflictingRecord)
        );
    }
}

#[test]
fn one_different_duplicate_is_a_conflict_regardless_of_order() {
    let same = local(1);
    let other = record(HOST, 1, vec![192, 168, 1, 8]);
    for records in [vec![same.clone(), other.clone()], vec![other, same]] {
        assert_eq!(
            observe(&packet(0x8400, &records, &[], &[])),
            Ok(Observation::ConflictingRecord)
        );
    }
}

#[test]
fn observation_does_not_mutate_input_or_frozen_announcements() {
    let ad = announcement();
    let bytes = ad.encode_at(1000).unwrap();
    let original = bytes.clone();
    assert_eq!(
        ad.observe_response_conflict(&bytes),
        Ok(Observation::NoConflictingRecord)
    );
    assert_eq!(bytes, original);
    assert_eq!(ad.encode_at(1000).unwrap(), original);
}

#[test]
fn response_compatibility_ignores_query_id_without_relaxing_old_probe_rules() {
    for id in [1_u16, 0x7fff, u16::MAX] {
        let mut bytes = packet(0x8400, &[local(1)], &[], &[]);
        bytes[..2].copy_from_slice(&id.to_be_bytes());
        assert_eq!(observe(&bytes), Ok(Observation::NoConflictingRecord));
        assert_eq!(
            announcement().observe_probe_packet(&bytes),
            Err(MdnsError::InvalidPacket)
        );
    }
}

#[test]
fn response_compatibility_ignores_aa_tc_rd_ra_z_ad_cd_bits_without_omitting_full_framing() {
    for ignored_bits in 0_u16..128 {
        let flags = 0x8000 | (ignored_bits << 4);
        assert_eq!(
            observe(&packet(flags, &[local(1)], &[], &[])),
            Ok(Observation::NoConflictingRecord)
        );
    }
}

#[test]
fn response_compatibility_accepts_zero_length_txt_as_different_from_nonempty_frozen_txt() {
    assert_eq!(
        observe(&packet(0x8400, &[record(INSTANCE, 16, vec![])], &[], &[])),
        Ok(Observation::ConflictingRecord)
    );
}
