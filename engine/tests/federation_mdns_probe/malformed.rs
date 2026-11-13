use super::*;

fn pointer(offset: usize) -> Vec<u8> {
    assert!(offset < 0x4000);
    (0xc000 | offset as u16).to_be_bytes().to_vec()
}

#[test]
fn rejects_every_nonprofile_flag_instead_of_returning_an_early_matching_claim() {
    for flags in [
        0x0001, 0x0010, 0x0020, 0x0040, 0x0080, 0x0100, 0x0200, 0x0400, 0x0800, 0x8001, 0x8100,
        0x8200, 0x8800, 0xffff,
    ] {
        assert_invalid(&packet(flags, &[], &[record(HOST, 1)], &[], &[]));
    }
}

#[test]
fn nonzero_id_is_rejected_for_response_and_query() {
    for mut bytes in [
        packet(0x8400, &[], &[record(HOST, 1)], &[], &[]),
        packet(0, &[question(HOST, 255, 1)], &[], &[record(HOST, 1)], &[]),
    ] {
        bytes[1] = 1;
        assert_invalid(&bytes);
    }
}

#[test]
fn both_allowed_response_flag_values_are_accepted() {
    for flags in [0x8000, 0x8400] {
        assert_eq!(
            observe(&packet(flags, &[], &[record(HOST, 1)], &[], &[])),
            Ok(ProbeObservation::ExistingResponse)
        );
    }
}

#[test]
fn truncation_anywhere_in_an_encoded_probe_never_becomes_no_relevant_claim() {
    let bytes = announcement().encode_probe_at(1000).unwrap();
    for length in 0..bytes.len() {
        assert!(
            observe(&bytes[..length]).is_err(),
            "accepted prefix {length}"
        );
    }
}

#[test]
fn trailing_bytes_are_rejected_even_after_a_valid_matching_claim() {
    for flags in [0x8400, 0] {
        let q = if flags == 0 {
            vec![question(HOST, 255, 1)]
        } else {
            vec![]
        };
        let rr = record(HOST, 1);
        let mut bytes = if flags == 0 {
            packet(flags, &q, &[], &[rr], &[])
        } else {
            packet(flags, &q, &[rr], &[], &[])
        };
        bytes.push(0);
        assert_invalid(&bytes);
    }
}

#[test]
fn oversized_packet_rejected_before_classification_and_exact_size_is_accepted() {
    let mut rr = record(HOST, 65_000);
    rr.data.clear();
    let base = packet(0x8400, &[], &[rr.clone()], &[], &[]).len();
    rr.data.resize(MAX_MDNS_PACKET_BYTES - base, 0xff);
    let mut bytes = packet(0x8400, &[], &[rr.clone()], &[], &[]);
    assert_eq!(bytes.len(), 4096);
    assert_eq!(observe(&bytes), Ok(ProbeObservation::ExistingResponse));
    rr.data.push(0xff);
    bytes = packet(0x8400, &[], &[rr], &[], &[]);
    assert_eq!(observe(&bytes), Err(MdnsError::TooLarge));
}

#[test]
fn eight_questions_are_accepted_ninth_rejected_even_with_a_valid_claim() {
    let qs = vec![question(HOST, 255, 0x8001); 8];
    assert_eq!(
        observe(&packet(0, &qs, &[], &[record(HOST, 1)], &[])),
        Ok(ProbeObservation::CompetingProbe)
    );
    let qs = vec![question(HOST, 255, 0x8001); 9];
    assert_invalid(&packet(0, &qs, &[], &[record(HOST, 1)], &[]));
}

#[test]
fn total_record_budget_is_shared_across_all_three_rr_sections() {
    assert_eq!(MAX_MDNS_RECORDS, 64);
    let rr = record(HOST, 1);
    let sixty_four = vec![rr.clone(); MAX_MDNS_RECORDS];
    assert_eq!(
        observe(&packet(0x8400, &[], &sixty_four, &[], &[])),
        Ok(ProbeObservation::ExistingResponse)
    );
    assert_invalid(&packet(
        0x8400,
        &[],
        &sixty_four[..20],
        &sixty_four[20..40],
        &vec![rr; 25],
    ));
}

#[test]
fn truncated_later_records_and_questions_are_not_hidden_by_an_earlier_match() {
    let valid = record(HOST, 1);
    let mut bytes = packet(0x8400, &[], &[valid.clone()], &[], &[]);
    bytes[11] = 1;
    assert_invalid(&bytes);
    let mut bytes = packet(0x8400, &[], &[valid], &[], &[]);
    bytes[5] = 1;
    assert_invalid(&bytes);
}

#[test]
fn owner_compression_to_a_prior_question_is_supported_and_case_insensitive() {
    let mut bytes = packet(
        0,
        &[question("HoSt-OnE.LoCaL.", 255, 0x8001)],
        &[],
        &[],
        &[],
    );
    bytes[9] = 1;
    append_rr(&mut bytes, &pointer(12), &record(HOST, 1));
    assert_eq!(observe(&bytes), Ok(ProbeObservation::CompetingProbe));
    bytes[2..4].copy_from_slice(&0x8400_u16.to_be_bytes());
    assert_eq!(observe(&bytes), Ok(ProbeObservation::ExistingResponse));
}

#[test]
fn compression_can_refer_to_a_prior_full_owner_across_rr_sections() {
    let mut bytes = packet(0x8400, &[], &[record(HOST, 1)], &[], &[]);
    bytes[11] = 1;
    append_rr(&mut bytes, &pointer(12), &record(HOST, 16));
    assert_eq!(observe(&bytes), Ok(ProbeObservation::ExistingResponse));
}

