use super::*;

#[test]
fn srv_query_adds_only_the_frozen_address() {
    check(&query(&[(INSTANCE, 33, 1)], &[]), 1000, &[33], &[1], 100);
}
#[test]
fn txt_query_has_no_additional_dependency() {
    check(&query(&[(INSTANCE, 16, 1)], &[]), 1000, &[16], &[], 100);
}
#[test]
fn address_query_has_no_additional_dependency() {
    check(&query(&[(HOST, 1, 1)], &[]), 1000, &[1], &[], 100);
}
#[test]
fn instance_any_selects_srv_txt_and_one_additional_address() {
    check(
        &query(&[(INSTANCE, 255, 255)], &[]),
        1000,
        &[33, 16],
        &[1],
        100,
    );
}
#[test]
fn host_any_selects_only_the_frozen_address() {
    check(&query(&[(HOST, 255, 255)], &[]), 1000, &[1], &[], 100);
}
#[test]
fn service_any_selects_ptr_with_its_three_dependencies() {
    check(
        &query(&[(SERVICE, 255, 1)], &[]),
        1000,
        &[12],
        &[33, 16, 1],
        100,
    );
}
#[test]
fn direct_records_are_deduplicated_and_canonical_with_no_additional_copies() {
    let qs = [
        (HOST, 1, 1),
        (INSTANCE, 16, 1),
        (SERVICE, 12, 1),
        (INSTANCE, 33, 1),
        (SERVICE, 12, 255),
        (INSTANCE, 255, 1),
    ];
    check(&query(&qs, &[]), 1000, &[12, 33, 16, 1], &[], 100);
}
#[test]
fn an_explicit_address_answer_is_not_repeated_as_a_ptr_dependency() {
    check(
        &query(&[(SERVICE, 12, 1), (HOST, 1, 1)], &[]),
        1000,
        &[12, 1],
        &[33, 16],
        100,
    );
}
#[test]
fn nonrequested_known_srv_suppresses_additional_srv_but_not_ptr_address_dependency() {
    let input = query(&[(SERVICE, 12, 1)], &[local(33, 50)]);
    check(&input, 1000, &[12], &[16, 1], 100);
    assert_eq!(
        announcement(100).plan_query(&input, 5353, true, 1000),
        Ok(MdnsQueryPlan::Multicast {
            requested: MdnsQueryRecordSet {
                ptr: true,
                ..Default::default()
            },
            suppressed: MdnsQueryRecordSet::default(),
        })
    );
}
#[test]
fn nonrequested_known_txt_suppresses_only_additional_txt() {
    check(
        &query(&[(SERVICE, 12, 1)], &[local(16, 50)]),
        1000,
        &[12],
        &[33, 1],
        100,
    );
}
#[test]
fn nonrequested_known_address_suppresses_only_additional_address() {
    check(
        &query(&[(SERVICE, 12, 1)], &[local(1, 50)]),
        1000,
        &[12],
        &[33, 16],
        100,
    );
}
#[test]
fn every_dependency_may_be_known_without_suppressing_the_ptr_answer() {
    check(
        &query(
            &[(SERVICE, 12, 1)],
            &[local(33, 50), local(16, 50), local(1, 50)],
        ),
        1000,
        &[12],
        &[],
        100,
    );
}
#[test]
fn suppressed_ptr_does_not_create_dependencies_for_an_active_txt_answer() {
    check(
        &query(&[(SERVICE, 12, 1), (INSTANCE, 16, 1)], &[local(12, 50)]),
        1000,
        &[16],
        &[],
        100,
    );
}
#[test]
fn suppressed_ptr_leaves_only_active_srv_and_its_address_dependency() {
    check(
        &query(&[(SERVICE, 12, 1), (INSTANCE, 33, 1)], &[local(12, 50)]),
        1000,
        &[33],
        &[1],
        100,
    );
}
#[test]
fn suppressed_direct_srv_is_not_resurrected_as_a_ptr_additional() {
    check(
        &query(&[(SERVICE, 12, 1), (INSTANCE, 33, 1)], &[local(33, 50)]),
        1000,
        &[12],
        &[16, 1],
        100,
    );
}
#[test]
fn suppressed_direct_address_is_not_resurrected_as_a_srv_additional() {
    check(
        &query(&[(INSTANCE, 33, 1), (HOST, 1, 1)], &[local(1, 50)]),
        1000,
        &[33],
        &[],
        100,
    );
}
#[test]
fn suppressed_srv_never_generates_an_address_for_active_txt() {
    check(
        &query(&[(INSTANCE, 255, 1)], &[local(33, 50)]),
        1000,
        &[16],
        &[],
        100,
    );
}
#[test]
fn all_four_direct_answers_suppressed_return_a_typed_nonpacket() {
    let qs = [(SERVICE, 12, 1), (INSTANCE, 255, 1), (HOST, 1, 1)];
    let known = [local(12, 50), local(33, 50), local(16, 50), local(1, 50)];
    assert_eq!(
        prepare(&query(&qs, &known), 1000),
        Ok(MdnsQueryResponse::AllDirectAnswersSuppressed)
    );
}
#[test]
fn unrelated_questions_remain_no_match_even_with_all_known_answers() {
    let known = [local(12, 50), local(33, 50), local(16, 50), local(1, 50)];
    assert_eq!(
        prepare(&query(&[("else.local.", 255, 1)], &known), 1000),
        Ok(MdnsQueryResponse::NoMatchingQuestion)
    );
}
#[test]
fn non_in_questions_and_wrong_owner_types_cannot_select_a_frozen_record() {
    for qs in [
        [(SERVICE, 12, 3)],
        [(HOST, 33, 1)],
        [(INSTANCE, 1, 1)],
        [(SERVICE, 28, 1)],
    ] {
        assert_eq!(
            prepare(&query(&qs, &[]), 1000),
            Ok(MdnsQueryResponse::NoMatchingQuestion)
        );
    }
}
#[test]
fn different_rdata_and_opaque_peer_records_are_never_reflected() {
    let mut different = local(1, u32::MAX);
    different.data = vec![192, 168, 9, 99];
    let opaque = Record {
        owner: name("peer.local."),
        kind: 65000,
        class: 1,
        ttl: u32::MAX,
        data: b"peer-only-secret-marker".to_vec(),
    };
    let input = query(&[(SERVICE, 12, 1)], &[different, opaque]);
    check(&input, 1000, &[12], &[33, 16, 1], 100);
    assert!(!packet(&input, 1000)
        .windows(23)
        .any(|v| v == b"peer-only-secret-marker"));
}
#[test]
fn non_in_known_records_do_not_suppress_even_when_the_data_matches() {
    let mut rr = local(33, u32::MAX);
    rr.class = 3;
    check(
        &query(&[(SERVICE, 12, 1)], &[rr]),
        1000,
        &[12],
        &[33, 16, 1],
        100,
    );
}
#[test]
fn incoming_additional_records_are_not_known_answers() {
    let mut input = query(&[(SERVICE, 12, 1)], &[]);
    input[11] = 3;
    for rr in [local(33, 100), local(16, 100), local(1, 100)] {
        append(&mut input, &rr);
    }
    check(&input, 1000, &[12], &[33, 16, 1], 100);
}
#[test]
fn known_duplicate_ttls_are_not_added_but_any_sufficient_duplicate_wins() {
    let low = query(&[(SERVICE, 12, 1)], &vec![local(33, 49); 10]);
    check(&low, 1000, &[12], &[33, 16, 1], 100);
    for known in [
        vec![local(33, 49), local(33, 50)],
        vec![local(33, 50), local(33, 49)],
    ] {
        check(
            &query(&[(SERVICE, 12, 1)], &known),
            1000,
            &[12],
            &[16, 1],
            100,
        );
    }
}

