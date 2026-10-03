use cyanrex_engine::models::collaboration::*;
use serde_json::{json, Value};

const ID: &str = "8b075b0b-8118-4c37-9224-337ba22f40c6";
const OTHER: &str = "267c6b0f-182c-4980-a925-cdc967327c57";

fn reference(index: u128) -> ArtifactRef {
    ArtifactRef {
        workspace: WorkspaceRef {
            authority_id: ID.parse().unwrap(),
            workspace_id: OTHER.parse().unwrap(),
        },
        artifact_id: ArtifactId::try_from(uuid::Uuid::from_u128(index + 1)).unwrap(),
        revision_id: ID.parse().unwrap(),
        sha256: "a".repeat(64).parse().unwrap(),
    }
}

fn item(index: u128) -> TextPayloadBinding {
    TextPayloadBinding::new(reference(index), "notes.md", "markdown").unwrap()
}

fn manifest() -> TaskContentManifest {
    TaskContentManifest::new("A non-teaching task", vec![item(0), item(1)]).unwrap()
}

fn snapshot(value: &TaskContentManifest) -> TaskSnapshot {
    TaskSnapshot {
        schema_version: CoreSchemaVersion,
        reference: TaskRef {
            workspace: reference(0).workspace,
            task_id: ID.parse().unwrap(),
        },
        owner: PrincipalRef {
            authority_id: ID.parse().unwrap(),
            principal_id: OTHER.parse().unwrap(),
        },
        title: value.title().into(),
        definition: None,
        input_refs: value.input_refs(),
        status: TaskStatus::Draft,
        revision: RevisionNumber::try_from(1).unwrap(),
        created_at: "2026-10-03T00:00:00Z".parse().unwrap(),
        updated_at: "2026-10-03T00:00:00Z".parse().unwrap(),
    }
}

#[test]
fn content_manifest_round_trips_exact_metadata_and_pins_without_local_authority() {
    let original = manifest();
    let encoded = serde_json::to_vec(&original).unwrap();
    let decoded = TaskContentManifest::parse_json(&encoded).unwrap();
    assert_eq!(original, decoded);
    assert_eq!(u8::from(decoded.schema_version()), 1);
    assert_eq!(decoded.payload()[0].artifact(), &reference(0));
    assert_eq!(decoded.payload()[0].filename(), "notes.md");
    assert_eq!(decoded.payload()[0].language(), "markdown");
    let value: Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(value["payload"][0]["kind"], "text");
    assert_eq!(value.as_object().unwrap().len(), 3);
    assert_eq!(value["payload"][0].as_object().unwrap().len(), 4);
}

#[test]
fn content_manifest_allows_named_tasks_without_inventing_payload_or_code() {
    let value = TaskContentManifest::new("Discuss a proposal", vec![]).unwrap();
    assert!(value.payload().is_empty());
    assert!(value.input_refs().is_empty());
    assert!(value.validate_task_snapshot(&snapshot(&value)).is_ok());
}

#[test]
fn saved_content_requires_a_bounded_nonblank_title_without_silent_normalization() {
    for title in ["", " \t ", "\n", "bad\0title", "bad\u{85}title"] {
        assert!(TaskContentManifest::new(title, vec![]).is_err());
    }
    assert!(TaskContentManifest::new("a".repeat(256), vec![]).is_ok());
    assert!(TaskContentManifest::new("a".repeat(257), vec![]).is_err());
    assert!(TaskContentManifest::new("中".repeat(85), vec![]).is_ok());
    assert!(TaskContentManifest::new("中".repeat(86), vec![]).is_err());
    let preserved = TaskContentManifest::new("  Cafe\u{301} 📝  ", vec![]).unwrap();
    assert_eq!(preserved.title(), "  Cafe\u{301} 📝  ");
}

#[test]
fn filename_labels_use_utf16_bounds_and_preserve_valid_local_metadata() {
    for filename in [
        "a".repeat(128),
        "😀".repeat(64),
        format!("{}😀", "a".repeat(126)),
        "中".repeat(128),
        "CON".into(),
        "   ".into(),
        "a<>:\"|?*. ".into(),
        "notes\u{202e}.txt".into(),
    ] {
        let entry = TextPayloadBinding::new(reference(0), &filename, "plaintext").unwrap();
        assert_eq!(entry.filename(), filename);
    }
    for filename in [
        "".into(),
        ".".into(),
        "..".into(),
        "../notes".into(),
        "a/b".into(),
        "a\\b".into(),
        "bad\u{7f}".into(),
        "bad\u{9f}".into(),
        "line\nbreak".into(),
        "a".repeat(129),
        format!("{}😀", "a".repeat(127)),
        "😀".repeat(65),
    ] {
        assert!(TextPayloadBinding::new(reference(0), filename, "plaintext").is_err());
    }
}

