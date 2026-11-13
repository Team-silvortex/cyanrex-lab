use super::*;

#[test]
fn invalid_headers_question_counts_and_record_totals_are_rejected() {
    for (offset, value) in [
        (0, 1_u16),
        (2, 0),
        (2, 0x8800),
        (2, 0x8200),
        (2, 0x8001),
        (4, 9),
        (6, 65),
    ] {
        let mut bytes = packet(&records());
        bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        assert_invalid(&bytes);
    }
    let mut bytes = packet(&records());
    bytes[6..8].copy_from_slice(&32_u16.to_be_bytes());
    bytes[8..10].copy_from_slice(&33_u16.to_be_bytes());
    assert_invalid(&bytes);
}

#[test]
fn truncated_packets_and_trailing_bytes_cannot_return_partial_events() {
    let bytes = packet(&records());
    for length in 0..bytes.len() {
        assert!(parse_response(&bytes[..length]).is_err(), "prefix {length}");
    }
    let mut garbage = bytes;
    garbage.extend([0, 0]);
    assert_invalid(&garbage);
}

#[test]
fn compression_cycles_forward_pointers_bad_offsets_and_reserved_label_tags_fail() {
    for owner in [
        vec![0xc0, 12],
        vec![0xc0, 14],
        vec![1, b'a', 0xc0, 12],
        vec![0xff, 0xff],
        vec![0x40, 0],
        vec![0x80, 0],
        vec![0xc0],
    ] {
        let mut bytes = vec![0, 0, 0x84, 0, 0, 0, 0, 1, 0, 0, 0, 0];
        append_rr(&mut bytes, &owner, 65000, 1, 30, &[]);
        assert_invalid(&bytes);
    }
}

#[test]
fn expanded_names_above_dns_length_limit_or_non_ascii_names_fail_even_if_unknown() {
    let valid_name = format!(
        "{}.{}.{}.{}.",
        "a".repeat(63),
        "b".repeat(63),
        "c".repeat(63),
        "d".repeat(61)
    );
    let rr = Record {
        owner: valid_name.clone(),
        kind: 65000,
        class: 1,
        ttl: 30,
        data: vec![],
    };
    assert!(parse_response(&packet(&[rr])).unwrap().is_empty());
    let rr = Record {
        owner: format!("{valid_name}x."),
        kind: 65000,
        class: 1,
        ttl: 30,
        data: vec![],
    };
    assert_invalid(&packet(&[rr]));
    let rr = Record {
        owner: "노드.local.".into(),
        kind: 65000,
        class: 1,
        ttl: 30,
        data: vec![],
    };
    assert_invalid(&packet(&[rr]));
}

#[test]
fn relevant_rdata_has_exact_lengths_and_name_consumption() {
    for index in [0, 1] {
        let mut rr = records();
        rr[index].data.push(0);
        assert_invalid(&packet(&rr));
    }
    for data in [vec![], vec![192, 168, 1], vec![192, 168, 1, 7, 8]] {
        let mut rr = records();
        rr[3].data = data;
        assert_invalid(&packet(&rr));
    }
    let mut rr = records();
    rr[1].data = vec![0; 6];
    assert_invalid(&packet(&rr));
}

#[test]
fn compressed_ptr_and_srv_rdata_lengths_cannot_consume_the_next_record() {
    let (original, offsets) = compressed_offsets();
    assert_eq!(parse_response(&original).unwrap().len(), 1);
    for offset in offsets {
        let length = u16::from_be_bytes([original[offset], original[offset + 1]]);
        for changed in [length - 1, length + 1] {
            let mut bytes = original.clone();
            bytes[offset..offset + 2].copy_from_slice(&changed.to_be_bytes());
            assert_invalid(&bytes);
        }
    }
}

#[test]
fn ptr_and_srv_names_must_stay_in_the_exact_profile_without_alias_following() {
    for target in [
        "node-one._cyanrex._tcp.local.",
        "node-one._cyanrex._udp.example.",
        "node.one._cyanrex._udp.local.",
        "-node._cyanrex._udp.local.",
    ] {
        let mut rr = records();
        rr[0].data = dns_name(target);
        assert_invalid(&packet(&rr));
    }
    for target in [
        "host.example.",
        "local.",
        "-host.local.",
        "host-.local.",
        "host_name.local.",
    ] {
        let mut rr = records();
        rr[1].data = srv(target, 4433);
        assert_invalid(&packet(&rr));
    }
}

#[test]
fn port_zero_and_nonzero_priority_or_weight_are_rejected_for_profile_srv() {
    for data in [
        srv(HOST, 0),
        {
            let mut data = srv(HOST, 4433);
            data[1] = 1;
            data
        },
        {
            let mut data = srv(HOST, 4433);
            data[3] = 1;
            data
        },
    ] {
        let mut rr = records();
        rr[1].data = data;
        assert_invalid(&packet(&rr));
    }
}

#[test]
fn only_the_exact_srv_target_supplies_a_records() {
    let mut rr = records();
    rr[3].owner = "other.local.".into();
    assert!(parse_response(&packet(&rr)).unwrap().is_empty());
    rr.push(Record {
        owner: HOST.into(),
        kind: 28,
        class: 1,
        ttl: 30,
        data: vec![0; 16],
    });
    assert!(parse_response(&packet(&rr)).unwrap().is_empty());
}

#[test]
fn duplicate_ptr_srv_txt_or_a_records_are_ambiguous_even_if_bytes_match() {
    for index in 0..4 {
        let mut rr = records();
        rr.push(rr[index].clone());
        assert_invalid(&packet(&rr));
    }
    let mut rr = records();
    let mut goodbye = rr[0].clone();
    goodbye.ttl = 0;
    rr.push(goodbye);
    assert_invalid(&packet(&rr));
}

#[test]
fn more_than_eight_target_addresses_are_not_silently_truncated() {
    let mut rr = records();
    for index in 8..=15 {
        let mut a = rr[3].clone();
        a.data[3] = index;
        rr.push(a);
    }
    assert_invalid(&packet(&rr));
}

#[test]
fn malformed_txt_keeps_static_nested_classification_without_secret_values() {
    let mut rr = records();
    rr[2].data = b"private-cookie-private-password".to_vec();
    let error = match parse_response(&packet(&rr)) {
        Err(error) => error,
        Ok(_) => panic!("bad TXT must not resolve"),
    };
    assert!(matches!(error, MdnsError::RejectedTxt(_)));
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains("private-cookie"));
    assert!(!rendered.contains("private-password"));
}

#[test]
fn zero_ttl_srv_without_the_complete_necessary_chain_does_not_remove_arbitrary_names() {
    let mut rr = records();
    rr[1].ttl = 0;
    rr.remove(2);
    assert!(parse_response(&packet(&rr)).unwrap().is_empty());
}
