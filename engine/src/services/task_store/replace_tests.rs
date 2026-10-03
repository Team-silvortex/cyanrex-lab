use super::super::*;
use super::prepare_replacement;
use chrono::{DateTime, Utc};
use sqlx_postgres::PgPoolOptions;

fn id<T: std::str::FromStr>(value: u128) -> T
where
    T::Err: std::fmt::Debug,
{
    uuid::Uuid::from_u128(value).to_string().parse().unwrap()
}

fn revision(value: u64) -> RevisionNumber {
    value.try_into().unwrap()
}

fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: id(1),
        workspace_id: id(2),
    }
}

fn input(value: u128) -> ArtifactRef {
    ArtifactRef {
        workspace: scope(),
        artifact_id: id(3),
        revision_id: id(value),
        sha256: "a".repeat(64).parse().unwrap(),
    }
}

fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(20, 0).unwrap()
}

fn snapshot() -> TaskSnapshot {
    let package = VersionedName {
        name: "local.documents".parse().unwrap(),
        version: revision(1),
    };
    TaskSnapshot {
        schema_version: CoreSchemaVersion,
        reference: TaskRef {
            workspace: scope(),
            task_id: id(4),
        },
        owner: PrincipalRef {
            authority_id: id(1),
            principal_id: id(5),
        },
        title: "Draft content".into(),
        definition: Some(TaskDefinition {
            schema_version: CoreSchemaVersion,
            reference: TaskDefinitionRef {
                package: package.clone(),
                name: "local.documents.write".parse().unwrap(),
                version: revision(1),
            },
            title: "Document".into(),
            summary: "A frozen definition".into(),
            evidence_schema: VersionedName {
                name: "local.documents.content".parse().unwrap(),
                version: revision(1),
            },
            assessment_policy: VersionedName {
                name: "local.documents.check".parse().unwrap(),
                version: revision(1),
            },
        }),
        input_refs: vec![input(6)],
        status: TaskStatus::Draft,
        revision: revision(1),
        created_at: DateTime::from_timestamp(10, 0).unwrap(),
        updated_at: DateTime::from_timestamp(10, 0).unwrap(),
    }
}

#[test]
fn replacement_changes_only_inputs_revision_and_update_time() {
    let original = snapshot();
    let mut expected = original.clone();
    expected.input_refs = vec![input(7), input(8)];
    expected.revision = revision(2);
    expected.updated_at = now();
    assert_eq!(
        prepare_replacement(&original, revision(1), vec![input(7), input(8)], now()),
        Ok(expected.clone())
    );
    assert_eq!(original, snapshot());
    expected.definition = None;
    let mut manual = original;
    manual.definition = None;
    assert_eq!(
        prepare_replacement(&manual, revision(1), vec![input(7), input(8)], now()),
        Ok(expected)
    );
}

#[test]
fn replacement_rejects_stale_revisions_before_noop_or_status_checks() {
    let mut current = snapshot();
    current.revision = revision(2);
    assert_eq!(
        prepare_replacement(&current, revision(1), current.input_refs.clone(), now()),
        Err(TaskStoreError::StaleRevision)
    );
    current.status = TaskStatus::Cancelled;
    assert_eq!(
        prepare_replacement(&current, revision(1), vec![input(7)], now()),
        Err(TaskStoreError::StaleRevision)
    );
}

#[test]
fn replacement_is_draft_only_and_cannot_reopen_or_cancel_work() {
    for status in [
        TaskStatus::Ready,
        TaskStatus::InProgress,
        TaskStatus::Blocked,
        TaskStatus::InReview,
        TaskStatus::Cancelled,
    ] {
        let mut current = snapshot();
        current.status = status;
        current.revision = revision(2);
        assert_eq!(
            prepare_replacement(&current, revision(2), vec![input(7)], now()),
            Err(TaskStoreError::InvalidTransition)
        );
    }
}

#[test]
fn replacement_rejects_exact_noop_but_preserves_new_reference_order() {
    let mut current = snapshot();
    current.input_refs = vec![input(6), input(7)];
    assert_eq!(
        prepare_replacement(&current, revision(1), current.input_refs.clone(), now()),
        Err(TaskStoreError::InvalidInput)
    );
    let swapped =
        prepare_replacement(&current, revision(1), vec![input(7), input(6)], now()).unwrap();
    assert_eq!(swapped.input_refs, vec![input(7), input(6)]);
}

