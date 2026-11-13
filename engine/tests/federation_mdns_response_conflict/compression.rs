use super::*;

fn pointer(offset: usize) -> Vec<u8> {
    assert!(offset < 0x4000);
    (0xc000 | offset as u16).to_be_bytes().to_vec()
}

fn question_prefix(owner: &str) -> Vec<u8> {
    let mut bytes = packet(0x8400, &[], &[], &[]);
    bytes[5] = 1;
    bytes.extend(name(owner));
    bytes.extend([0, 255, 0, 1]);
    bytes
}

#[test]
fn dns_owner_labels_are_ascii_case_insensitive_for_all_three_unique_tuples() {
    for kind in [1, 16, 33] {
        let mut rr = local(kind);
        rr.owner.make_ascii_uppercase();
        assert_eq!(
            observe(&packet(0x8400, &[rr.clone()], &[], &[])),
            Ok(Observation::NoConflictingRecord)
        );
        if kind == 33 {
            rr.data[5] ^= 1;
        } else {
            *rr.data.last_mut().unwrap() ^= 1;
        }
        assert_eq!(
            observe(&packet(0x8400, &[rr], &[], &[])),
            Ok(Observation::ConflictingRecord)
        );
    }
}

#[test]
fn srv_target_case_is_dns_name_case_not_raw_rdata_case() {
    let rr = record(INSTANCE, 33, srv(0, 0, 4433, "HoSt-OnE.LoCaL."));
    assert_eq!(
        observe(&packet(0x8400, &[rr], &[], &[])),
        Ok(Observation::NoConflictingRecord)
    );
}

#[test]
fn compressed_owner_can_reference_a_prior_question() {
    let mut bytes = question_prefix("NoDe-OnE._CyAnReX._UdP.LoCaL.");
    bytes[7] = 1;
    let mut rr = local(16);
    rr.owner = pointer(12);
    append(&mut bytes, &rr);
    assert_eq!(observe(&bytes), Ok(Observation::NoConflictingRecord));
}

#[test]
fn compressed_srv_target_can_reference_a_prior_question_without_slicing_offsets() {
    let mut bytes = question_prefix("HoSt-OnE.LoCaL.");
    bytes[7] = 1;
    let mut rr = local(33);
    rr.data.truncate(6);
    rr.data.extend(pointer(12));
    append(&mut bytes, &rr);
    assert_eq!(observe(&bytes), Ok(Observation::NoConflictingRecord));
}

#[test]
fn partial_srv_target_compression_preserves_the_original_label_boundaries() {
    let mut bytes = question_prefix("HoSt-OnE.LoCaL.");
    bytes[7] = 1;
    let mut rr = local(33);
    rr.data.truncate(6);
    rr.data.extend(b"\x08hOsT-oNe");
    rr.data.extend(pointer(21));
    append(&mut bytes, &rr);
    assert_eq!(observe(&bytes), Ok(Observation::NoConflictingRecord));
}

#[test]
fn srv_target_compression_can_cross_resource_record_sections() {
    let mut bytes = packet(0x8400, &[local(1)], &[], &[]);
    bytes[11] = 1;
    let mut rr = local(33);
    rr.data.truncate(6);
    rr.data.extend(pointer(12));
    append(&mut bytes, &rr);
    assert_eq!(observe(&bytes), Ok(Observation::NoConflictingRecord));
    let last_target = bytes.len() - 2;
    bytes[last_target - 1] ^= 1;
    assert_eq!(observe(&bytes), Ok(Observation::ConflictingRecord));
}

#[test]
fn compressed_different_srv_target_is_a_conflict() {
    let mut bytes = question_prefix("elsewhere.local.");
    bytes[7] = 1;
    let mut rr = local(33);
    rr.data.truncate(6);
    rr.data.extend(pointer(12));
    append(&mut bytes, &rr);
    assert_eq!(observe(&bytes), Ok(Observation::ConflictingRecord));
}

