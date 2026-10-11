use super::{fixture::*, json_support::*, *};

#[test]
fn state_is_only_a_canonical_string_never_an_external_tagged_object() {
    for (state, text) in [
        (FileGrantState::Active, "active"),
        (FileGrantState::Revoked, "revoked"),
    ] {
        let scalar = serde_json::to_vec(&json!(text)).unwrap();
        assert_eq!(
            serde_json::from_slice::<FileGrantState>(&scalar).unwrap(),
            state
        );
        let mut p = raw_publisher();
        p["state"] = json!(text);
        assert_eq!(
            FilePublisherGrant::parse_json(&serde_json::to_vec(&p).unwrap())
                .unwrap()
                .state(),
            state
        );
        let mut r = raw_recipient();
        r["state"] = json!(text);
        assert_eq!(
            FileRecipientGrant::parse_json(&serde_json::to_vec(&r).unwrap())
                .unwrap()
                .state(),
            state
        );
    }
    for tagged in [json!({"active":null}), json!({"revoked":null})] {
        let bytes = serde_json::to_vec(&tagged).unwrap();
        assert!(serde_json::from_slice::<FileGrantState>(&bytes).is_err(),
            "state protocol is a scalar string, not the default Serde externally tagged unit-enum alias");
        assert!(serde_json::from_value::<FileGrantState>(tagged.clone()).is_err());
        let mut p = raw_publisher();
        p["state"] = tagged.clone();
        reject_publisher(&p);
        let mut r = raw_recipient();
        r["state"] = tagged;
        reject_recipient(&r);
    }
}

#[test]
fn schema_revision_time_and_file_size_require_exact_bounded_json_integers() {
    for kind in [Kind::Publisher, Kind::Recipient] {
        let raw = kind.raw();
        let mut integers = vec![
            "/schema_version",
            "/revision",
            "/validity/not_before",
            "/validity/expires_at",
        ];
        if matches!(kind, Kind::Recipient) {
            integers.push("/file/byte_length");
        }
        for path in integers {
            for token in [
                "-1",
                "1.0",
                "1e0",
                "\"1\"",
                "true",
                "null",
                "18446744073709551616",
                "+1",
                "01",
            ] {
                let changed = substitute(&raw, path, token);
                kind.reject_bytes(changed.as_bytes());
            }
        }
        for (path, bad) in [
            ("/schema_version", json!(0)),
            ("/schema_version", json!(2)),
            ("/revision", json!(0)),
            ("/revision", json!(9_007_199_254_740_992_u64)),
            (
                "/validity/not_before",
                json!(MAX_FILE_GRANT_UNIX_SECONDS + 1),
            ),
            (
                "/validity/expires_at",
                json!(MAX_FILE_GRANT_UNIX_SECONDS + 1),
            ),
            ("/validity/expires_at", json!(100)),
            ("/validity/not_before", json!(200)),
        ] {
            let mut changed = raw.clone();
            *changed.pointer_mut(path).unwrap() = bad;
            kind.reject(&changed);
        }
        let mut max_revision = raw.clone();
        max_revision["revision"] = json!(9_007_199_254_740_991_u64);
        match kind {
            Kind::Publisher => {
                assert!(FilePublisherGrant::parse_json(
                    &serde_json::to_vec(&max_revision).unwrap()
                )
                .is_ok());
            }
            Kind::Recipient => {
                assert!(FileRecipientGrant::parse_json(
                    &serde_json::to_vec(&max_revision).unwrap()
                )
                .is_ok());
            }
        }
    }
    let mut oversized = raw_recipient();
    oversized["file"]["byte_length"] = json!(MAX_FILE_TRANSFER_BYTES + 1);
    reject_recipient(&oversized);
    let mut wrong_empty = raw_recipient();
    wrong_empty["file"]["byte_length"] = json!(0);
    reject_recipient(&wrong_empty);
    wrong_empty["file"]["sha256"] = json!(EMPTY_SHA);
    assert!(FileRecipientGrant::parse_json(&serde_json::to_vec(&wrong_empty).unwrap()).is_ok());
}

#[test]
fn all_nested_ids_hashes_and_states_remain_canonical() {
    for kind in [Kind::Publisher, Kind::Recipient] {
        let raw = kind.raw();
        let id_paths: Vec<String> = paths(&raw)
            .into_iter()
            .flat_map(|path| {
                at(&raw, &path)
                    .as_object()
                    .unwrap()
                    .keys()
                    .filter(|key| *key == "id" || key.ends_with("_id"))
                    .map(|key| format!("{path}/{key}"))
                    .collect::<Vec<_>>()
            })
            .collect();
        for path in id_paths {
            let original = raw.pointer(&path).unwrap().as_str().unwrap();
            for bad in [
                original.to_uppercase(),
                original.replace('-', ""),
                format!("urn:uuid:{original}"),
                "00000000-0000-0000-0000-000000000000".into(),
                "not-uuid".into(),
            ] {
                assert_ne!(
                    bad, original,
                    "fixture IDs must contain lowercase letters for the uppercase probe"
                );
                let mut changed = raw.clone();
                *changed.pointer_mut(&path).unwrap() = json!(bad);
                kind.reject(&changed);
            }
        }
        for bad in [
            json!("Active"),
            json!("Revoked"),
            json!("published"),
            json!(true),
            json!(1),
            json!(["active"]),
        ] {
            let mut changed = raw.clone();
            changed["state"] = bad;
            kind.reject(&changed);
        }
    }
    for sha in [
        "A".repeat(64),
        "g".repeat(64),
        "a".repeat(63),
        format!(" {}", "a".repeat(64)),
    ] {
        let mut changed = raw_recipient();
        changed["file"]["sha256"] = json!(sha);
        reject_recipient(&changed);
    }
}

#[test]
fn bounded_parse_admission_accepts_exact_limit_and_rejects_oversize_or_extra_documents() {
    assert_eq!(MAX_FILE_GRANT_JSON_BYTES, 4096);
    for kind in [Kind::Publisher, Kind::Recipient] {
        let raw = kind.raw();
        let encoded = serde_json::to_vec(&raw).unwrap();
        assert!(encoded.len() < MAX_FILE_GRANT_JSON_BYTES);
        let mut boundary = encoded.clone();
        boundary.resize(MAX_FILE_GRANT_JSON_BYTES, b' ');
        let mut oversized = boundary.clone();
        oversized.push(b' ');
        match kind {
            Kind::Publisher => {
                assert!(FilePublisherGrant::parse_json(&boundary).is_ok());
                assert!(FilePublisherGrant::parse_json(&oversized).is_err());
                assert!(
                    serde_json::from_slice::<FilePublisherGrant>(&oversized).is_ok(),
                    "generic serde callers own their input byte admission"
                );
            }
            Kind::Recipient => {
                assert!(FileRecipientGrant::parse_json(&boundary).is_ok());
                assert!(FileRecipientGrant::parse_json(&oversized).is_err());
                assert!(
                    serde_json::from_slice::<FileRecipientGrant>(&oversized).is_ok(),
                    "parse limit is not a universal serde allocation ceiling"
                );
            }
        }
        let mut extra = encoded.clone();
        extra.extend_from_slice(b" {}");
        kind.reject_bytes(&extra);
        let mut garbage = encoded.clone();
        garbage.push(0);
        kind.reject_bytes(&garbage);
        let mut truncated = encoded.clone();
        truncated.pop();
        kind.reject_bytes(&truncated);
        for invalid in [b"".as_slice(), b"null", b"{}", b"[]", b"true", b"\xff"] {
            kind.reject_bytes(invalid);
        }
    }
}
