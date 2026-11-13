use super::*;

#[test]
fn rejects_queries_opcodes_and_rcodes_instead_of_reporting_absence() {
    for flags in [0, 0x0400, 0x0200, 0x8800, 0x9000, 0xf800] {
        assert_invalid(&packet(flags, &[local(1)], &[], &[]));
    }
    for rcode in 1..=15 {
        assert_invalid(&packet(0x8400 | rcode, &[local(1)], &[], &[]));
    }
}

#[test]
fn every_truncated_prefix_of_a_complete_announcement_is_rejected() {
    let bytes = announcement().encode_at(1000).unwrap();
    for end in 0..bytes.len() {
        assert!(observe(&bytes[..end]).is_err(), "accepted prefix {end}");
    }
}

#[test]
fn tc_does_not_allow_missing_bytes_even_when_the_bit_is_ignored() {
    let mut bytes = packet(0x8200, &[local(1)], &[], &[]);
    bytes.pop();
    assert_invalid(&bytes);
}

#[test]
fn a_short_or_long_a_record_is_invalid_not_a_conflict() {
    for length in [0, 1, 3, 5, 16] {
        assert_invalid(&packet(
            0x8400,
            &[record(HOST, 1, vec![0; length])],
            &[],
            &[],
        ));
    }
}

#[test]
fn srv_requires_six_fixed_bytes_and_one_complete_name() {
    for length in 0..7 {
        assert_invalid(&packet(
            0x8400,
            &[record(INSTANCE, 33, vec![0; length])],
            &[],
            &[],
        ));
    }
    let mut trailing = srv(0, 0, 4433, HOST);
    trailing.push(0);
    assert_invalid(&packet(0x8400, &[record(INSTANCE, 33, trailing)], &[], &[]));
}

#[test]
fn ptr_requires_exactly_one_complete_name_not_a_prefix_or_padding() {
    for data in [vec![], vec![2, b'a'], vec![0, 0], vec![0xc0], vec![0x40, 0]] {
        assert_invalid(&packet(0x8400, &[record(SERVICE, 12, data)], &[], &[]));
    }
}

#[test]
fn txt_string_lengths_must_fit_the_entire_record() {
    for data in [
        vec![1],
        vec![2, b'a'],
        vec![255, b'a'],
        vec![1, b'a', 2, b'b'],
    ] {
        assert_invalid(&packet(0x8400, &[record(INSTANCE, 16, data)], &[], &[]));
    }
}

#[test]
fn empty_strings_inside_a_txt_are_well_formed_different_data() {
    for data in [vec![0], vec![0, 0], vec![0, 1, b'a', 0]] {
        assert_eq!(
            observe(&packet(0x8400, &[record(INSTANCE, 16, data)], &[], &[])),
            Ok(Observation::ConflictingRecord)
        );
    }
}

#[test]
fn an_early_conflict_cannot_hide_later_bad_known_in_rdata_in_any_section() {
    let conflict = record(HOST, 1, vec![192, 168, 1, 8]);
    for kind in [1, 12, 16, 33] {
        let bad = record("unrelated.local.", kind, vec![0xff]);
        for section in 0..3 {
            let mut sections: [Vec<Record>; 3] = std::array::from_fn(|_| Vec::new());
            sections[0].push(conflict.clone());
            sections[section].push(bad.clone());
            assert_invalid(&packet(0x8400, &sections[0], &sections[1], &sections[2]));
        }
    }
}

#[test]
fn bad_known_in_rdata_before_a_conflict_is_also_rejected() {
    let conflict = record(HOST, 1, vec![192, 168, 1, 8]);
    for kind in [1, 12, 16, 33] {
        assert_invalid(&packet(
            0x8400,
            &[
                record("unrelated.local.", kind, vec![0xff]),
                conflict.clone(),
            ],
            &[],
            &[],
        ));
    }
}

#[test]
fn malformed_unrelated_in_data_never_becomes_no_conflicting_record() {
    for kind in [1, 12, 16, 33] {
        assert_invalid(&packet(
            0x8400,
            &[record("unrelated.local.", kind, vec![0xff])],
            &[],
            &[],
        ));
    }
}

#[test]
fn trailing_bytes_are_rejected_after_both_equal_and_conflicting_data() {
    for data in [vec![192, 168, 1, 7], vec![192, 168, 1, 8]] {
        let mut bytes = packet(0x8400, &[record(HOST, 1, data)], &[], &[]);
        bytes.push(0);
        assert_invalid(&bytes);
    }
}

#[test]
fn exact_packet_bound_is_accepted_and_one_more_is_too_large() {
    let mut rr = record(HOST, 65_000, vec![]);
    let base = packet(0x8400, &[rr.clone()], &[], &[]).len();
    rr.data.resize(MAX_MDNS_PACKET_BYTES - base, 0xff);
    let bytes = packet(0x8400, &[rr.clone()], &[], &[]);
    assert_eq!(bytes.len(), 4096);
    assert_eq!(observe(&bytes), Ok(Observation::NoConflictingRecord));
    rr.data.push(0xff);
    assert_eq!(
        observe(&packet(0x8400, &[rr], &[], &[])),
        Err(MdnsError::TooLarge)
    );
}

#[test]
fn record_budget_is_shared_across_sections_including_identical_duplicates() {
    let records = vec![local(1); MAX_MDNS_RECORDS];
    assert_eq!(
        observe(&packet(
            0x8400,
            &records[..20],
            &records[20..40],
            &records[40..]
        )),
        Ok(Observation::NoConflictingRecord)
    );
    let extra = vec![local(1); 25];
    assert_invalid(&packet(0x8400, &records[..20], &records[20..40], &extra));
}

fn questions(count: usize) -> Vec<u8> {
    let mut bytes = packet(0x8400, &[], &[], &[]);
    bytes[4..6].copy_from_slice(&(count as u16).to_be_bytes());
    for _ in 0..count {
        bytes.extend(name(HOST));
        bytes.extend([0, 255, 0x80, 1]);
    }
    bytes
}

#[test]
fn eight_questions_are_bounded_and_a_ninth_is_rejected() {
    assert_eq!(observe(&questions(8)), Ok(Observation::NoConflictingRecord));
    assert_invalid(&questions(9));
}

#[test]
fn missing_late_records_and_questions_fail_before_classification() {
    let mut bytes = packet(0x8400, &[record(HOST, 1, vec![192, 168, 1, 8])], &[], &[]);
    bytes[11] = 1;
    assert_invalid(&bytes);
    let mut bytes = questions(1);
    bytes.pop();
    assert_invalid(&bytes);
}

#[test]
fn declared_rdata_length_cannot_extend_into_a_nonexistent_packet_suffix() {
    let mut bytes = packet(0x8400, &[local(1)], &[], &[]);
    let at = 12 + name(HOST).len() + 8;
    bytes[at..at + 2].copy_from_slice(&u16::MAX.to_be_bytes());
    assert_invalid(&bytes);
}

#[test]
fn packet_budget_is_not_reset_for_each_record_or_section() {
    let data = vec![0xff; 2100];
    let rr = record("unrelated.local.", 65_000, data);
    assert_eq!(
        observe(&packet(0x8400, &[rr.clone()], &[], &[rr])),
        Err(MdnsError::TooLarge)
    );
}