#[test]
fn every_direct_and_known_mask_preserves_sections_and_never_revives_suppression() {
    let kinds = [12, 33, 16, 1];
    for direct in 0..16_u16 {
        for known in 0..16_u16 {
            let mut questions = vec![("unrelated.local.", 1, 1)];
            let mut records = Vec::new();
            let mut answer_kinds = Vec::new();
            for (index, kind) in kinds.iter().enumerate() {
                let selected = direct & (1 << index) != 0;
                let suppressed = known & (1 << index) != 0;
                if selected {
                    let owner = match kind {
                        12 => SERVICE,
                        33 | 16 => INSTANCE,
                        _ => HOST,
                    };
                    questions.push((owner, *kind, 1));
                    if !suppressed {
                        answer_kinds.push(*kind);
                    }
                }
                if suppressed {
                    records.push(local(*kind, 50));
                }
            }
            let input = query(&questions, &records);
            if direct == 0 {
                assert_eq!(
                    prepare(&input, 1000),
                    Ok(MdnsQueryResponse::NoMatchingQuestion)
                );
            } else if answer_kinds.is_empty() {
                assert_eq!(
                    prepare(&input, 1000),
                    Ok(MdnsQueryResponse::AllDirectAnswersSuppressed)
                );
            } else {
                let mut extras = Vec::new();
                for (index, kind) in kinds.iter().enumerate() {
                    let needed = match kind {
                        33 | 16 => answer_kinds.contains(&12),
                        1 => answer_kinds.contains(&12) || answer_kinds.contains(&33),
                        _ => false,
                    };
                    if needed && !answer_kinds.contains(kind) && known & (1 << index) == 0 {
                        extras.push(*kind);
                    }
                }
                check(&input, 1000, &answer_kinds, &extras, 100);
            }
        }
    }
}
