use chrono::{TimeZone, Utc};
use cyanrex_engine::{
    models::collaboration::*, services::task_content::validate_task_content_snapshot,
};
use sha2::{Digest, Sha256};

fn id<T: TryFrom<uuid::Uuid>>(value: u128) -> T
where
    T::Error: std::fmt::Debug,
{
    uuid::Uuid::from_u128(value).try_into().unwrap()
}

fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: id(1),
        workspace_id: id(2),
    }
}

fn owner() -> PrincipalRef {
    PrincipalRef {
        authority_id: scope().authority_id,
        principal_id: id(3),
    }
}

fn content(index: u128, bytes: &[u8]) -> ArtifactContent {
    ArtifactContent {
        revision: ArtifactRevision {
            schema_version: CoreSchemaVersion,
            reference: ArtifactRef {
                workspace: scope(),
                artifact_id: id(100 + index),
                revision_id: id(200 + index),
                sha256: format!("{:x}", Sha256::digest(bytes)).parse().unwrap(),
            },
            owner: owner(),
            sequence: 1.try_into().unwrap(),
            parent: None,
            kind: "example.document".parse().unwrap(),
            title: "Content".into(),
            media_type: "text/plain".into(),
            byte_length: bytes.len() as u64,
            created_at: Utc.timestamp_opt(0, 0).unwrap(),
        },
        bytes: bytes.to_vec(),
    }
}

