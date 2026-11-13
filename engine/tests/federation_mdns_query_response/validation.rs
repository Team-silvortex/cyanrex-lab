use super::*;
use cyanrex_engine::services::federation_mdns::ResponseConflictObservation;

fn invalid_for_both(bytes: &[u8]) {
    assert_eq!(prepare(bytes, 1000), Err(MdnsError::InvalidPacket));
    assert_eq!(
        announcement(100).plan_query(bytes, 5353, true, 1000),
        Err(MdnsError::InvalidPacket)
    );
}

fn require_context_for_both(bytes: &[u8], port: u16, multicast: bool, context: MdnsQueryContext) {
    let ad = announcement(100);
    assert_eq!(
        ad.prepare_query_response(bytes, port, multicast, 1000),
        Ok(MdnsQueryResponse::RequiresContext(context))
    );
    assert_eq!(
        ad.plan_query(bytes, port, multicast, 1000),
        Ok(MdnsQueryPlan::RequiresContext(context))
    );
}

fn add_section_record(bytes: &mut Vec<u8>, count_offset: usize, rr: &Record) {
    let count = u16::from_be_bytes(bytes[count_offset..count_offset + 2].try_into().unwrap());
    bytes[count_offset..count_offset + 2].copy_from_slice(&(count + 1).to_be_bytes());
    append(bytes, rr);
}

fn malformed_known(kind: u16) -> Record {
    let mut rr = local(kind, 1);
    rr.owner = name("unrelated.local.");
    rr.data = vec![0xff];
    rr
}

#[test]
fn truncated_known_answer_queries_require_context_instead_of_prepared_bytes() {
    let mut bytes = query(&[(SERVICE, 12, 1)], &[local(12, 50)]);
    bytes[2..4].copy_from_slice(&0x0200_u16.to_be_bytes());
    require_context_for_both(
        &bytes,
        5353,
        true,
        MdnsQueryContext {
            truncated: true,
            ..Default::default()
        },
    );
}

#[test]
fn an_unrelated_qu_question_defers_the_whole_mixed_packet() {
    let bytes = query(&[(SERVICE, 12, 1), ("other.local.", 65000, 0x8001)], &[]);
    require_context_for_both(
        &bytes,
        5353,
        true,
        MdnsQueryContext {
            unicast_question: true,
            ..Default::default()
        },
    );
}

#[test]
fn even_an_opaque_unrelated_authority_record_requires_context() {
    let mut bytes = query(&[(SERVICE, 12, 1)], &[]);
    let mut rr = local(1, 1);
    rr.owner = name("other.local.");
    rr.kind = 65000;
    rr.data = vec![0xff];
    add_section_record(&mut bytes, 8, &rr);
    require_context_for_both(
        &bytes,
        5353,
        true,
        MdnsQueryContext {
            authority_records: true,
            ..Default::default()
        },
    );
}

#[test]
fn zero_questions_require_context_with_or_without_known_answers() {
    for known in [vec![], vec![local(12, 50)]] {
        require_context_for_both(
            &query(&[], &known),
            5353,
            true,
            MdnsQueryContext {
                no_questions: true,
                ..Default::default()
            },
        );
    }
}

#[test]
fn context_reasons_accumulate_including_the_empty_question_branch() {
    for (questions, unicast_question, no_questions) in [
        (vec![(SERVICE, 12, 0x8001)], true, false),
        (vec![], false, true),
    ] {
        let mut bytes = query(&questions, &[]);
        bytes[2..4].copy_from_slice(&0x0200_u16.to_be_bytes());
        add_section_record(&mut bytes, 8, &local(1, 1));
        require_context_for_both(
            &bytes,
            53,
            false,
            MdnsQueryContext {
                truncated: true,
                unicast_question,
                authority_records: true,
                legacy_port: true,
                direct_unicast: true,
                no_questions,
            },
        );
    }
}

#[test]
fn legacy_source_ports_are_explicit_context_not_an_id_inference() {
    for port in [1, 53, 49152, u16::MAX] {
        require_context_for_both(
            &browse_query(),
            port,
            true,
            MdnsQueryContext {
                legacy_port: true,
                ..Default::default()
            },
        );
    }
}

#[test]
fn direct_unicast_needs_context_even_when_the_source_port_is_5353() {
    require_context_for_both(
        &browse_query(),
        5353,
        false,
        MdnsQueryContext {
            direct_unicast: true,
            ..Default::default()
        },
    );
}

#[test]
fn zero_source_port_is_invalid_configuration_before_packet_or_time_processing() {
    let ad = announcement(100);
    for (bytes, now) in [(browse_query(), 1000), (vec![], 999)] {
        assert_eq!(
            ad.prepare_query_response(&bytes, 0, true, now),
            Err(MdnsError::InvalidConfig)
        );
        assert_eq!(
            ad.plan_query(&bytes, 0, true, now),
            Err(MdnsError::InvalidConfig)
        );
    }
}

#[test]
fn no_direct_match_does_not_erase_context_or_create_a_no_match_response() {
    let mut bytes = query(&[("unrelated.local.", 65000, 0x8001)], &[]);
    bytes[2..4].copy_from_slice(&0x0200_u16.to_be_bytes());
    require_context_for_both(
        &bytes,
        5353,
        true,
        MdnsQueryContext {
            truncated: true,
            unicast_question: true,
            ..Default::default()
        },
    );
}

#[test]
fn response_headers_nonstandard_opcodes_and_nonzero_rcodes_are_errors() {
    for flags in [0x8000_u16, 0x8400, 0x0800, 0x1000, 0x7800, 1, 15] {
        let mut bytes = browse_query();
        bytes[2..4].copy_from_slice(&flags.to_be_bytes());
        invalid_for_both(&bytes);
    }
}

