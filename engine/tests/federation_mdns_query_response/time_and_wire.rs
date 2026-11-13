use super::*;

#[test]
fn fresh_sample_caps_every_answer_and_additional_ttl_at_original_expiry() {
    for (now, ttl) in [(1000, 100), (1050, 50), (1097, 3), (1099, 1)] {
        check(&browse_query(), now, &[12], &[33, 16, 1], ttl);
    }
}
#[test]
fn requested_ttl_is_preserved_when_it_is_shorter_than_remaining_lifetime() {
    let bytes = announcement(7)
        .prepare_query_response(&browse_query(), 5353, true, 1050)
        .unwrap();
    let MdnsQueryResponse::Packet(bytes) = bytes else {
        panic!("expected packet")
    };
    assert_eq!(
        inspect(&bytes),
        (expected(&[12], 7), expected(&[33, 16, 1], 7))
    );
}
#[test]
fn odd_ttl_additional_suppression_uses_exact_half_threshold() {
    for ttl in [1_u32, 3, 5, 99] {
        let ad = announcement(ttl);
        for known in [ttl / 2, ttl.div_ceil(2)] {
            let input = query(&[(SERVICE, 12, 1)], &[local(33, known)]);
            let MdnsQueryResponse::Packet(bytes) =
                ad.prepare_query_response(&input, 5353, true, 1000).unwrap()
            else {
                panic!("expected packet")
            };
            let extras: &[u16] = if known * 2 >= ttl {
                &[16, 1]
            } else {
                &[33, 16, 1]
            };
            assert_eq!(
                inspect(&bytes),
                (expected(&[12], ttl), expected(extras, ttl))
            );
        }
    }
}
#[test]
fn maximum_known_ttl_does_not_overflow_additional_suppression() {
    check(
        &query(&[(SERVICE, 12, 1)], &[local(33, u32::MAX)]),
        1000,
        &[12],
        &[16, 1],
        100,
    );
}
#[test]
fn previous_plan_is_not_reused_for_later_ttl_and_suppression() {
    let ad = announcement(100);
    let input = query(&[(SERVICE, 12, 1)], &[local(12, 2)]);
    assert!(matches!(
        ad.plan_query(&input, 5353, true, 1000),
        Ok(MdnsQueryPlan::Multicast {
            suppressed: MdnsQueryRecordSet { ptr: false, .. },
            ..
        })
    ));
    assert_eq!(
        ad.prepare_query_response(&input, 5353, true, 1097),
        Ok(MdnsQueryResponse::AllDirectAnswersSuppressed)
    );
}
#[test]
fn clock_before_construction_is_rejected_before_no_match_or_suppression() {
    for input in [
        query(&[("other.local.", 1, 1)], &[]),
        query(&[(SERVICE, 12, 1)], &[local(12, 50)]),
    ] {
        assert_eq!(prepare(&input, 999), Err(MdnsError::ClockInvalid));
    }
}
#[test]
fn expiry_is_terminal_for_response_preparation_not_a_goodbye_branch() {
    let ad = announcement(100);
    for now in [1100, 1101, u64::MAX] {
        let response = ad.prepare_query_response(&browse_query(), 5353, true, now);
        assert!(response.is_err());
        assert_eq!(
            response.unwrap_err(),
            ad.plan_query(&browse_query(), 5353, true, now).unwrap_err()
        );
    }
    assert!(ad.encode_goodbye().is_ok());
}
#[test]
fn stateless_samples_do_not_claim_cross_call_rollback_detection() {
    let ad = announcement(100);
    for (now, ttl) in [(1090, 10), (1000, 100)] {
        let MdnsQueryResponse::Packet(bytes) = ad
            .prepare_query_response(&browse_query(), 5353, true, now)
            .unwrap()
        else {
            panic!("expected packet")
        };
        assert_eq!(
            inspect(&bytes),
            (expected(&[12], ttl), expected(&[33, 16, 1], ttl))
        );
    }
}
#[test]
fn query_id_and_ignored_flags_are_never_echoed_into_prepared_header() {
    let mut input = browse_query();
    input[0..2].copy_from_slice(&0xabcd_u16.to_be_bytes());
    input[2..4].copy_from_slice(&0x05f0_u16.to_be_bytes());
    check(&input, 1000, &[12], &[33, 16, 1], 100);
}
#[test]
fn mixed_case_and_compressed_questions_encode_only_frozen_canonical_names() {
    let mut input = query(&[("NoDe-OnE._CyAnReX._UdP.LoCaL.", 33, 1)], &[]);
    input[5] = 2;
    input.extend([0xc0, 12, 0, 16, 0, 1]);
    check(&input, 1000, &[33, 16], &[1], 100);
}
#[test]
fn compressed_known_srv_target_is_compared_but_never_copied_as_output_pointer() {
    let qs = [(SERVICE, 12, 1), ("HoSt-OnE.LoCaL.", 1, 1)];
    let target = 12 + name(SERVICE).len() + 4;
    let mut known = local(33, 50);
    known.data.truncate(6);
    known.data.extend((0xc000 | target as u16).to_be_bytes());
    check(&query(&qs, &[known]), 1000, &[12, 1], &[16], 100);
}
#[test]
fn compressed_ptr_known_answer_suppresses_without_creating_additional_only_output() {
    let qs = [(SERVICE, 12, 1), (INSTANCE, 16, 1)];
    let target = 12 + name(SERVICE).len() + 4;
    let mut known = local(12, 50);
    known.owner = vec![0xc0, 12];
    known.data = (0xc000 | target as u16).to_be_bytes().to_vec();
    check(&query(&qs, &[known]), 1000, &[16], &[], 100);
}
#[test]
fn refactored_announcement_and_goodbye_keep_the_exact_old_four_answer_layout() {
    let ad = announcement(100);
    for (actual, ttl) in [
        (ad.encode_at(1000).unwrap(), 100),
        (ad.encode_goodbye().unwrap(), 0),
    ] {
        let mut wanted = vec![0, 0, 0x84, 0, 0, 0, 0, 4, 0, 0, 0, 0];
        for rr in expected(&[12, 33, 16, 1], ttl) {
            append(&mut wanted, &rr);
        }
        assert_eq!(actual, wanted);
    }
}
#[test]
fn refactored_probe_keeps_exact_old_questions_and_unflushed_authority_records() {
    let mut wanted = vec![0, 0, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0];
    for owner in [INSTANCE, HOST] {
        wanted.extend(name(owner));
        wanted.extend([0, 255, 0x80, 1]);
    }
    for kind in [33, 16, 1] {
        append(&mut wanted, &local(kind, 100));
    }
    assert_eq!(announcement(100).encode_probe_at(1000).unwrap(), wanted);
}
