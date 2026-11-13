use super::*;

#[test]
fn each_matching_answer_suppresses_only_its_requested_record() {
    for kind in [12, 33, 16, 1] {
        assert_eq!(
            plan(
                &packet(0, &questions_all(), &[local(kind, 50)], &[], &[]),
                1000
            ),
            Ok(selected(all(), set(kind)))
        );
    }
}

#[test]
fn requested_and_fully_suppressed_are_not_the_same_as_no_matching_question() {
    let answers = [local(12, 50), local(33, 50), local(16, 50), local(1, 50)];
    assert_eq!(
        plan(&packet(0, &questions_all(), &answers, &[], &[]), 1000),
        Ok(selected(all(), all()))
    );
    assert_eq!(
        plan(
            &packet(0, &[question("other.local.", 1, 1)], &answers, &[], &[]),
            1000
        ),
        Ok(selected(Set::default(), Set::default()))
    );
}

#[test]
fn odd_ttls_use_the_exact_half_threshold_not_integer_rounding_down() {
    for ttl in [1_u32, 3, 5, 99] {
        let ad = announcement(ttl);
        for known in [ttl / 2, ttl.div_ceil(2)] {
            let bytes = packet(
                0,
                &[question(SERVICE, 12, 1)],
                &[local(12, known)],
                &[],
                &[],
            );
            let suppressed = if u64::from(known) * 2 >= u64::from(ttl) {
                set(12)
            } else {
                Set::default()
            };
            assert_eq!(
                ad.plan_query(&bytes, 5353, true, 1000),
                Ok(selected(set(12), suppressed))
            );
        }
    }
}

#[test]
fn ttl_below_half_and_zero_do_not_suppress_live_records() {
    for known in [0, 1, 49] {
        assert_eq!(
            plan(
                &packet(0, &questions_all(), &[local(12, known)], &[], &[]),
                1000
            ),
            Ok(selected(all(), Set::default()))
        );
    }
}

#[test]
fn maximum_known_ttl_cannot_overflow_the_comparison() {
    let bytes = packet(
        0,
        &questions_all(),
        &[
            local(12, u32::MAX),
            local(33, u32::MAX),
            local(16, u32::MAX),
            local(1, u32::MAX),
        ],
        &[],
        &[],
    );
    assert_eq!(plan(&bytes, 1000), Ok(selected(all(), all())));
}

#[test]
fn f0_remaining_lifetime_caps_the_local_ttl_used_by_suppression() {
    let ad = announcement(120);
    for (now, known, suppressed) in [
        (1000, 49, false),
        (1000, 50, true),
        (1097, 1, false),
        (1097, 2, true),
        (1099, 0, false),
        (1099, 1, true),
    ] {
        let bytes = packet(
            0,
            &[question(SERVICE, 12, 1)],
            &[local(12, known)],
            &[],
            &[],
        );
        assert_eq!(
            ad.plan_query(&bytes, 5353, true, now),
            Ok(selected(
                set(12),
                if suppressed { set(12) } else { Set::default() }
            ))
        );
    }
}

#[test]
fn duplicate_low_ttls_are_not_added_together() {
    let answers = vec![local(12, 49); 10];
    assert_eq!(
        plan(
            &packet(0, &[question(SERVICE, 12, 1)], &answers, &[], &[]),
            1000
        ),
        Ok(selected(set(12), Set::default()))
    );
}

#[test]
fn any_one_equivalent_answer_above_threshold_suffices_without_order_dependence() {
    for answers in [
        vec![local(12, 1), local(12, 50)],
        vec![local(12, 50), local(12, 1)],
    ] {
        assert_eq!(
            plan(
                &packet(0, &[question(SERVICE, 12, 1)], &answers, &[], &[]),
                1000
            ),
            Ok(selected(set(12), set(12)))
        );
    }
}

#[test]
fn additional_records_do_not_suppress_direct_answers() {
    assert_eq!(
        plan(
            &packet(
                0,
                &questions_all(),
                &[],
                &[],
                &[local(12, u32::MAX), local(1, u32::MAX)]
            ),
            1000
        ),
        Ok(selected(all(), Set::default()))
    );
}

#[test]
fn a_known_answer_without_a_matching_question_is_not_exported_as_suppression() {
    let bytes = packet(
        0,
        &[question(SERVICE, 12, 1)],
        &[local(1, u32::MAX), local(33, u32::MAX)],
        &[],
        &[],
    );
    assert_eq!(plan(&bytes, 1000), Ok(selected(set(12), Set::default())));
}

#[test]
fn different_ptr_target_and_different_ipv4_bytes_do_not_suppress() {
    let answers = [
        record(SERVICE, 12, name("other._cyanrex._udp.local."), u32::MAX),
        record(HOST, 1, vec![192, 168, 1, 8], u32::MAX),
    ];
    assert_eq!(
        plan(&packet(0, &questions_all(), &answers, &[], &[]), 1000),
        Ok(selected(all(), Set::default()))
    );
}

#[test]
fn all_srv_numeric_fields_and_target_must_match_before_suppression() {
    for offset in [1, 3, 5] {
        let mut rr = local(33, u32::MAX);
        rr.data[offset] ^= 1;
        assert_eq!(
            plan(&packet(0, &questions_all(), &[rr], &[], &[]), 1000),
            Ok(selected(all(), Set::default()))
        );
    }
    let mut rr = local(33, u32::MAX);
    rr.data.truncate(6);
    rr.data.extend(name("different.example."));
    assert_eq!(
        plan(&packet(0, &questions_all(), &[rr], &[], &[]), 1000),
        Ok(selected(all(), Set::default()))
    );
}

#[test]
fn txt_case_order_and_raw_non_f0_data_do_not_become_equivalent_known_answers() {
    let mut case = local(16, u32::MAX);
    case.data[1] = b'V';
    let mut order = local(16, u32::MAX);
    let first = 1 + order.data[0] as usize;
    order.data.rotate_left(first);
    for rr in [
        case,
        order,
        record(INSTANCE, 16, vec![], u32::MAX),
        record(INSTANCE, 16, vec![0], u32::MAX),
        record(INSTANCE, 16, vec![2, 0xc0, 0x0c], u32::MAX),
    ] {
        assert_eq!(
            plan(&packet(0, &questions_all(), &[rr], &[], &[]), 1000),
            Ok(selected(all(), Set::default()))
        );
    }
}

#[test]
fn non_in_or_unknown_answer_data_is_opaque_and_cannot_suppress() {
    let mut rr = local(12, u32::MAX);
    rr.class = 3;
    rr.data = vec![0xff];
    let other = record(SERVICE, 65_000, vec![0xc0, 0xff], u32::MAX);
    assert_eq!(
        plan(&packet(0, &questions_all(), &[rr, other], &[], &[]), 1000),
        Ok(selected(all(), Set::default()))
    );
}
