use super::*;

#[test]
fn import_rejects_non_objects_unknown_fields_and_duplicate_fields_at_both_levels() {
    for bytes in [
        "null",
        "[]",
        "true",
        "1",
        "\"draft\"",
        r#"{"format":"cyanrex.task-draft","version":1,"title":"T","payload":[],"owner":"x"}"#,
        r#"{"format":"cyanrex.task-draft","version":1,"title":"T","title":"T","payload":[]}"#,
        r#"{"format":"cyanrex.task-draft","version":1,"title":"T","payload":[[]]}"#,
        r#"{"format":"cyanrex.task-draft","version":1,"title":"T","payload":[null]}"#,
        r#"{"format":"cyanrex.task-draft","version":1,"title":"T","payload":[{"id":"a","revision":1,"kind":"text","filename":"a","language":"text","text":"a","text":"a"}]}"#,
    ] {
        assert!(TaskDraftPublicationPlan::parse_json(bytes.as_bytes()).is_err());
    }
    for field in [
        "owner",
        "artifact",
        "workspace",
        "path",
        "task_id",
        "expected_revision",
    ] {
        let mut value = draft(vec![item(0, "text")]);
        value["payload"][0][field] = json!("untrusted");
        assert!(parse(&value).is_err(), "unknown item field {field}");
    }
    let mut value = draft(vec![item(0, "text")]);
    value["payload"][0]["kind"] = json!({"text":null});
    assert!(parse(&value).is_err());
    value["payload"][0]["kind"] = json!("binary");
    assert!(parse(&value).is_err());
    for key in ["format", "version", "title", "payload"] {
        let mut value = draft(vec![]);
        value.as_object_mut().unwrap().remove(key);
        assert!(parse(&value).is_err());
    }
    for key in ["id", "revision", "kind", "filename", "language", "text"] {
        let mut value = draft(vec![item(0, "text")]);
        value["payload"][0].as_object_mut().unwrap().remove(key);
        assert!(parse(&value).is_err());
    }
}

#[test]
fn import_requires_exact_format_and_lexical_positive_safe_integer_versions() {
    let mut value = draft(vec![]);
    for format in ["cyanrex.task-content", "cyanrex.task-draft ", ""] {
        value["format"] = json!(format);
        assert!(parse(&value).is_err());
    }
    for version in ["0", "2", "1.0", "1e0", "-1", "\"1\"", "null"] {
        let body = format!(
            r#"{{"format":"cyanrex.task-draft","version":{version},"title":"T","payload":[]}}"#
        );
        assert!(TaskDraftPublicationPlan::parse_json(body.as_bytes()).is_err());
    }
    for revision in [
        "0",
        "-1",
        "1.0",
        "1e0",
        "9007199254740992",
        "18446744073709551616",
        "\"1\"",
        "null",
    ] {
        let body = format!(
            r#"{{"format":"cyanrex.task-draft","version":1,"title":"T","payload":[{{"id":"a","revision":{revision},"kind":"text","filename":"a","language":"text","text":""}}]}}"#
        );
        assert!(TaskDraftPublicationPlan::parse_json(body.as_bytes()).is_err());
    }
    let mut value = draft(vec![item(0, "")]);
    value["payload"][0]["revision"] = json!(9_007_199_254_740_991_u64);
    assert!(parse(&value).is_ok());
}

#[test]
fn local_ids_are_bounded_unique_ascii_labels_not_server_identities() {
    let mut value = draft(vec![item(0, "")]);
    for invalid in [
        "".to_owned(),
        "a".repeat(81),
        "local.id".into(),
        "a/b".into(),
        "中文".into(),
        " a ".into(),
    ] {
        value["payload"][0]["id"] = json!(invalid);
        assert!(parse(&value).is_err());
    }
    value["payload"][0]["id"] = json!("_Az09-".repeat(13));
    assert!(parse(&value).is_ok());
    assert!(parse(&draft(vec![item(0, "first"), item(0, "second")])).is_err());
}

#[test]
fn explicit_task_title_is_required_bounded_in_utf8_and_never_trimmed() {
    let mut value = draft(vec![]);
    for title in [
        "".into(),
        " \t\n".into(),
        "\u{2003}".into(),
        "x".repeat(257),
        "中".repeat(86),
        "a\0b".into(),
    ] {
        value["title"] = json!(title);
        assert!(parse(&value).is_err());
    }
    for title in [
        "x".repeat(256),
        format!("{}x", "中".repeat(85)),
        "  unchanged  ".into(),
    ] {
        value["title"] = json!(title);
        assert_eq!(parse(&value).ok().unwrap().title(), title);
    }
}

