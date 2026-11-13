//! Pure single-packet query selection; caller metadata is not ingress or sending authority.
#![cfg(target_os = "linux")]
use cyanrex_engine::services::{
    federation_discovery::{lan::encode_txt, PeerAdvertisement},
    federation_mdns::{
        browse_query, MdnsAnnouncement, MdnsError, MdnsQueryContext, MdnsQueryPlan,
        MdnsQueryRecordSet as Set, MAX_MDNS_PACKET_BYTES, MAX_MDNS_RECORDS,
    },
};

const SERVICE: &str = "_cyanrex._udp.local.";
const INSTANCE: &str = "node-one._cyanrex._udp.local.";
const HOST: &str = "host-one.local.";

#[path = "federation_mdns_query_plan/compression.rs"]
mod compression;
#[path = "federation_mdns_query_plan/context.rs"]
mod context;
#[path = "federation_mdns_query_plan/known_answers.rs"]
mod known_answers;
#[path = "federation_mdns_query_plan/malformed.rs"]
mod malformed;

#[derive(Clone)]
struct Question {
    owner: Vec<u8>,
    kind: u16,
    class: u16,
}
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
        .filter(|v| !v.is_empty())
    {
        bytes.push(label.len() as u8);
        bytes.extend(label.as_bytes());
    }
    bytes.push(0);
    bytes
}
fn question(owner: &str, kind: u16, class: u16) -> Question {
    Question {
        owner: name(owner),
        kind,
        class,
    }
}
fn record(owner: &str, kind: u16, data: Vec<u8>, ttl: u32) -> Record {
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
        bytes.extend(&q.owner);
        bytes.extend(q.kind.to_be_bytes());
        bytes.extend(q.class.to_be_bytes());
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
    match kind {
        12 => record(SERVICE, 12, name(INSTANCE), ttl),
        33 => {
            let mut data = vec![0, 0, 0, 0];
            data.extend(4433_u16.to_be_bytes());
            data.extend(name(HOST));
            record(INSTANCE, 33, data, ttl)
        }
        16 => record(INSTANCE, 16, encode_txt(&advertisement()).unwrap(), ttl),
        1 => record(HOST, 1, vec![192, 168, 1, 7], ttl),
        _ => panic!("four frozen fixture tuples only"),
    }
}
fn plan(bytes: &[u8], now: u64) -> Result<MdnsQueryPlan, MdnsError> {
    announcement(100).plan_query(bytes, 5353, true, now)
}
fn set(kind: u16) -> Set {
    Set {
        ptr: kind == 12,
        srv: kind == 33,
        txt: kind == 16,
        a: kind == 1,
    }
}
fn selected(requested: Set, suppressed: Set) -> MdnsQueryPlan {
    MdnsQueryPlan::Multicast {
        requested,
        suppressed,
    }
}

#[test]
fn ordinary_ptr_browse_selects_only_the_direct_ptr_record() {
    assert_eq!(
        plan(&browse_query(), 1000),
        Ok(selected(set(12), Set::default()))
    );
}

#[test]
fn a_matching_answer_at_half_the_local_ttl_suppresses_only_that_record() {
    let bytes = packet(0, &[question(SERVICE, 12, 1)], &[local(12, 50)], &[], &[]);
    assert_eq!(plan(&bytes, 1000), Ok(selected(set(12), set(12))));
}

#[test]
fn a_qu_question_requires_context_instead_of_a_multicast_selection() {
    let bytes = packet(0, &[question(SERVICE, 12, 0x8001)], &[], &[], &[]);
    assert_eq!(
        plan(&bytes, 1000),
        Ok(MdnsQueryPlan::RequiresContext(MdnsQueryContext {
            unicast_question: true,
            ..Default::default()
        }))
    );
}

fn all() -> Set {
    Set {
        ptr: true,
        srv: true,
        txt: true,
        a: true,
    }
}
fn questions_all() -> Vec<Question> {
    vec![
        question(SERVICE, 12, 1),
        question(INSTANCE, 33, 1),
        question(INSTANCE, 16, 1),
        question(HOST, 1, 1),
    ]
}
fn owner(kind: u16) -> &'static str {
    match kind {
        12 => SERVICE,
        1 => HOST,
        _ => INSTANCE,
    }
}