#[test]
fn language_hints_are_extensible_data_not_frontend_capability_or_execution_selectors() {
    for language in [
        "typescript",
        "javascript",
        "json",
        "html",
        "css",
        "rust",
        "python",
        "c",
        "cpp",
        "markdown",
        "yaml",
        "sql",
        "shell",
        "plaintext",
        "c++",
        "future.language-v2",
    ] {
        let value = TextPayloadBinding::new(reference(0), "anything", language).unwrap();
        assert_eq!(value.language(), language);
    }
    assert!(TextPayloadBinding::new(reference(0), "x", "a".repeat(64)).is_ok());
    for language in [
        "".into(),
        "Rust".into(),
        "1lang".into(),
        " rust".into(),
        "rust ".into(),
        "rust/analyzer".into(),
        "exec:sh".into(),
        "sh;id".into(),
        "中文".into(),
        "a".repeat(65),
    ] {
        assert!(TextPayloadBinding::new(reference(0), "x", language).is_err());
    }
}

#[test]
fn ordered_payload_bounds_and_duplicate_revision_rules_match_task_inputs() {
    assert_eq!(MAX_TASK_CONTENT_ITEMS, 32);
    assert_eq!(MAX_TEXT_PAYLOAD_BYTES, 256 * 1024);
    assert!(TaskContentManifest::new("Notes", (0..32).map(item).collect()).is_ok());
    assert!(TaskContentManifest::new("Notes", (0..33).map(item).collect()).is_err());
    assert!(TaskContentManifest::new("Notes", vec![item(0), item(0)]).is_err());
    let mut forged = reference(0);
    forged.sha256 = "b".repeat(64).parse().unwrap();
    assert!(TaskContentManifest::new(
        "Notes",
        vec![
            item(0),
            TextPayloadBinding::new(forged, "renamed", "json").unwrap()
        ]
    )
    .is_err());
    let mut next = reference(0);
    next.revision_id = OTHER.parse().unwrap();
    let original = TaskContentManifest::new(
        "Notes",
        vec![
            item(0),
            TextPayloadBinding::new(next.clone(), "notes.md", "markdown").unwrap(),
        ],
    )
    .unwrap();
    assert_eq!(original.input_refs(), vec![reference(0), next]);
}

#[test]
fn mixed_authorities_or_workspaces_cannot_share_a_content_manifest() {
    for authority in [false, true] {
        let mut changed = reference(1);
        if authority {
            changed.workspace.authority_id = OTHER.parse().unwrap();
        } else {
            changed.workspace.workspace_id = ID.parse().unwrap();
        }
        let second = TextPayloadBinding::new(changed, "notes.md", "markdown").unwrap();
        assert!(TaskContentManifest::new("Notes", vec![item(0), second]).is_err());
    }
}

#[test]
fn strict_deserialization_cannot_bypass_constructors_or_smuggle_authority() {
    let base = serde_json::to_value(manifest()).unwrap();
    for field in [
        "owner",
        "task_id",
        "revision",
        "input_refs",
        "status",
        "format",
    ] {
        let mut value = base.clone();
        value[field] = json!("untrusted");
        assert!(serde_json::from_value::<TaskContentManifest>(value).is_err());
    }
    for field in ["id", "revision", "text", "owner", "path", "run", "sha256"] {
        let mut value = base.clone();
        value["payload"][0][field] = json!("untrusted");
        assert!(serde_json::from_value::<TaskContentManifest>(value).is_err());
    }
    for (field, invalid) in [
        ("kind", json!("binary")),
        ("filename", json!("../secret")),
        ("language", json!("exec:sh")),
        ("artifact", Value::Null),
    ] {
        let mut value = base.clone();
        value["payload"][0][field] = invalid;
        assert!(serde_json::from_value::<TaskContentManifest>(value).is_err());
    }
    let mut blank = base.clone();
    blank["title"] = json!("");
    assert!(serde_json::from_value::<TaskContentManifest>(blank).is_err());
    let mut duplicate = base.clone();
    duplicate["payload"][1] = duplicate["payload"][0].clone();
    assert!(serde_json::from_value::<TaskContentManifest>(duplicate).is_err());
    for field in ["schema_version", "title", "payload"] {
        let mut missing = base.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<TaskContentManifest>(missing).is_err());
    }
    for version in [json!(0), json!(2), json!(1.0), json!("1"), Value::Null] {
        let mut value = base.clone();
        value["schema_version"] = version;
        assert!(serde_json::from_value::<TaskContentManifest>(value).is_err());
    }
}

