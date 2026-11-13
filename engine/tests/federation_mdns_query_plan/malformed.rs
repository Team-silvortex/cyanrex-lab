use super::*;

fn invalid(bytes: &[u8]) {
    assert_eq!(plan(bytes, 1000), Err(MdnsError::InvalidPacket));
}

#[test]
fn responses_nonstandard_opcodes_and_nonzero_rcodes_are_errors_not_empty_selections() {
    for flags in [0x8000, 0x8400, 0x0800, 0x1000, 0x7800, 1, 15] {
        invalid(&packet(flags, &questions_all(), &[], &[], &[]));
    }
}

#[test]
fn every_truncated_prefix_of_a_query_with_known_answers_is_rejected() {
    let bytes = packet(
        0,
        &questions_all(),
        &[local(12, 50), local(33, 50), local(16, 50), local(1, 50)],
        &[],
        &[],
    );
    for end in 0..bytes.len() {
        assert!(plan(&bytes[..end], 1000).is_err(), "accepted prefix {end}");
    }
}

#[test]
fn all_suppressed_does_not_hide_malformed_later_known_in_data() {
    for kind in [1, 12, 16, 33] {
        let bad = record("unrelated.local.", kind, vec![0xff], 1);
        let answers = [local(12, 50), local(33, 50), local(16, 50), local(1, 50)];
        invalid(&packet(0, &questions_all(), &answers, &[], &[bad]));
    }
}

#[test]
fn full_known_record_validation_precedes_requires_context() {
    for kind in [1, 12, 16, 33] {
        let bad = record("unrelated.local.", kind, vec![0xff], 1);
        invalid(&packet(
            0x0200,
            &[question("unrelated.local.", 1, 0x8001)],
            &[],
            &[],
            &[bad.clone()],
        ));
        invalid(&packet(0, &[], &[], &[bad], &[]));
    }
}

#[test]
fn known_answer_cache_flush_is_rejected_for_each_known_in_type() {
    for kind in [1, 12, 16, 33] {
        let mut rr = local(kind, u32::MAX);
        rr.class = 0x8001;
        invalid(&packet(0, &questions_all(), &[rr.clone()], &[], &[]));
        rr.owner = name("unrelated.local.");
        invalid(&packet(0x0200, &questions_all(), &[rr], &[], &[]));
    }
}

#[test]
fn additional_flush_bits_are_not_known_answers_or_unicast_question_bits() {
    let mut rr = local(12, u32::MAX);
    rr.class = 0x8001;
    assert_eq!(
        plan(&packet(0, &questions_all(), &[], &[], &[rr]), 1000),
        Ok(selected(all(), Set::default()))
    );
}

#[test]
fn pseudo_record_class_bits_remain_opaque_not_cache_flush_flags() {
    let mut rr = record(".", 41, vec![0xff], 1);
    rr.class = 0x8001;
    assert_eq!(
        plan(&packet(0, &questions_all(), &[rr], &[], &[]), 1000),
        Ok(selected(all(), Set::default()))
    );
}

#[test]
fn trailing_data_is_rejected_even_after_an_all_suppressed_match() {
    let mut bytes = packet(0, &[question(SERVICE, 12, 1)], &[local(12, 50)], &[], &[]);
    bytes.push(0);
    invalid(&bytes);
}

#[test]
fn exact_packet_size_is_accepted_and_one_more_is_rejected() {
    let mut rr = record("other.local.", 65000, vec![], 1);
    let base = packet(0, &[question(SERVICE, 12, 1)], &[], &[], &[rr.clone()]).len();
    rr.data.resize(MAX_MDNS_PACKET_BYTES - base, 0xff);
    let bytes = packet(0, &[question(SERVICE, 12, 1)], &[], &[], &[rr.clone()]);
    assert_eq!(bytes.len(), 4096);
    assert_eq!(plan(&bytes, 1000), Ok(selected(set(12), Set::default())));
    rr.data.push(0xff);
    assert_eq!(
        plan(
            &packet(0, &[question(SERVICE, 12, 1)], &[], &[], &[rr]),
            1000
        ),
        Err(MdnsError::TooLarge)
    );
}

#[test]
fn question_budget_rejects_nine_without_returning_a_partial_match() {
    invalid(&packet(
        0,
        &vec![question(SERVICE, 12, 1); 9],
        &[],
        &[],
        &[],
    ));
}

#[test]
fn total_record_budget_spans_all_sections_before_context_or_suppression() {
    let rrs = vec![local(12, 50); MAX_MDNS_RECORDS];
    assert_eq!(
        plan(
            &packet(0, &[question(SERVICE, 12, 1)], &rrs, &[], &[]),
            1000
        ),
        Ok(selected(set(12), set(12)))
    );
    invalid(&packet(
        0,
        &[question(SERVICE, 12, 1)],
        &rrs[..20],
        &rrs[20..40],
        &vec![local(12, 50); 25],
    ));
}

#[test]
fn tc_never_allows_missing_declared_packet_bytes() {
    let mut bytes = packet(0x0200, &questions_all(), &[local(12, 50)], &[], &[]);
    bytes.pop();
    invalid(&bytes);
}

#[test]
fn malformed_txt_lengths_and_srv_targets_are_not_equivalent_known_answers() {
    for data in [vec![1], vec![2, b'a'], vec![255]] {
        invalid(&packet(
            0,
            &questions_all(),
            &[record(INSTANCE, 16, data, 50)],
            &[],
            &[],
        ));
    }
    for data in [vec![], vec![0; 6], vec![0, 0, 0, 0, 0, 1, 0, 0]] {
        invalid(&packet(
            0,
            &questions_all(),
            &[record(INSTANCE, 33, data, 50)],
            &[],
            &[],
        ));
    }
}

#[test]
fn non_in_known_type_data_does_not_receive_in_specific_rdata_validation() {
    for kind in [1, 12, 16, 33] {
        let mut rr = record("unrelated.local.", kind, vec![0xff], 1);
        rr.class = 3;
        assert_eq!(
            plan(&packet(0, &questions_all(), &[rr], &[], &[]), 1000),
            Ok(selected(all(), Set::default()))
        );
    }
}
