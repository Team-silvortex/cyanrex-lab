use super::*;

fn pointer(offset: usize) -> Vec<u8> {
    assert!(offset < 0x4000);
    (0xc000 | offset as u16).to_be_bytes().to_vec()
}

#[test]
fn owner_case_and_compressed_question_names_match_frozen_labels() {
    let q = question("NoDe-OnE._CyAnReX._UdP.LoCaL.", 33, 1);
    let mut bytes = packet(0, &[q], &[], &[], &[]);
    bytes[5] = 2;
    bytes.extend(pointer(12));
    bytes.extend([0, 16, 0, 1]);
    assert_eq!(
        plan(&bytes, 1000),
        Ok(selected(
            Set {
                srv: true,
                txt: true,
                ..Default::default()
            },
            Set::default()
        ))
    );
}

#[test]
fn compressed_ptr_target_and_owner_can_suppress_the_same_frozen_record() {
    let qs = [
        question(SERVICE, 12, 1),
        question("NoDe-OnE._CyAnReX._UdP.LoCaL.", 16, 1),
    ];
    let target_at = 12 + name(SERVICE).len() + 4;
    let mut bytes = packet(0, &qs, &[], &[], &[]);
    bytes[7] = 1;
    let mut rr = local(12, 50);
    rr.owner = pointer(12);
    rr.data = pointer(target_at);
    append(&mut bytes, &rr);
    assert_eq!(
        plan(&bytes, 1000),
        Ok(selected(
            Set {
                ptr: true,
                txt: true,
                ..Default::default()
            },
            set(12)
        ))
    );
}

#[test]
fn compressed_srv_target_is_compared_against_full_packet_offsets() {
    let qs = [question(INSTANCE, 33, 1), question("HoSt-OnE.LoCaL.", 1, 1)];
    let target_at = 12 + name(INSTANCE).len() + 4;
    let mut bytes = packet(0, &qs, &[], &[], &[]);
    bytes[7] = 1;
    let mut rr = local(33, 50);
    rr.owner = pointer(12);
    rr.data.truncate(6);
    rr.data.extend(pointer(target_at));
    append(&mut bytes, &rr);
    assert_eq!(
        plan(&bytes, 1000),
        Ok(selected(
            Set {
                srv: true,
                a: true,
                ..Default::default()
            },
            set(33)
        ))
    );
}

#[test]
fn a_single_label_with_dots_does_not_alias_a_multilabel_question() {
    let mut q = question(HOST, 1, 1);
    q.owner = vec![14];
    q.owner.extend(b"host-one.local");
    q.owner.push(0);
    assert_eq!(
        plan(&packet(0, &[q], &[], &[], &[]), 1000),
        Ok(selected(Set::default(), Set::default()))
    );
}

#[test]
fn malformed_question_compression_never_becomes_an_empty_selection() {
    for owner in [
        pointer(12),
        pointer(4095),
        vec![0xc0],
        vec![0x40, 0],
        vec![0x80, 0],
    ] {
        let mut q = question(HOST, 1, 1);
        q.owner = owner;
        assert_eq!(
            plan(&packet(0, &[q], &[], &[], &[]), 1000),
            Err(MdnsError::InvalidPacket)
        );
    }
}

#[test]
fn malformed_late_ptr_target_is_not_hidden_by_a_matching_known_answer() {
    let bad = record("other.local.", 12, pointer(4095), 1);
    assert_eq!(
        plan(
            &packet(
                0,
                &[question(SERVICE, 12, 1)],
                &[local(12, 50)],
                &[],
                &[bad]
            ),
            1000
        ),
        Err(MdnsError::InvalidPacket)
    );
}

#[test]
fn compressed_target_requires_its_exact_record_end() {
    let qs = [question(HOST, 1, 1), question(INSTANCE, 33, 1)];
    let mut bytes = packet(0, &qs, &[], &[], &[]);
    bytes[7] = 1;
    let mut rr = local(33, 50);
    rr.data.truncate(6);
    rr.data.extend(pointer(12));
    rr.data.push(0);
    append(&mut bytes, &rr);
    assert_eq!(plan(&bytes, 1000), Err(MdnsError::InvalidPacket));
}

#[test]
fn ascii_profile_rejects_control_and_non_ascii_names_in_questions_or_rdata() {
    for value in [0, 0x1f, 0x7f, 0x80, 0xff] {
        let mut q = question(HOST, 1, 1);
        q.owner = vec![1, value, 0];
        assert_eq!(
            plan(&packet(0, &[q], &[], &[], &[]), 1000),
            Err(MdnsError::InvalidPacket)
        );
        let rr = record(SERVICE, 12, vec![1, value, 0], 50);
        assert_eq!(
            plan(&packet(0, &questions_all(), &[rr], &[], &[]), 1000),
            Err(MdnsError::InvalidPacket)
        );
    }
}
