use super::{fixture::*, *};

#[test]
fn account_and_file_references_roundtrip_only_exact_pins() {
    let subject = account(1);
    assert_eq!(subject.principal(), account(1).principal());
    assert_eq!(subject.account_id(), account(1).account_id());
    let encoded = serde_json::to_value(subject).unwrap();
    assert_eq!(
        encoded,
        json!({"principal":account(1).principal(),"account_id":account(1).account_id()})
    );
    let decoded: FileAccountRef = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, account(1));
    for length in [0, 11, FILE_TRANSFER_CHUNK_BYTES + 17] {
        let m = manifest(length);
        let reference = FileVersionRef::from_manifest(&m);
        assert_eq!(reference.workspace(), m.workspace());
        assert_eq!(reference.file_id(), m.file_id());
        assert_eq!(reference.version_id(), m.version_id());
        assert_eq!(reference.byte_length(), length);
        assert_eq!(reference.sha256(), m.sha256());
        let encoded = serde_json::to_value(&reference).unwrap();
        assert_eq!(
            encoded,
            json!({"workspace":scope(),"file_id":m.file_id(),"version_id":m.version_id(),
            "byte_length":length,"sha256":m.sha256()})
        );
        let decoded: FileVersionRef = serde_json::from_value(encoded.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), encoded);
        for forbidden in [
            "display_name",
            "chunk_size",
            "chunk_sha256",
            "path",
            "url",
            "owner",
        ] {
            assert!(encoded.get(forbidden).is_none());
        }
    }
}

#[test]
fn version_reference_deserialization_rejects_invalid_geometry_and_noncanonical_hashes() {
    let raw = serde_json::to_value(file()).unwrap();
    let mut max = raw.clone();
    max["byte_length"] = json!(MAX_FILE_TRANSFER_BYTES);
    assert_eq!(
        serde_json::from_value::<FileVersionRef>(max)
            .unwrap()
            .byte_length(),
        MAX_FILE_TRANSFER_BYTES
    );
    for bad in [
        json!(MAX_FILE_TRANSFER_BYTES + 1),
        json!(u64::MAX),
        json!(-1),
        json!(1.0),
        json!("11"),
        Value::Null,
    ] {
        let mut changed = raw.clone();
        changed["byte_length"] = bad;
        assert!(serde_json::from_value::<FileVersionRef>(changed).is_err());
    }
    for bad in [
        "A".repeat(64),
        "a".repeat(63),
        "a".repeat(65),
        "g".repeat(64),
        format!("sha256:{}", "a".repeat(64)),
    ] {
        let mut changed = raw.clone();
        changed["sha256"] = json!(bad);
        assert!(serde_json::from_value::<FileVersionRef>(changed).is_err());
    }
    let mut empty = raw.clone();
    empty["byte_length"] = json!(0);
    assert!(
        serde_json::from_value::<FileVersionRef>(empty.clone()).is_err(),
        "zero length requires the exact empty digest"
    );
    empty["sha256"] = json!(EMPTY_SHA);
    assert_eq!(
        serde_json::from_value::<FileVersionRef>(empty)
            .unwrap()
            .byte_length(),
        0
    );
}

#[test]
fn validity_is_bounded_nonempty_half_open_epoch_seconds() {
    assert_eq!(MAX_FILE_GRANT_UNIX_SECONDS, 253_402_300_799);
    for (start, end) in [
        (0, 1),
        (100, 200),
        (MAX_FILE_GRANT_UNIX_SECONDS - 1, MAX_FILE_GRANT_UNIX_SECONDS),
    ] {
        let v = FileGrantValidity::new(start, end).unwrap();
        assert_eq!(v.not_before(), start);
        assert_eq!(v.expires_at(), end);
        assert!(v.contains(start));
        assert!(v.contains(end - 1));
        assert!(!v.contains(end));
        if start > 0 {
            assert!(!v.contains(start - 1));
        }
        assert!(!v.contains(u64::MAX));
        let encoded = serde_json::to_value(v).unwrap();
        assert_eq!(encoded, json!({"not_before":start,"expires_at":end}));
        let decoded: FileGrantValidity = serde_json::from_value(encoded).unwrap();
        assert!(decoded.contains(start));
    }
    for (start, end) in [
        (0, 0),
        (1, 1),
        (200, 100),
        (0, MAX_FILE_GRANT_UNIX_SECONDS + 1),
        (MAX_FILE_GRANT_UNIX_SECONDS, MAX_FILE_GRANT_UNIX_SECONDS + 1),
        (u64::MAX - 1, u64::MAX),
        (0, u64::MAX),
    ] {
        assert!(
            FileGrantValidity::new(start, end).is_err(),
            "accepted [{start},{end})"
        );
        assert!(serde_json::from_value::<FileGrantValidity>(
            json!({"not_before":start,"expires_at":end})
        )
        .is_err());
    }
}

#[test]
fn grant_ids_are_typed_nonnil_canonical_lowercase_hyphenated_uuids() {
    assert!(FileGrantId::try_from(Uuid::nil()).is_err());
    let canonical = "8b075b0b-8118-4c37-9224-337ba22f40c6";
    let id: FileGrantId = canonical.parse().unwrap();
    assert_eq!(id.to_string(), canonical);
    assert_eq!(serde_json::to_value(id).unwrap(), json!(canonical));
    assert!(serde_json::from_value::<FileGrantId>(json!(canonical)).is_ok());
    for bad in [
        canonical.to_uppercase(),
        canonical.replace('-', ""),
        format!("urn:uuid:{canonical}"),
        format!("{{{canonical}}}"),
        "00000000-0000-0000-0000-000000000000".into(),
        "not-uuid".into(),
        format!(" {canonical}"),
        format!("{canonical} "),
    ] {
        assert!(bad.parse::<FileGrantId>().is_err(), "accepted {bad}");
        assert!(serde_json::from_value::<FileGrantId>(json!(bad)).is_err());
    }
    for bad in [Value::Null, json!(1), json!(true), json!([]), json!({})] {
        assert!(serde_json::from_value::<FileGrantId>(bad).is_err());
    }
}
