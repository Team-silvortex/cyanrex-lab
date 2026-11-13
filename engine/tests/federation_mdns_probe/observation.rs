use super::*;

#[test]
fn response_any_matching_owner_and_type_in_any_rr_section_is_an_existing_claim() {
    for owner in [INSTANCE, HOST] {
        for kind in [1, 16, 33, 28, 47, 65_000] {
            let rr = record(owner, kind);
            for section in 0..3 {
                let mut sections = [Vec::new(), Vec::new(), Vec::new()];
                sections[section].push(rr.clone());
                assert_eq!(
                    observe(&packet(
                        0x8400,
                        &[],
                        &sections[0],
                        &sections[1],
                        &sections[2]
                    )),
                    Ok(ProbeObservation::ExistingResponse),
                    "owner={owner}, type={kind}, section={section}"
                );
            }
        }
    }
}

#[test]
fn response_does_not_need_ptr_a_complete_service_or_valid_f0_txt() {
    for (owner, kind) in [(INSTANCE, 33), (INSTANCE, 16), (HOST, 1)] {
        let mut rr = record(owner, kind);
        rr.data = b"not valid SRV TXT or IPv4".to_vec();
        assert_eq!(
            observe(&packet(0x8000, &[], &[rr], &[], &[])),
            Ok(ProbeObservation::ExistingResponse)
        );
    }
}

#[test]
fn identical_live_response_is_neither_ignored_nor_treated_as_our_ownership() {
    assert_eq!(
        observe(&announcement().encode_at(1000).unwrap()),
        Ok(ProbeObservation::ExistingResponse)
    );
}

#[test]
fn zero_ttl_flush_bits_and_empty_or_opaque_rdata_do_not_erase_response_claims() {
    for class in [1, 0x8001] {
        for ttl in [0, 1, u32::MAX] {
            for data in [vec![], vec![0xc0, 0xff], vec![0, 255, 0]] {
                let mut rr = record(HOST, 65_000);
                rr.class = class;
                rr.ttl = ttl;
                rr.data = data;
                assert_eq!(
                    observe(&packet(0x8400, &[], &[rr], &[], &[])),
                    Ok(ProbeObservation::ExistingResponse)
                );
            }
        }
    }
}

#[test]
fn dns_owner_matching_uses_label_preserving_ascii_case_insensitivity() {
    let rr = record("NoDe-OnE._CyAnReX._UdP.LoCaL.", 33);
    assert_eq!(
        observe(&packet(0x8400, &[], &[rr], &[], &[])),
        Ok(ProbeObservation::ExistingResponse)
    );
    for owner in [
        "other.local.",
        SERVICE,
        "host-one.example.local.",
        "xhost-one.local.",
        "host-one.local.example.",
    ] {
        assert_eq!(
            observe(&packet(0x8400, &[], &[record(owner, 1)], &[], &[])),
            Ok(ProbeObservation::NoRelevantClaim)
        );
    }
}

#[test]
fn a_dot_inside_one_label_does_not_match_the_multilabel_local_name() {
    let mut bytes = vec![0, 0, 0x84, 0, 0, 0, 0, 1, 0, 0, 0, 0];
    let one_label = b"host-one.local";
    let mut owner = vec![one_label.len() as u8];
    owner.extend(one_label);
    owner.push(0);
    append_rr(&mut bytes, &owner, &record(HOST, 1));
    assert_eq!(observe(&bytes), Ok(ProbeObservation::NoRelevantClaim));
}

#[test]
fn different_record_class_is_not_our_in_class_even_if_name_is_equal() {
    for class in [0, 3, 255, 0x8003] {
        let mut rr = record(HOST, 1);
        rr.class = class;
        assert_eq!(
            observe(&packet(0x8400, &[], &[rr], &[], &[])),
            Ok(ProbeObservation::NoRelevantClaim)
        );
    }
}

#[test]
fn query_authority_requires_matching_in_question_type_or_any() {
    for owner in [INSTANCE, HOST] {
        for kind in [1, 16, 33, 65_000] {
            for qkind in [kind, 255] {
                for qclass in [1, 0x8001] {
                    let rr = record(owner, kind);
                    assert_eq!(
                        observe(&packet(
                            0,
                            &[question(owner, qkind, qclass)],
                            &[],
                            &[rr],
                            &[]
                        )),
                        Ok(ProbeObservation::CompetingProbe)
                    );
                }
            }
        }
    }
}

#[test]
fn query_known_answers_and_additional_records_do_not_establish_a_competing_claim() {
    let rr = record(HOST, 1);
    let q = question(HOST, 255, 0x8001);
    for qs in [vec![], vec![q]] {
        assert_eq!(
            observe(&packet(0, &qs, &[rr.clone()], &[], &[rr.clone()])),
            Ok(ProbeObservation::NoRelevantClaim)
        );
    }
}

#[test]
fn query_question_alone_never_proves_occupation_or_availability() {
    for owner in [HOST, INSTANCE] {
        assert_eq!(
            observe(&packet(0, &[question(owner, 255, 0x8001)], &[], &[], &[])),
            Ok(ProbeObservation::NoRelevantClaim)
        );
    }
    assert_eq!(
        observe(&packet(0, &[], &[], &[], &[])),
        Ok(ProbeObservation::NoRelevantClaim)
    );
}

#[test]
fn unrelated_query_authority_or_different_class_does_not_occupy_our_names() {
    for owner in [SERVICE, "other.local."] {
        assert_eq!(
            observe(&packet(0, &[], &[], &[record(owner, 1)], &[])),
            Ok(ProbeObservation::NoRelevantClaim)
        );
    }
    let mut rr = record(HOST, 1);
    rr.class = 3;
    assert_eq!(
        observe(&packet(0, &[], &[], &[rr], &[])),
        Ok(ProbeObservation::NoRelevantClaim)
    );
}

#[test]
fn matching_authority_without_answering_question_is_invalid_not_a_free_name() {
    let rr = record(HOST, 1);
    for questions in [
        vec![],
        vec![question(HOST, 16, 1)],
        vec![question(HOST, 255, 3)],
        vec![question(INSTANCE, 255, 1)],
    ] {
        assert_invalid(&packet(0, &questions, &[], &[rr.clone()], &[]));
    }
}

#[test]
fn authority_ttl_zero_identical_data_and_flush_do_not_create_a_tiebreak_win() {
    for class in [1, 0x8001] {
        let mut rr = record(HOST, 1);
        rr.ttl = 0;
        rr.class = class;
        assert_eq!(
            observe(&packet(0, &[question(HOST, 255, 1)], &[], &[rr], &[])),
            Ok(ProbeObservation::CompetingProbe)
        );
    }
}

#[test]
fn valid_matching_question_can_follow_an_unrelated_or_wrong_class_question() {
    let questions = [
        question(HOST, 16, 1),
        question(HOST, 255, 3),
        question(HOST, 1, 0x8001),
    ];
    assert_eq!(
        observe(&packet(0, &questions, &[], &[record(HOST, 1)], &[])),
        Ok(ProbeObservation::CompetingProbe)
    );
}

#[test]
fn one_bad_related_authority_is_not_hidden_by_another_matching_competitor() {
    let questions = [question(HOST, 255, 1)];
    for records in [
        vec![record(HOST, 1), record(INSTANCE, 16)],
        vec![record(INSTANCE, 16), record(HOST, 1)],
    ] {
        assert_invalid(&packet(0, &questions, &[], &records, &[]));
    }
}
