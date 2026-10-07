use super::*;

#[test]
fn checkpoint_records_are_map_only_and_reject_unknown_fields() {
    let value = wire(1);
    let records = [
        "",
        "/task",
        "/task/workspace",
        "/manifest",
        "/manifest/payload/0",
        "/manifest/payload/0/artifact",
        "/manifest/payload/0/artifact/workspace",
    ];
    for pointer in records {
        let record = value.pointer(pointer).unwrap();
        // Serde structs accept positional arrays in declaration order unless MapOnly intervenes.
        // Object map key order would often fail on field types even without that protection.
        let fields: &[&str] = match pointer {
            "" => &[
                "format",
                "version",
                "task",
                "manifest",
                "byte_lengths",
                "reported_state",
                "reported_confirmed_artifact_count",
            ],
            "/task" => &["workspace", "task_id"],
            "/manifest" => &["schema_version", "title", "payload"],
            "/manifest/payload/0" => &["kind", "filename", "language", "artifact"],
            "/manifest/payload/0/artifact" => {
                &["workspace", "artifact_id", "revision_id", "sha256"]
            }
            _ => &["authority_id", "workspace_id"],
        };
        let sequence = Value::Array(fields.iter().map(|key| record[*key].clone()).collect());
        for shape in [sequence, json!(null), json!("record")] {
            let mut changed = value.clone();
            *changed.pointer_mut(pointer).unwrap() = shape;
            assert!(
                parse(&changed).is_err(),
                "named record required at {pointer}"
            );
        }
        for extra in [
            "token",
            "fingerprint",
            "text",
            "source",
            "namespace",
            "path",
            "owner",
            "reported_unconfirmed_step",
            "confirmed_task",
        ] {
            let mut changed = value.clone();
            changed
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(extra.into(), json!("excluded-private-sentinel"));
            let result = parse(&changed);
            assert!(result.is_err(), "unknown field at {pointer}/{extra}");
            assert!(!result
                .err()
                .unwrap()
                .to_string()
                .contains("excluded-private-sentinel"));
        }
        for key in record.as_object().unwrap().keys() {
            let mut changed = value.clone();
            changed
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(parse(&changed).is_err(), "required field {pointer}/{key}");
        }
    }
}

#[test]
fn checkpoint_duplicate_fields_are_rejected_at_every_record_level() {
    let value = wire(1);
    for pointer in [
        "",
        "/task",
        "/task/workspace",
        "/manifest",
        "/manifest/payload/0",
        "/manifest/payload/0/artifact",
        "/manifest/payload/0/artifact/workspace",
    ] {
        let record = value.pointer(pointer).unwrap().as_object().unwrap();
        for (key, original) in record {
            for replacement in [original, &Value::Null] {
                let raw = duplicate_at(&value, pointer, key, replacement);
                assert!(
                    SessionDraftPublicationCheckpoint::parse_json(&raw).is_err(),
                    "duplicate field {pointer}/{key} must remain visible to strict deserialization"
                );
            }
        }
    }
}

#[test]
fn checkpoint_format_version_and_state_are_exact() {
    let value = wire(0);
    for format in ["", "cyanrex.task-draft", "cyanrex.task-draft-checkpoint "] {
        let mut changed = value.clone();
        changed["format"] = json!(format);
        assert!(parse(&changed).is_err());
    }
    for pointer in ["/version", "/manifest/schema_version"] {
        for raw in ["0", "2", "1.0", "1e0", "-1", "256", "\"1\"", "null", "true"] {
            assert!(
                SessionDraftPublicationCheckpoint::parse_json(&raw_at(&value, pointer, raw))
                    .is_err()
            );
        }
    }
    for raw in [
        "\"Ready\"",
        "\"ready \"",
        "\"unknown\"",
        "1",
        "null",
        "{\"ready\":null}",
        "[\"ready\"]",
    ] {
        assert!(SessionDraftPublicationCheckpoint::parse_json(&raw_at(
            &value,
            "/reported_state",
            raw
        ))
        .is_err());
    }
    let mut changed = wire(1);
    changed["manifest"]["payload"][0]["kind"] = json!({"text": null});
    assert!(parse(&changed).is_err());
}

#[test]
fn checkpoint_numeric_fields_are_lexical_bounded_and_parallel() {
    let value = wire(1);
    for pointer in ["/byte_lengths/0", "/reported_confirmed_artifact_count"] {
        for raw in [
            "-1",
            "0.0",
            "1e0",
            "\"0\"",
            "null",
            "false",
            "18446744073709551616",
        ] {
            assert!(
                SessionDraftPublicationCheckpoint::parse_json(&raw_at(&value, pointer, raw))
                    .is_err()
            );
        }
    }
    for lengths in [
        json!([]),
        json!([0, 0]),
        json!([MAX_TEXT_PAYLOAD_BYTES + 1]),
        json!(null),
    ] {
        let mut changed = value.clone();
        changed["byte_lengths"] = lengths;
        assert!(parse(&changed).is_err());
    }
    for length in [0, MAX_TEXT_PAYLOAD_BYTES] {
        let mut changed = value.clone();
        changed["byte_lengths"] = json!([length]);
        assert_eq!(parse(&changed).unwrap().byte_lengths(), &[length as u64]);
    }
    assert!(parse(&wire(32)).is_ok());
    assert!(parse(&wire(33)).is_err());
}

