use super::*;

fn id<T: std::str::FromStr>(value: u128) -> T
where
    T::Err: std::fmt::Debug,
{
    uuid::Uuid::from_u128(value).to_string().parse().unwrap()
}

fn revision(value: u64) -> RevisionNumber {
    value.try_into().unwrap()
}

fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(20, 0).unwrap()
}

fn snapshot() -> TaskContentSnapshot {
    let scope = WorkspaceRef {
        authority_id: id(1),
        workspace_id: id(2),
    };
    let manifest = TaskContentManifest::new(
        "Document",
        vec![TextPayloadBinding::new(
            ArtifactRef {
                workspace: scope,
                artifact_id: id(3),
                revision_id: id(4),
                sha256: "a".repeat(64).parse().unwrap(),
            },
            "old.txt",
            "plaintext",
        )
        .unwrap()],
    )
    .unwrap();
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
            created_at: DateTime::from_timestamp(10, 0).unwrap(),
            updated_at: DateTime::from_timestamp(10, 0).unwrap(),
        },
        manifest,
    }
}

#[test]
fn content_replacement_changes_metadata_at_one_task_revision() {
    let original = snapshot();
    let manifest = TaskContentManifest::new(
        "Renamed",
        vec![
            TextPayloadBinding::new(original.task.input_refs[0].clone(), "new.rs", "rust").unwrap(),
        ],
    )
    .unwrap();
    let next = prepare_replacement(&original, revision(1), manifest.clone(), now()).unwrap();
    assert_eq!(next.manifest, manifest);
    assert_eq!(next.task.title, "Renamed");
    assert_eq!(next.task.input_refs, original.task.input_refs);
    assert_eq!(next.task.revision, revision(2));
    assert_eq!(next.task.updated_at, now());
    assert_eq!(next.task.created_at, original.task.created_at);
    assert_eq!(next.task.reference, original.task.reference);
    assert_eq!(next.task.owner, original.task.owner);
    assert_eq!(next.task.definition, None);
    assert_eq!(next.task.status, TaskStatus::Draft);
    assert_eq!(original, snapshot());
}

#[test]
fn content_replacement_rejects_stale_noop_and_non_draft() {
    let original = snapshot();
    assert_eq!(
        prepare_replacement(&original, revision(2), original.manifest.clone(), now()),
        Err(TaskStoreError::StaleRevision)
    );
    assert_eq!(
        prepare_replacement(&original, revision(1), original.manifest.clone(), now()),
        Err(TaskStoreError::InvalidInput)
    );
    for status in [
        TaskStatus::Ready,
        TaskStatus::InProgress,
        TaskStatus::Blocked,
        TaskStatus::InReview,
        TaskStatus::Cancelled,
    ] {
        let mut current = original.clone();
        current.task.status = status;
        assert_eq!(
            prepare_replacement(
                &current,
                revision(1),
                TaskContentManifest::new("New", vec![]).unwrap(),
                now()
            ),
            Err(TaskStoreError::InvalidTransition)
        );
    }
}

#[test]
fn content_replacement_can_empty_payload_but_not_change_scope() {
    let original = snapshot();
    let empty = TaskContentManifest::new("Empty", vec![]).unwrap();
    let next = prepare_replacement(&original, revision(1), empty.clone(), now()).unwrap();
    assert_eq!(next.manifest, empty);
    assert!(next.task.input_refs.is_empty());
    let mut foreign = original.task.input_refs[0].clone();
    foreign.workspace.workspace_id = id(8);
    let manifest = TaskContentManifest::new(
        "Foreign",
        vec![TextPayloadBinding::new(foreign, "a", "text").unwrap()],
    )
    .unwrap();
    assert_eq!(
        prepare_replacement(&original, revision(1), manifest, now()),
        Err(TaskStoreError::ScopeMismatch)
    );
}

#[test]
fn content_transition_preserves_manifest_and_content_fields() {
    let original = snapshot();
    let next = prepare_transition(&original, revision(1), TaskStatus::Ready, now()).unwrap();
    let mut expected = original.clone();
    expected.task.status = TaskStatus::Ready;
    expected.task.revision = revision(2);
    expected.task.updated_at = now();
    assert_eq!(next, expected);
    assert_eq!(
        prepare_transition(&next, revision(1), TaskStatus::InProgress, now()),
        Err(TaskStoreError::StaleRevision)
    );
    assert_eq!(
        prepare_transition(&next, revision(2), TaskStatus::Draft, now()),
        Err(TaskStoreError::InvalidTransition)
    );
}

#[test]
fn content_mutation_refuses_revision_overflow() {
    let mut current = snapshot();
    current.task.revision = revision(9_007_199_254_740_991);
    assert_eq!(
        prepare_replacement(
            &current,
            current.task.revision,
            TaskContentManifest::new("New", vec![]).unwrap(),
            now()
        ),
        Err(TaskStoreError::InvalidRecord)
    );
    assert_eq!(
        prepare_transition(&current, current.task.revision, TaskStatus::Ready, now()),
        Err(TaskStoreError::InvalidRecord)
    );
}

#[test]
fn content_events_never_alias_legacy_input_replacement() {
    let mut current = snapshot();
    assert_eq!(records::event_type(&current.task), "cyanrex.task.created");
    current.task.revision = revision(2);
    assert_eq!(
        records::event_type(&current.task),
        "cyanrex.task.content_updated"
    );
    current.task.status = TaskStatus::Ready;
    assert_eq!(
        records::event_type(&current.task),
        "cyanrex.task.status_changed"
    );
    assert_eq!(TASK_STORAGE_VERSION, 2);
    assert_eq!(TASK_CONTENT_STORAGE_VERSION, 3);
}