#[test]
fn metadata_preserves_utf16_filename_labels_and_language_neutral_hints() {
    let mut value = draft(vec![item(0, "")]);
    for filename in [
        "中".repeat(128),
        "🙂".repeat(64),
        " ".into(),
        "  unchanged.txt  ".into(),
    ] {
        value["payload"][0]["filename"] = json!(filename);
        value["payload"][0]["language"] = json!("future.language+v2-test_3");
        let plan = parse(&value).ok().unwrap();
        assert_eq!(plan.items()[0].filename(), filename);
        assert_eq!(plan.items()[0].language(), "future.language+v2-test_3");
    }
    for filename in [
        "".into(),
        ".".into(),
        "..".into(),
        "a/b".into(),
        "a\\b".into(),
        "a\nb".into(),
        "🙂".repeat(65),
        "x".repeat(129),
    ] {
        value["payload"][0]["filename"] = json!(filename);
        assert!(parse(&value).is_err());
    }
    value["payload"][0]["filename"] = json!("note.txt");
    for language in [
        "".into(),
        "Rust".into(),
        "1rust".into(),
        "a/b".into(),
        "a b".into(),
        "a".repeat(65),
        "中文".into(),
    ] {
        value["payload"][0]["language"] = json!(language);
        assert!(parse(&value).is_err());
    }
    value["payload"][0]["language"] = json!("a".repeat(64));
    assert!(parse(&value).is_ok());
}

#[test]
fn text_bound_counts_utf8_bytes_and_allows_only_tab_lf_cr_controls() {
    for text in [
        "x".repeat(MAX_TEXT_PAYLOAD_BYTES),
        "中".repeat(MAX_TEXT_PAYLOAD_BYTES / 3),
        "\t\r\n".into(),
        "".into(),
    ] {
        let plan = parse(&draft(vec![item(0, &text)])).ok().unwrap();
        assert_eq!(plan.items()[0].text(), text);
    }
    for text in [
        "x".repeat(MAX_TEXT_PAYLOAD_BYTES + 1),
        "中".repeat(MAX_TEXT_PAYLOAD_BYTES / 3 + 1),
    ] {
        assert!(parse(&draft(vec![item(0, &text)])).is_err());
    }
    for value in (0u8..=31).chain(127..=159) {
        let character = char::from(value);
        assert_eq!(
            parse(&draft(vec![item(0, &character.to_string())])).is_ok(),
            matches!(character, '\t' | '\n' | '\r')
        );
    }
}

#[test]
fn json_envelope_is_bounded_before_parsing_and_includes_escaping_and_metadata() {
    let mut body = serde_json::to_vec(&draft(vec![])).unwrap();
    body.resize(MAX_TASK_DRAFT_BYTES, b' ');
    assert!(TaskDraftPublicationPlan::parse_json(&body).is_ok());
    body.push(b' ');
    assert!(TaskDraftPublicationPlan::parse_json(&body).is_err());
    assert!(parse(&draft((0..32).map(|i| item(i, "")).collect())).is_ok());
    assert!(parse(&draft((0..33).map(|i| item(i, "")).collect())).is_err());
    let raw = "x".repeat(MAX_TEXT_PAYLOAD_BYTES);
    let max_raw = serde_json::to_vec(&draft((0..32).map(|i| item(i, &raw)).collect())).unwrap();
    assert!(max_raw.len() > MAX_TASK_DRAFT_BYTES);
    assert!(TaskDraftPublicationPlan::parse_json(&max_raw).is_err());
    let escaped = "\n".repeat(MAX_TEXT_PAYLOAD_BYTES);
    let escaped = serde_json::to_vec(&draft((0..16).map(|i| item(i, &escaped)).collect())).unwrap();
    assert!(escaped.len() > MAX_TASK_DRAFT_BYTES);
    assert!(TaskDraftPublicationPlan::parse_json(&escaped).is_err());
}

#[test]
fn json_rejects_bom_invalid_utf8_surrogates_and_trailing_records_without_echo() {
    let body = serde_json::to_vec(&draft(vec![])).unwrap();
    let mut bom = vec![0xef, 0xbb, 0xbf];
    bom.extend(&body);
    let mut trailing = body.clone();
    trailing.extend(b"{}");
    for bytes in [
        bom,
        trailing,
        vec![0xff],
        br#"{"format":"cyanrex.task-draft","version":1,"title":"\ud800","payload":[]}"#.to_vec(),
    ] {
        assert!(TaskDraftPublicationPlan::parse_json(&bytes).is_err());
    }
    let secret_marker = "SECRET_NEVER_ECHO";
    let mut value = draft(vec![item(0, secret_marker)]);
    value["payload"][0]["revision"] = json!(secret_marker);
    let error = parse(&value).err().unwrap();
    assert!(!format!("{error:?} {error}").contains(secret_marker));
}