#[test]
fn no_matching_question_does_not_hide_later_malformed_known_in_records() {
    for kind in [1, 12, 16, 33] {
        let mut bytes = query(&[("other.local.", 65000, 1)], &[]);
        add_section_record(&mut bytes, 10, &malformed_known(kind));
        invalid_for_both(&bytes);
    }
}

#[test]
fn all_direct_answers_suppressed_does_not_hide_malformed_later_data() {
    for kind in [1, 12, 16, 33] {
        let mut bytes = query(&[(SERVICE, 12, 1)], &[local(12, 50)]);
        add_section_record(&mut bytes, 10, &malformed_known(kind));
        invalid_for_both(&bytes);
    }
}

#[test]
fn context_flags_do_not_hide_malformed_known_in_data_in_any_section() {
    for kind in [1, 12, 16, 33] {
        for count_offset in [6, 8, 10] {
            let mut bytes = query(&[(SERVICE, 12, 0x8001)], &[]);
            bytes[2..4].copy_from_slice(&0x0200_u16.to_be_bytes());
            add_section_record(&mut bytes, count_offset, &malformed_known(kind));
            invalid_for_both(&bytes);
        }
    }
}

#[test]
fn answer_cache_flush_is_invalid_for_every_known_in_type_even_unrelated_ones() {
    for kind in [1, 12, 16, 33] {
        let mut rr = local(kind, 50);
        rr.class = 0x8001;
        invalid_for_both(&query(&[(SERVICE, 12, 1)], &[rr.clone()]));
        rr.owner = name("unrelated.local.");
        invalid_for_both(&query(&[("other.local.", 65000, 1)], &[rr]));
    }
}

#[test]
fn strict_query_flush_validation_does_not_change_f1m_response_equality() {
    let ad = announcement(100);
    for kind in [1, 16, 33] {
        let mut rr = local(kind, 0);
        rr.class = 0x8001;
        invalid_for_both(&query(&[(SERVICE, 12, 1)], &[rr.clone()]));
        let mut response = query(&[], &[rr.clone()]);
        response[2..4].copy_from_slice(&0x8400_u16.to_be_bytes());
        assert_eq!(
            ad.observe_response_conflict(&response),
            Ok(ResponseConflictObservation::NoConflictingRecord)
        );
        rr.data[if kind == 33 { 5 } else { 1 }] ^= 1;
        for class in [1, 0x8001] {
            rr.class = class;
            let mut response = query(&[], &[rr.clone()]);
            response[2..4].copy_from_slice(&0x8400_u16.to_be_bytes());
            assert_eq!(
                ad.observe_response_conflict(&response),
                Ok(ResponseConflictObservation::ConflictingRecord)
            );
        }
    }
}

#[test]
fn additional_flush_is_not_a_known_answer_or_a_qu_question() {
    let mut bytes = browse_query();
    let mut rr = local(33, u32::MAX);
    rr.class = 0x8001;
    add_section_record(&mut bytes, 10, &rr);
    assert_eq!(
        announcement(100).plan_query(&bytes, 5353, true, 1000),
        Ok(MdnsQueryPlan::Multicast {
            requested: MdnsQueryRecordSet {
                ptr: true,
                ..Default::default()
            },
            suppressed: MdnsQueryRecordSet::default(),
        })
    );
    check(&bytes, 1000, &[12], &[33, 16, 1], 100);
}

#[test]
fn every_truncated_prefix_is_invalid_even_for_a_tc_marked_query() {
    let mut bytes = query(&[(SERVICE, 12, 1)], &[local(12, 50), local(33, 50)]);
    bytes[2..4].copy_from_slice(&0x0200_u16.to_be_bytes());
    for end in 0..bytes.len() {
        invalid_for_both(&bytes[..end]);
    }
}

#[test]
fn trailing_bytes_are_rejected_after_an_otherwise_all_suppressed_query() {
    let mut bytes = query(&[(SERVICE, 12, 1)], &[local(12, 50)]);
    bytes.push(0);
    invalid_for_both(&bytes);
}

#[test]
fn bounded_framing_failures_never_return_partial_context_or_packets() {
    let mut bytes = browse_query();
    bytes[4..6].copy_from_slice(&9_u16.to_be_bytes());
    invalid_for_both(&bytes);
    for count_offset in [6, 8, 10] {
        let mut bytes = browse_query();
        bytes[count_offset..count_offset + 2].copy_from_slice(&65_u16.to_be_bytes());
        invalid_for_both(&bytes);
    }
    let bytes = vec![0; 4097];
    assert_eq!(prepare(&bytes, 1000), Err(MdnsError::TooLarge));
    assert_eq!(
        announcement(100).plan_query(&bytes, 5353, true, 1000),
        Err(MdnsError::TooLarge)
    );
}

#[test]
fn invalid_freshness_cannot_be_hidden_by_any_nonpacket_result() {
    let ad = announcement(100);
    for bytes in [
        query(&[("other.local.", 65000, 1)], &[]),
        query(&[(SERVICE, 12, 1)], &[local(12, 50)]),
        query(&[(SERVICE, 12, 0x8001)], &[]),
    ] {
        assert_eq!(
            ad.prepare_query_response(&bytes, 5353, true, 999),
            Err(MdnsError::ClockInvalid)
        );
        assert_eq!(
            ad.plan_query(&bytes, 5353, true, 999),
            Err(MdnsError::ClockInvalid)
        );
        let error = ad.plan_query(&bytes, 5353, true, 1100).unwrap_err();
        assert!(matches!(error, MdnsError::RejectedAdvertisement(_)));
        assert_eq!(
            ad.prepare_query_response(&bytes, 5353, true, 1100),
            Err(error)
        );
    }
}
