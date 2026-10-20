use super::*;

fn id<T: std::str::FromStr>(value: u128) -> T
where
    T::Err: std::fmt::Debug,
{
    uuid::Uuid::from_u128(value).to_string().parse().unwrap()
}

fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: id(1),
        workspace_id: id(2),
    }
}

fn reference() -> TaskRef {
    TaskRef {
        workspace: scope(),
        task_id: id(3),
    }
}

fn owner() -> PrincipalRef {
    PrincipalRef {
        authority_id: id(1),
        principal_id: id(4),
    }
}

#[test]
fn content_replace_preflight_accepts_empty_manifest_without_sql() {
    let manifest = TaskContentManifest::new("Revised lesson", vec![]).unwrap();
    assert_eq!(
        check_replace_input(scope(), reference(), owner(), &manifest),
        Ok(())
    );
}

#[test]
fn content_replace_preflight_refuses_foreign_reference_and_owner_before_sql() {
    let manifest = TaskContentManifest::new("Empty", vec![]).unwrap();
    let mut foreign_reference = reference();
    foreign_reference.workspace.workspace_id = id(8);
    assert_eq!(
        check_replace_input(scope(), foreign_reference, owner(), &manifest),
        Err(TaskStoreError::ScopeMismatch)
    );
    let mut foreign_owner = owner();
    foreign_owner.authority_id = id(8);
    assert_eq!(
        check_replace_input(scope(), reference(), foreign_owner, &manifest),
        Err(TaskStoreError::ScopeMismatch)
    );
}

#[test]
fn content_replace_preflight_refuses_foreign_payload_workspace_before_sql() {
    let mut foreign = scope();
    foreign.workspace_id = id(8);
    let manifest = TaskContentManifest::new(
        "Foreign",
        vec![TextPayloadBinding::new(
            ArtifactRef {
                workspace: foreign,
                artifact_id: id(5),
                revision_id: id(6),
                sha256: "a".repeat(64).parse().unwrap(),
            },
            "lesson.txt",
            "plaintext",
        )
        .unwrap()],
    )
    .unwrap();
    assert_eq!(
        check_replace_input(scope(), reference(), owner(), &manifest),
        Err(TaskStoreError::ScopeMismatch)
    );
}
