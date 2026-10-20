use super::*;
use crate::models::collaboration::{ArtifactRef, CoreSchemaVersion, TaskRef, TaskSnapshot};
use chrono::{DateTime, Utc};

fn id<T: std::str::FromStr>(value: u128) -> T
where
    T::Err: std::fmt::Debug,
{
    uuid::Uuid::from_u128(value).to_string().parse().unwrap()
}

fn revision(value: u64) -> RevisionNumber {
    value.try_into().unwrap()
}

fn snapshot() -> TaskContentSnapshot {
    let scope = crate::models::collaboration::WorkspaceRef {
        authority_id: id(1),
        workspace_id: id(2),
    };
    let manifest = TaskContentManifest::new(
        "Discussion",
        vec![TextPayloadBinding::new(
            ArtifactRef {
                workspace: scope,
                artifact_id: id(3),
                revision_id: id(4),
                sha256: "a".repeat(64).parse().unwrap(),
            },
            "notes.txt",
            "plaintext",
        )
        .unwrap()],
    )
    .unwrap();
    let now = DateTime::<Utc>::from_timestamp(10, 0).unwrap();
    TaskContentSnapshot {
        task: TaskSnapshot {
            schema_version: CoreSchemaVersion,
            reference: TaskRef {
                workspace: scope,
                task_id: id(5),
            },
            owner: PrincipalRef {
                authority_id: id(1),
                principal_id: id(6),
            },
            title: manifest.title().into(),
            definition: None,
            input_refs: manifest.input_refs(),
            status: TaskStatus::Draft,
            revision: revision(1),
            created_at: now,
            updated_at: now,
        },
        manifest,
    }
}

#[test]
fn replacement_checks_retain_conflicting_old_and_new_pins_with_distinct_error_roles() {
    let original = snapshot();
    let old = original.manifest.payload()[0].clone();
    let mut reference = old.artifact().clone();
    reference.sha256 = "b".repeat(64).parse().unwrap();
    let new = TextPayloadBinding::new(reference, "new.txt", "plaintext").unwrap();
    let manifest = TaskContentManifest::new("Revised", vec![new.clone()]).unwrap();
    assert_eq!(
        replacement_content_checks(&original, revision(1), &manifest).unwrap(),
        vec![
            (old, TaskStoreError::InvalidRecord),
            (new, TaskStoreError::InvalidInput)
        ]
    );
}

#[test]
fn replacement_checks_keep_removed_old_inputs_even_when_new_payload_is_empty() {
    let original = snapshot();
    let manifest = TaskContentManifest::new("Empty", vec![]).unwrap();
    assert_eq!(
        replacement_content_checks(&original, revision(1), &manifest).unwrap(),
        vec![(
            original.manifest.payload()[0].clone(),
            TaskStoreError::InvalidRecord
        )]
    );
}

#[test]
fn replacement_checks_reject_stale_then_non_draft_then_noop_in_original_order() {
    let mut original = snapshot();
    original.task.status = TaskStatus::Ready;
    assert_eq!(
        replacement_content_checks(&original, revision(2), &original.manifest),
        Err(TaskStoreError::StaleRevision.into())
    );
    assert_eq!(
        replacement_content_checks(&original, revision(1), &original.manifest),
        Err(TaskStoreError::InvalidTransition.into())
    );
    original.task.status = TaskStatus::Draft;
    assert_eq!(
        replacement_content_checks(&original, revision(1), &original.manifest),
        Err(TaskStoreError::InvalidInput.into())
    );
}

#[test]
fn replacement_checks_allow_title_only_change_of_an_empty_manual_task() {
    let mut original = snapshot();
    original.manifest = TaskContentManifest::new("Empty", vec![]).unwrap();
    original.task.title = "Empty".into();
    original.task.input_refs.clear();
    assert!(replacement_content_checks(
        &original,
        revision(1),
        &TaskContentManifest::new("Renamed empty", vec![]).unwrap(),
    )
    .unwrap()
    .is_empty());
}