#[test]
fn pointer_cycles_forward_references_out_of_bounds_and_reserved_labels_are_rejected() {
    for owner in [
        pointer(12),
        pointer(14),
        pointer(4095),
        vec![0xc0],
        vec![0x40, 0],
        vec![0x80, 0],
    ] {
        let mut bytes = vec![0, 0, 0x84, 0, 0, 0, 0, 1, 0, 0, 0, 0];
        append_rr(&mut bytes, &owner, &record(HOST, 1));
        assert_invalid(&bytes);
    }
}

#[test]
fn a_later_malformed_compressed_owner_is_not_hidden_by_existing_response() {
    let mut bytes = packet(0x8400, &[], &[record(HOST, 1)], &[], &[]);
    bytes[11] = 1;
    let at = bytes.len();
    append_rr(&mut bytes, &pointer(at), &record(HOST, 1));
    assert_invalid(&bytes);
}

#[test]
fn invalid_ascii_and_overexpanded_owner_names_are_rejected_before_classification() {
    for label in [vec![0], vec![0x1f], vec![0x7f], vec![0x80], vec![0xff]] {
        let mut owner = vec![label.len() as u8];
        owner.extend(label);
        owner.push(0);
        let mut bytes = vec![0, 0, 0x84, 0, 0, 0, 0, 1, 0, 0, 0, 0];
        append_rr(&mut bytes, &owner, &record(HOST, 1));
        assert_invalid(&bytes);
    }
    let mut owner = Vec::new();
    for _ in 0..4 {
        owner.push(63);
        owner.extend([b'x'; 63]);
    }
    owner.push(0);
    let mut bytes = vec![0, 0, 0x84, 0, 0, 0, 0, 1, 0, 0, 0, 0];
    append_rr(&mut bytes, &owner, &record(HOST, 1));
    assert_invalid(&bytes);
}

#[test]
fn opaque_rdata_is_not_followed_as_a_compressed_name_or_lexicographic_tiebreak() {
    let mut rr = record(HOST, 33);
    rr.data = vec![0xc0, 0x0c];
    assert_eq!(
        observe(&packet(0x8400, &[], &[rr.clone()], &[], &[])),
        Ok(ProbeObservation::ExistingResponse)
    );
    assert_eq!(
        observe(&packet(0, &[question(HOST, 255, 1)], &[], &[rr], &[])),
        Ok(ProbeObservation::CompetingProbe)
    );
}

#[test]
fn rr_declared_rdata_length_cannot_escape_packet_even_after_earlier_match() {
    let mut bytes = packet(0x8400, &[], &[record(HOST, 1)], &[], &[]);
    bytes[11] = 1;
    let start = bytes.len();
    append_rr(
        &mut bytes,
        &dns_name("other.local."),
        &record("other.local.", 1),
    );
    let length_at = start + dns_name("other.local.").len() + 8;
    bytes[length_at..length_at + 2].copy_from_slice(&u16::MAX.to_be_bytes());
    assert_invalid(&bytes);
}

#[test]
fn exact_expanded_name_limit_is_accepted_but_one_extra_byte_rejected() {
    for (last, expected) in [
        (61, Ok(ProbeObservation::NoRelevantClaim)),
        (62, Err(MdnsError::InvalidPacket)),
    ] {
        let mut owner = Vec::new();
        for length in [63, 63, 63, last] {
            owner.push(length);
            owner.extend(vec![b'x'; length as usize]);
        }
        owner.push(0);
        let mut bytes = vec![0, 0, 0x84, 0, 0, 0, 0, 1, 0, 0, 0, 0];
        append_rr(&mut bytes, &owner, &record(HOST, 1));
        assert_eq!(observe(&bytes), expected);
    }
}

#[test]
fn compression_jump_budget_accepts_128_but_rejects_129_backwards_pointers() {
    for jumps in [128, 129] {
        let mut holder = record("other.local.", 65_000);
        let data_start = 12 + dns_name(&holder.owner).len() + 10;
        holder.data = dns_name(HOST);
        let mut last = data_start;
        for _ in 1..jumps {
            let next = data_start + holder.data.len();
            holder.data.extend(pointer(last));
            last = next;
        }
        let mut bytes = packet(0x8400, &[], &[holder], &[], &[]);
        bytes[11] = 1;
        append_rr(&mut bytes, &pointer(last), &record(HOST, 1));
        if jumps == 128 {
            assert_eq!(observe(&bytes), Ok(ProbeObservation::ExistingResponse));
        } else {
            assert_invalid(&bytes);
        }
    }
}

#[test]
fn malformed_question_name_is_not_ignored_in_an_otherwise_existing_response() {
    let mut bytes = packet(
        0x8400,
        &[question(HOST, 255, 1)],
        &[record(HOST, 1)],
        &[],
        &[],
    );
    bytes[12] = 0x40;
    assert_invalid(&bytes);
}

#[test]
fn response_with_no_matching_rr_and_valid_questions_is_only_no_relevant_claim() {
    for flags in [0x8000, 0x8400] {
        assert_eq!(
            observe(&packet(
                flags,
                &[question(HOST, 255, 0x8001)],
                &[],
                &[],
                &[]
            )),
            Ok(ProbeObservation::NoRelevantClaim)
        );
    }
}