fn manifest(contents: &[ArtifactContent]) -> TaskContentManifest {
    TaskContentManifest::new(
        "Write a document",
        contents
            .iter()
            .map(|content| {
                TextPayloadBinding::new(
                    content.revision.reference.clone(),
                    "document.txt",
                    "plaintext",
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn task(manifest: &TaskContentManifest) -> TaskSnapshot {
    TaskSnapshot {
        schema_version: CoreSchemaVersion,
        reference: TaskRef {
            workspace: scope(),
            task_id: id(4),
        },
        owner: owner(),
        title: manifest.title().into(),
        definition: None,
        input_refs: manifest.input_refs(),
        status: TaskStatus::Draft,
        revision: 1.try_into().unwrap(),
        created_at: Utc.timestamp_opt(0, 0).unwrap(),
        updated_at: Utc.timestamp_opt(0, 0).unwrap(),
    }
}

fn accepts(contents: &[ArtifactContent]) -> bool {
    let manifest = manifest(contents);
    validate_task_content_snapshot(&manifest, &task(&manifest), contents).is_ok()
}

#[test]
fn task_content_accepts_exact_unicode_text_without_mutating_inputs() {
    let contents = vec![
        content(1, "中文🙂\tline\r\nnext\n".as_bytes()),
        content(2, b""),
    ];
    let manifest = manifest(&contents);
    let task = task(&manifest);
    let before = (manifest.clone(), task.clone(), contents.clone());
    assert!(validate_task_content_snapshot(&manifest, &task, &contents).is_ok());
    assert_eq!((manifest, task, contents), before);
}

#[test]
fn task_content_accepts_an_empty_manifest_without_claiming_session_admission() {
    // Manual Session input commands have a separate nonempty policy. This is structural only.
    assert!(accepts(&[]));
}

#[test]
fn task_content_preserves_distinct_exact_revisions_of_the_same_artifact() {
    let first = content(1, b"old text");
    let mut second = content(2, b"new text");
    second.revision.reference.artifact_id = first.revision.reference.artifact_id;
    second.revision.sequence = 2.try_into().unwrap();
    second.revision.parent = Some(first.revision.reference.clone());
    assert!(accepts(&[first, second]));
}

#[test]
fn task_content_rejects_missing_extra_and_reordered_content() {
    let contents = vec![content(1, b"first"), content(2, b"second")];
    let manifest = manifest(&contents);
    let task = task(&manifest);
    for altered in [
        vec![],
        vec![contents[0].clone()],
        vec![
            contents[0].clone(),
            contents[1].clone(),
            content(3, b"extra"),
        ],
        vec![contents[1].clone(), contents[0].clone()],
    ] {
        assert!(validate_task_content_snapshot(&manifest, &task, &altered).is_err());
    }
}

#[test]
fn task_content_rejects_every_changed_reference_coordinate() {
    let contents = vec![content(1, b"same bytes")];
    let manifest = manifest(&contents);
    let task = task(&manifest);
    let original = contents[0].clone();
    let mut cases = vec![original.clone(); 5];
    cases[0].revision.reference.workspace.authority_id = id(8);
    cases[1].revision.reference.workspace.workspace_id = id(8);
    cases[2].revision.reference.artifact_id = id(8);
    cases[3].revision.reference.revision_id = id(8);
    cases[4].revision.reference.sha256 = "0".repeat(64).parse().unwrap();
    for changed in cases {
        assert!(validate_task_content_snapshot(&manifest, &task, &[changed]).is_err());
    }
}

#[test]
fn task_content_rejects_content_owned_by_another_principal_or_authority() {
    let contents = vec![content(1, b"text")];
    let manifest = manifest(&contents);
    let task = task(&manifest);
    for other in [
        PrincipalRef {
            principal_id: id(8),
            ..owner()
        },
        PrincipalRef {
            authority_id: id(8),
            ..owner()
        },
    ] {
        let mut changed = contents[0].clone();
        changed.revision.owner = other;
        assert!(validate_task_content_snapshot(&manifest, &task, &[changed]).is_err());
    }
}

#[test]
fn task_content_rejects_misstated_byte_lengths() {
    for length in [0, 3, 5, u64::MAX] {
        let mut changed = content(1, b"text");
        changed.revision.byte_length = length;
        assert!(!accepts(&[changed]), "accepted length {length}");
    }
}

#[test]
fn task_content_bounds_each_text_item_in_bytes() {
    assert!(accepts(&[content(1, &vec![b'x'; MAX_TEXT_PAYLOAD_BYTES])]));
    assert!(!accepts(&[content(
        1,
        &vec![b'x'; MAX_TEXT_PAYLOAD_BYTES + 1]
    )]));
    // The byte bound, not a character count, applies to valid multi-byte UTF-8 too.
    assert!(!accepts(&[content(
        1,
        "中".repeat(MAX_TEXT_PAYLOAD_BYTES / 3 + 1).as_bytes()
    )]));
}

#[test]
fn task_content_rejects_invalid_utf8_even_with_a_matching_digest() {
    for bytes in [
        vec![0xff],
        vec![0xc0, 0xaf],
        vec![0xf0, 0x9f, 0x99],
        vec![0xed, 0xa0, 0x80],
    ] {
        assert!(!accepts(&[content(1, &bytes)]));
    }
}

#[test]
fn task_content_allows_only_tab_lf_and_cr_control_characters() {
    for value in (0u8..=31).chain(127..=159) {
        let character = char::from(value);
        let accepted = accepts(&[content(1, format!("before{character}after").as_bytes())]);
        assert_eq!(
            accepted,
            matches!(character, '\t' | '\n' | '\r'),
            "U+{value:04X}"
        );
    }
}

#[test]
fn task_content_recomputes_digest_after_a_same_size_content_change() {
    let mut changed = content(1, b"good");
    changed.bytes = b"evil".to_vec();
    assert!(!accepts(&[changed]));
}

#[test]
fn task_content_rejects_a_forged_digest_shared_by_all_supplied_pins() {
    let mut changed = content(1, b"text");
    changed.revision.reference.sha256 = "0".repeat(64).parse().unwrap();
    assert!(!accepts(&[changed]));
}

#[test]
fn task_content_checks_the_task_title_and_ordered_manifest_references() {
    let contents = vec![content(1, b"first"), content(2, b"second")];
    let manifest = manifest(&contents);
    let original = task(&manifest);
    let mut cases = vec![original.clone(); 4];
    cases[0].title = "Different task title".into();
    cases[1].input_refs.reverse();
    cases[2].input_refs.pop();
    cases[3].reference.workspace.workspace_id = id(8);
    for changed in cases {
        assert!(validate_task_content_snapshot(&manifest, &changed, &contents).is_err());
    }
}

#[test]
fn task_content_does_not_infer_execution_or_text_validity_from_metadata_labels() {
    let mut text = content(1, b"print('not executed')\n");
    text.revision.kind = "example.existing_payload".parse().unwrap();
    text.revision.media_type = "application/octet-stream".into();
    let manifest = TaskContentManifest::new(
        "Write a document",
        vec![
            TextPayloadBinding::new(text.revision.reference.clone(), "script.py", "python")
                .unwrap(),
        ],
    )
    .unwrap();
    assert!(validate_task_content_snapshot(&manifest, &task(&manifest), &[text]).is_ok());
}