#[test]
fn manifest_json_is_bounded_and_is_not_the_local_draft_import_format() {
    assert_eq!(MAX_TASK_CONTENT_MANIFEST_BYTES, 64 * 1024);
    let encoded = serde_json::to_vec(&manifest()).unwrap();
    let mut padded = encoded.clone();
    padded.resize(MAX_TASK_CONTENT_MANIFEST_BYTES, b' ');
    assert!(TaskContentManifest::parse_json(&padded).is_ok());
    padded.push(b' ');
    assert!(TaskContentManifest::parse_json(&padded).is_err());
    let mut bom = vec![0xef, 0xbb, 0xbf];
    bom.extend(&encoded);
    assert!(TaskContentManifest::parse_json(&bom).is_err());
    assert!(TaskContentManifest::parse_json(b"\xff").is_err());
    assert!(TaskContentManifest::parse_json(
        br#"{"schema_version":1,"title":"\ud800","payload":[]}"#
    )
    .is_err());
    assert!(TaskContentManifest::parse_json(
        br#"{"schema_version":1,"title":"one","title":"two","payload":[]}"#
    )
    .is_err());
    assert!(TaskContentManifest::parse_json(
        br#"{"format":"cyanrex.task-draft","version":1,"title":"Draft","payload":[]}"#
    )
    .is_err());
}

#[test]
fn manifest_accepts_only_object_records_and_a_string_kind_not_serde_sequence_forms() {
    let base = serde_json::to_value(manifest()).unwrap();
    let record = base["payload"][0].clone();
    let pin = record["artifact"].clone();
    let workspace = pin["workspace"].clone();
    let mut cases = vec![json!([1, base["title"], base["payload"]])];
    let mut changed = base.clone();
    changed["payload"][0] = json!(["text", record["filename"], record["language"], pin]);
    cases.push(changed);
    changed = base.clone();
    changed["payload"][0]["artifact"] = json!([
        workspace,
        pin["artifact_id"],
        pin["revision_id"],
        pin["sha256"]
    ]);
    cases.push(changed);
    changed = base.clone();
    changed["payload"][0]["artifact"]["workspace"] =
        json!([workspace["authority_id"], workspace["workspace_id"]]);
    cases.push(changed);
    changed = base;
    changed["payload"][0]["kind"] = json!({"text": null});
    cases.push(changed);
    for value in cases {
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(
            TaskContentManifest::parse_json(&bytes).is_err(),
            "accepted {value}"
        );
        assert!(serde_json::from_value::<TaskContentManifest>(value).is_err());
    }
}

#[test]
fn snapshot_matching_rejects_title_count_order_scope_and_exact_pin_drift() {
    let original = manifest();
    let task = snapshot(&original);
    assert!(original.validate_task_snapshot(&task).is_ok());
    let mut changed = task.clone();
    changed.title.push('!');
    assert!(original.validate_task_snapshot(&changed).is_err());
    changed = task.clone();
    changed.input_refs.pop();
    assert!(original.validate_task_snapshot(&changed).is_err());
    changed = task.clone();
    changed.input_refs.push(reference(2));
    assert!(original.validate_task_snapshot(&changed).is_err());
    changed = task.clone();
    changed.input_refs.swap(0, 1);
    assert!(original.validate_task_snapshot(&changed).is_err());
    changed = task.clone();
    changed.input_refs[0].sha256 = "b".repeat(64).parse().unwrap();
    assert!(original.validate_task_snapshot(&changed).is_err());
    changed = task.clone();
    changed.input_refs[0].revision_id = OTHER.parse().unwrap();
    assert!(original.validate_task_snapshot(&changed).is_err());
    changed = task.clone();
    changed.reference.workspace.workspace_id = ID.parse().unwrap();
    assert!(original.validate_task_snapshot(&changed).is_err());
    changed = task.clone();
    changed.owner.authority_id = OTHER.parse().unwrap();
    assert!(original.validate_task_snapshot(&changed).is_err());
}

#[test]
fn snapshot_matching_is_read_only_and_never_a_session_or_revision_authorization() {
    let original = manifest();
    let mut task = snapshot(&original);
    task.status = TaskStatus::InReview;
    task.revision = RevisionNumber::try_from(19).unwrap();
    task.owner.principal_id = ID.parse().unwrap();
    // The manifest carries no owner, Session or expected Task revision. Structural matching alone
    // cannot establish any of them, and can also describe an immutable historical task snapshot.
    let before = task.clone();
    assert!(original.validate_task_snapshot(&task).is_ok());
    assert_eq!(task, before);
}
