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

fn owner() -> PrincipalRef {
    PrincipalRef {
        authority_id: id(1),
        principal_id: id(3),
    }
}

#[test]
fn content_create_preflight_accepts_empty_manual_content_without_sql() {
    let manifest = TaskContentManifest::new("Collaborative lesson", vec![]).unwrap();
    assert_eq!(
        check_create_input(scope(), id(4), owner(), &manifest),
        Ok(())
    );
}

#[test]
fn content_create_preflight_refuses_foreign_authority_before_sql() {
    let mut owner = owner();
    owner.authority_id = id(8);
    let manifest = TaskContentManifest::new("Empty", vec![]).unwrap();
    assert_eq!(
        check_create_input(scope(), id(4), owner, &manifest),
        Err(TaskStoreError::ScopeMismatch)
    );
}

#[test]
fn content_create_preflight_refuses_foreign_payload_workspace_before_sql() {
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
            "handout.txt",
            "plaintext",
        )
        .unwrap()],
    )
    .unwrap();
    assert_eq!(
        check_create_input(scope(), id(4), owner(), &manifest),
        Err(TaskStoreError::ScopeMismatch)
    );
}
