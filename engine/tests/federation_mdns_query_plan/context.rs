use super::*;

fn require(bytes: &[u8], source_port: u16, multicast: bool, reasons: MdnsQueryContext) {
    assert_eq!(
        announcement(100).plan_query(bytes, source_port, multicast, 1000),
        Ok(MdnsQueryPlan::RequiresContext(reasons))
    );
}

#[test]
fn a_truncated_known_answer_sequence_is_not_a_partial_multicast_plan() {
    let bytes = packet(0x0200, &questions_all(), &[local(12, u32::MAX)], &[], &[]);
    require(
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
fn even_an_unrelated_qu_question_defers_the_whole_mixed_query() {
    let mut questions = questions_all();
    questions.push(question("unrelated.local.", 1, 0x8001));
    require(
        &packet(0, &questions, &[], &[], &[]),
        5353,
        true,
        MdnsQueryContext {
            unicast_question: true,
            ..Default::default()
        },
    );
}

#[test]
fn any_authority_records_require_context_instead_of_being_known_answers() {
    for rr in [
        local(12, u32::MAX),
        record("unrelated.local.", 65_000, vec![0xff], 1),
    ] {
        require(
            &packet(0, &questions_all(), &[], &[rr], &[]),
            5353,
            true,
            MdnsQueryContext {
                authority_records: true,
                ..Default::default()
            },
        );
    }
}

#[test]
fn legacy_source_ports_are_not_inferred_from_a_zero_query_id() {
    for source in [1, 53, 49152, u16::MAX] {
        require(
            &browse_query(),
            source,
            true,
            MdnsQueryContext {
                legacy_port: true,
                ..Default::default()
            },
        );
    }
}

#[test]
fn direct_unicast_requires_routing_context_even_from_source_port_5353() {
    require(
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
fn zero_questions_require_sequence_context_even_with_no_records() {
    for answers in [vec![], vec![local(12, u32::MAX)]] {
        require(
            &packet(0, &[], &answers, &[], &[]),
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
fn all_context_reasons_accumulate_without_a_precedence_loss() {
    let bytes = packet(
        0x0200,
        &[question(SERVICE, 12, 0x8001)],
        &[],
        &[local(1, 1)],
        &[],
    );
    require(
        &bytes,
        53,
        false,
        MdnsQueryContext {
            truncated: true,
            unicast_question: true,
            authority_records: true,
            legacy_port: true,
            direct_unicast: true,
            no_questions: false,
        },
    );
    let bytes = packet(0x0200, &[], &[], &[local(1, 1)], &[]);
    require(
        &bytes,
        53,
        false,
        MdnsQueryContext {
            truncated: true,
            authority_records: true,
            legacy_port: true,
            direct_unicast: true,
            no_questions: true,
            ..Default::default()
        },
    );
}

#[test]
fn zero_source_port_is_invalid_configuration_not_an_implicit_mdns_source() {
    assert_eq!(
        announcement(100).plan_query(&browse_query(), 0, true, 1000),
        Err(MdnsError::InvalidConfig)
    );
}

#[test]
fn no_local_match_does_not_erase_unicast_or_truncated_context() {
    let bytes = packet(
        0x0200,
        &[question("unrelated.local.", 65_000, 0x8001)],
        &[],
        &[],
        &[],
    );
    require(
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