#[test]
fn root_srv_target_compression_is_valid_different_data() {
    let mut bytes = question_prefix(".");
    bytes[7] = 1;
    let mut rr = local(33);
    rr.data.truncate(6);
    rr.data.extend(pointer(12));
    append(&mut bytes, &rr);
    assert_eq!(observe(&bytes), Ok(Observation::ConflictingRecord));
}

#[test]
fn malformed_srv_target_compression_does_not_become_a_conflict_or_absence() {
    for suffix in [pointer(4095), vec![0xc0], vec![0x40, 0], vec![0x80, 0]] {
        let mut rr = local(33);
        rr.data.truncate(6);
        rr.data.extend(suffix);
        assert_invalid(&packet(0x8400, &[rr], &[], &[]));
    }
    let mut rr = local(33);
    rr.data.truncate(6);
    let at = 12 + rr.owner.len() + 10 + 6;
    rr.data.extend(pointer(at));
    assert_invalid(&packet(0x8400, &[rr], &[], &[]));
}

#[test]
fn compressed_rdata_must_end_at_its_exact_declared_boundary() {
    let mut bytes = question_prefix(HOST);
    bytes[7] = 1;
    let mut rr = local(33);
    rr.data.truncate(6);
    rr.data.extend(pointer(12));
    rr.data.push(0);
    append(&mut bytes, &rr);
    assert_invalid(&bytes);
}

#[test]
fn ptr_compression_is_validated_even_when_it_is_not_a_compared_tuple() {
    let mut bytes = question_prefix(INSTANCE);
    bytes[7] = 1;
    append(&mut bytes, &record(SERVICE, 12, pointer(12)));
    assert_eq!(observe(&bytes), Ok(Observation::NoConflictingRecord));
    *bytes.last_mut().unwrap() = 0xff;
    assert_invalid(&bytes);
}

#[test]
fn dots_inside_one_owner_label_cannot_alias_a_multilabel_owner() {
    let mut rr = local(1);
    rr.owner = vec![14];
    rr.owner.extend(b"host-one.local");
    rr.owner.push(0);
    rr.data[3] = 8;
    assert_eq!(
        observe(&packet(0x8400, &[rr], &[], &[])),
        Ok(Observation::NoConflictingRecord)
    );
}

#[test]
fn dots_inside_one_srv_target_label_are_different_from_separate_labels() {
    let mut rr = local(33);
    rr.data.truncate(6);
    rr.data.push(14);
    rr.data.extend(b"host-one.local");
    rr.data.push(0);
    assert_eq!(
        observe(&packet(0x8400, &[rr], &[], &[])),
        Ok(Observation::ConflictingRecord)
    );
}

#[test]
fn later_bad_compressed_owner_cannot_be_hidden_by_an_early_conflict() {
    let mut bytes = packet(0x8400, &[record(HOST, 1, vec![192, 168, 1, 8])], &[], &[]);
    bytes[11] = 1;
    let mut rr = local(1);
    rr.owner = pointer(bytes.len());
    append(&mut bytes, &rr);
    assert_invalid(&bytes);
}

#[test]
fn the_existing_ascii_name_profile_rejects_controls_and_non_ascii() {
    for label in [0, 0x1f, 0x7f, 0x80, 0xff] {
        let mut rr = local(1);
        rr.owner = vec![1, label, 0];
        assert_invalid(&packet(0x8400, &[rr], &[], &[]));
        let mut rr = local(33);
        rr.data.truncate(6);
        rr.data.extend([1, label, 0]);
        assert_invalid(&packet(0x8400, &[rr], &[], &[]));
    }
}

#[test]
fn txt_bytes_that_look_like_pointers_remain_opaque_string_data() {
    let rr = record(INSTANCE, 16, vec![2, 0xc0, 0x0c]);
    assert_eq!(
        observe(&packet(0x8400, &[rr], &[], &[])),
        Ok(Observation::ConflictingRecord)
    );
}