#[test]
fn exact_types_with_in_or_any_class_select_each_frozen_record_independently() {
    for kind in [12, 33, 16, 1] {
        for class in [1, 255] {
            let bytes = packet(0, &[question(owner(kind), kind, class)], &[], &[], &[]);
            assert_eq!(plan(&bytes, 1000), Ok(selected(set(kind), Set::default())));
        }
    }
}

#[test]
fn any_type_selects_only_records_at_the_exact_owner() {
    for class in [1, 255] {
        for (owner, expected) in [
            (SERVICE, set(12)),
            (HOST, set(1)),
            (
                INSTANCE,
                Set {
                    srv: true,
                    txt: true,
                    ..Default::default()
                },
            ),
        ] {
            assert_eq!(
                plan(
                    &packet(0, &[question(owner, 255, class)], &[], &[], &[]),
                    1000
                ),
                Ok(selected(expected, Set::default()))
            );
        }
    }
}

#[test]
fn multiple_and_duplicate_questions_form_a_bounded_union_without_additionals() {
    let mut questions = questions_all();
    questions.extend(questions_all());
    assert_eq!(questions.len(), 8);
    assert_eq!(
        plan(&packet(0, &questions, &[], &[], &[]), 1000),
        Ok(selected(all(), Set::default()))
    );
}

#[test]
fn an_srv_question_does_not_implicitly_select_the_target_a_or_txt() {
    assert_eq!(
        plan(
            &packet(0, &[question(INSTANCE, 33, 1)], &[], &[], &[]),
            1000
        ),
        Ok(selected(set(33), Set::default()))
    );
}

#[test]
fn wrong_class_type_and_owner_do_not_create_frozen_positive_answers() {
    for q in [
        question(HOST, 1, 3),
        question(HOST, 1, 0),
        question(HOST, 28, 1),
        question(SERVICE, 1, 1),
        question("elsewhere.local.", 255, 255),
        question("host-one.example.local.", 1, 1),
    ] {
        assert_eq!(
            plan(&packet(0, &[q], &[], &[], &[]), 1000),
            Ok(selected(Set::default(), Set::default()))
        );
    }
}

#[test]
fn query_id_is_not_an_ingress_classification_or_a_record_selector() {
    for id in [1_u16, 0x7fff, u16::MAX] {
        let mut bytes = browse_query();
        bytes[..2].copy_from_slice(&id.to_be_bytes());
        assert_eq!(plan(&bytes, 1000), Ok(selected(set(12), Set::default())));
    }
}

#[test]
fn ignored_query_header_bits_do_not_change_qm_record_selection() {
    for bits in 0_u16..128 {
        let flags = (bits << 4) & !0x0200;
        assert_eq!(
            plan(
                &packet(flags, &[question(SERVICE, 12, 1)], &[], &[], &[]),
                1000
            ),
            Ok(selected(set(12), Set::default()))
        );
    }
}

#[test]
fn before_preparation_time_cannot_select_even_a_matching_record() {
    assert_eq!(plan(&browse_query(), 999), Err(MdnsError::ClockInvalid));
}

#[test]
fn exact_f0_expiry_rejects_instead_of_returning_an_empty_plan() {
    assert!(matches!(
        plan(&browse_query(), 1100),
        Err(MdnsError::RejectedAdvertisement(_))
    ));
}

#[test]
fn expired_declaration_is_not_hidden_by_a_requires_context_result() {
    let bytes = packet(0x0200, &[question(SERVICE, 12, 0x8001)], &[], &[], &[]);
    assert!(matches!(
        plan(&bytes, 1100),
        Err(MdnsError::RejectedAdvertisement(_))
    ));
}

#[test]
fn query_selection_does_not_change_input_or_the_frozen_announcement() {
    let ad = announcement(100);
    let before = ad.encode_at(1000).unwrap();
    let bytes = browse_query();
    let original = bytes.clone();
    ad.plan_query(&bytes, 5353, true, 1000).unwrap();
    assert_eq!(bytes, original);
    assert_eq!(ad.encode_at(1000).unwrap(), before);
}