#[test]
fn replacement_reuses_bounded_content_validation_and_rejects_duplicate_coordinates() {
    let current = snapshot();
    let distinct: Vec<_> = (100..132).map(input).collect();
    assert_eq!(
        prepare_replacement(&current, revision(1), distinct, now())
            .unwrap()
            .input_refs
            .len(),
        32
    );
    assert_eq!(
        prepare_replacement(
            &current,
            revision(1),
            (100..133).map(input).collect(),
            now()
        ),
        Err(TaskStoreError::InvalidInput)
    );
    let mut conflicting = input(7);
    conflicting.sha256 = "b".repeat(64).parse().unwrap();
    for inputs in [vec![input(7), input(7)], vec![input(7), conflicting]] {
        assert_eq!(
            prepare_replacement(&current, revision(1), inputs, now()),
            Err(TaskStoreError::InvalidInput)
        );
    }
    // Empty is valid storage content; the owning Session adapter enforces its manual/catalogue policy.
    assert!(prepare_replacement(&current, revision(1), vec![], now())
        .unwrap()
        .input_refs
        .is_empty());
}

#[test]
fn replacement_rejects_foreign_workspace_or_authority_inputs() {
    for foreign in [
        WorkspaceRef {
            workspace_id: id(44),
            ..scope()
        },
        WorkspaceRef {
            authority_id: id(45),
            ..scope()
        },
    ] {
        let mut reference = input(7);
        reference.workspace = foreign;
        assert_eq!(
            prepare_replacement(&snapshot(), revision(1), vec![reference], now()),
            Err(TaskStoreError::ScopeMismatch)
        );
    }
}

#[test]
fn replacement_revalidates_original_title_and_definition() {
    for title in ["".into(), " ".into(), "x".repeat(257), "control\n".into()] {
        let mut current = snapshot();
        current.title = title;
        assert_eq!(
            prepare_replacement(&current, revision(1), vec![input(7)], now()),
            Err(TaskStoreError::InvalidInput)
        );
    }
    let mut current = snapshot();
    current.definition.as_mut().unwrap().title.clear();
    assert_eq!(
        prepare_replacement(&current, revision(1), vec![input(7)], now()),
        Err(TaskStoreError::InvalidInput)
    );
}

#[test]
fn replacement_revision_overflow_is_rejected_without_wrapping() {
    let mut current = snapshot();
    current.revision = revision(9_007_199_254_740_991);
    assert_eq!(
        prepare_replacement(&current, current.revision, vec![input(7)], now()),
        Err(TaskStoreError::InvalidRecord)
    );
}

#[tokio::test]
async fn revised_drafts_validate_and_only_draft_revision_one_is_a_creation() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    let store = TaskStore::new(pool, scope());
    let mut current = snapshot();
    assert_eq!(store.validate_snapshot(&current), Ok(()));
    assert_eq!(records::event_type(&current), "cyanrex.task.created");
    current.revision = revision(2);
    assert_eq!(store.validate_snapshot(&current), Ok(()));
    assert_eq!(
        records::event_type(&current),
        "cyanrex.task.inputs_replaced"
    );
    for status in [
        TaskStatus::Ready,
        TaskStatus::InProgress,
        TaskStatus::Blocked,
        TaskStatus::InReview,
        TaskStatus::Cancelled,
    ] {
        current.status = status;
        assert_eq!(store.validate_snapshot(&current), Ok(()));
        assert_eq!(records::event_type(&current), "cyanrex.task.status_changed");
        current.revision = revision(1);
        assert_eq!(
            store.validate_snapshot(&current),
            Err(TaskStoreError::InvalidRecord)
        );
        current.revision = revision(2);
    }
}

#[test]
fn replacement_requires_fresh_task_storage_v2_without_changing_core_contract() {
    assert_eq!(TASK_STORAGE_VERSION, 2);
    assert_eq!(u8::from(CoreSchemaVersion), 1);
}