#[test]
fn checkpoint_requires_uuid4_targets_and_canonical_non_nil_scope_ids() {
    let value = wire(1);
    for pointer in [
        "/task/task_id",
        "/manifest/payload/0/artifact/artifact_id",
        "/manifest/payload/0/artifact/revision_id",
    ] {
        for bad in [
            "00000000-0000-0000-0000-000000000000",
            "00000000-0000-5000-8000-000000000001",
            "00000000-0000-4000-0000-000000000001", // Reserved variant, although version nibble is 4.
            "00000000-0000-4000-c000-000000000001", // Microsoft variant, not RFC UUIDv4.
            "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
            "aaaaaaaaaaaa4aaa8aaaaaaaaaaaaaaa",
        ] {
            let mut changed = value.clone();
            *changed.pointer_mut(pointer).unwrap() = json!(bad);
            assert!(parse(&changed).is_err(), "invalid allocated ID {bad}");
        }
    }
    // Scope identity contracts deliberately accept non-v4 canonical non-nil UUIDs.
    assert!(parse(&value).is_ok());
    for field in ["authority_id", "workspace_id"] {
        for bad in [
            "00000000-0000-0000-0000-000000000000",
            "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
            "not-an-id",
        ] {
            let mut changed = value.clone();
            changed["task"]["workspace"][field] = json!(bad);
            changed["manifest"]["payload"][0]["artifact"]["workspace"][field] = json!(bad);
            assert!(parse(&changed).is_err());
        }
    }
    for digest in [
        "a".repeat(63),
        "a".repeat(65),
        "A".repeat(64),
        "g".repeat(64),
    ] {
        let mut changed = value.clone();
        changed["manifest"]["payload"][0]["artifact"]["sha256"] = json!(digest);
        assert!(parse(&changed).is_err());
    }
}

#[test]
fn checkpoint_rejects_scope_mismatch_and_reused_allocated_identities() {
    let value = wire(2);
    for field in ["authority_id", "workspace_id"] {
        let mut changed = value.clone();
        changed["manifest"]["payload"][0]["artifact"]["workspace"][field] =
            json!(Uuid::new_v4().to_string());
        assert!(parse(&changed).is_err());
        let mut changed = value.clone();
        changed["task"]["workspace"][field] = json!(Uuid::new_v4().to_string());
        assert!(parse(&changed).is_err());
    }
    for field in ["artifact_id", "revision_id"] {
        let mut changed = value.clone();
        changed["manifest"]["payload"][1]["artifact"][field] =
            changed["manifest"]["payload"][0]["artifact"][field].clone();
        assert!(
            parse(&changed).is_err(),
            "allocation identity {field} must be unique independently"
        );
    }
    let mut changed = value.clone();
    changed["manifest"]["payload"][1]["artifact"] =
        changed["manifest"]["payload"][0]["artifact"].clone();
    changed["manifest"]["payload"][1]["artifact"]["sha256"] = json!("b".repeat(64));
    assert!(parse(&changed).is_err());
    let parsed = parse(&value).unwrap();
    let refs = parsed.manifest().input_refs();
    assert_eq!(refs[0].sha256, refs[1].sha256);
    assert_ne!(refs[0].artifact_id, refs[1].artifact_id);
}

#[test]
fn checkpoint_metadata_limits_preserve_inert_labels_and_language_hints() {
    let value = wire(1);
    for (title, filename, language) in [
        ("x".repeat(256), "中".repeat(128), "x".repeat(64)),
        (
            TITLE.into(),
            "😀".repeat(64),
            "future.language+v2-test_3".into(),
        ),
        ("Task".into(), "CON".into(), "c++".into()),
        ("Task".into(), "  \u{202e}  ".into(), "plaintext".into()),
    ] {
        let mut changed = value.clone();
        changed["manifest"]["title"] = json!(title);
        changed["manifest"]["payload"][0]["filename"] = json!(filename);
        changed["manifest"]["payload"][0]["language"] = json!(language);
        let parsed = parse(&changed).unwrap();
        assert_eq!(parsed.manifest().title(), title);
        assert_eq!(parsed.manifest().payload()[0].filename(), filename);
        assert_eq!(parsed.manifest().payload()[0].language(), language);
    }
    for (pointer, invalid) in [
        (
            "/manifest/title",
            vec![
                "".into(),
                " \u{2003} ".into(),
                "x".repeat(257),
                "line\n".into(),
            ],
        ),
        (
            "/manifest/payload/0/filename",
            vec![
                "".into(),
                "..".into(),
                "a/b".into(),
                "a\\b".into(),
                "😀".repeat(65),
            ],
        ),
        (
            "/manifest/payload/0/language",
            vec![
                "".into(),
                "Rust".into(),
                "a".repeat(65),
                "1x".into(),
                "sh;id".into(),
            ],
        ),
    ] {
        for bad in invalid {
            let mut changed = value.clone();
            *changed.pointer_mut(pointer).unwrap() = json!(bad);
            assert!(parse(&changed).is_err());
        }
    }
}

#[test]
fn checkpoint_raw_byte_limit_and_json_framing_are_strict() {
    assert_eq!(MAX_SESSION_DRAFT_CHECKPOINT_BYTES, 64 * 1024);
    let bytes = serde_json::to_vec(&wire(1)).unwrap();
    let mut padded = bytes.clone();
    padded.resize(MAX_SESSION_DRAFT_CHECKPOINT_BYTES, b' ');
    assert!(SessionDraftPublicationCheckpoint::parse_json(&padded).is_ok());
    padded.push(b' ');
    assert!(SessionDraftPublicationCheckpoint::parse_json(&padded).is_err());
    for raw in [
        [b"\xef\xbb\xbf".as_slice(), &bytes].concat(),
        [bytes.as_slice(), b" {}"].concat(),
        [bytes.as_slice(), &[0xff]].concat(),
        raw_at(&wire(0), "/manifest/title", r#""\uD800""#),
        format!("{}0{}", "[".repeat(150), "]".repeat(150)).into_bytes(),
        b"".to_vec(),
    ] {
        assert!(SessionDraftPublicationCheckpoint::parse_json(&raw).is_err());
    }
}
