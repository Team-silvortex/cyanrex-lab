use super::*;

#[test]
fn binding_preserves_order_labels_and_exact_supplied_refs_without_changing_snapshots() {
    let mut value = draft(vec![item(0, "中文\t\r\n"), item(1, "")]);
    value["payload"][0]["filename"] = json!("custom.rs");
    value["payload"][0]["language"] = json!("future-lang");
    let plan = parse(&value).ok().unwrap();
    let contents = vec![content(1, "中文\t\r\n".as_bytes()), content(2, b"")];
    let before = contents.clone();
    let manifest = plan.bind_published(scope(), owner(), &contents).unwrap();
    assert_eq!(manifest.title(), plan.title());
    assert_eq!(manifest.payload()[0].filename(), "custom.rs");
    assert_eq!(manifest.payload()[0].language(), "future-lang");
    assert_eq!(
        manifest.input_refs(),
        contents
            .iter()
            .map(|c| c.revision.reference.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(contents, before);
    let bytes = serde_json::to_vec(&manifest).unwrap();
    assert!(bytes.len() <= MAX_TASK_CONTENT_MANIFEST_BYTES);
    assert_eq!(TaskContentManifest::parse_json(&bytes).unwrap(), manifest);
}

#[test]
fn binding_requires_every_item_and_rejects_partial_extra_or_different_byte_order() {
    let plan = plan(&["first", "second"]);
    let first = content(1, b"first");
    let second = content(2, b"second");
    for contents in [
        vec![],
        vec![first.clone()],
        vec![first.clone(), second.clone(), content(3, b"extra")],
        vec![second, first],
    ] {
        assert!(plan.bind_published(scope(), owner(), &contents).is_err());
    }
    // Even a self-consistent replacement with its own correct digest is not this plan's text.
    assert!(plan
        .bind_published(
            scope(),
            owner(),
            &[content(1, b"other"), content(2, b"second")]
        )
        .is_err());
}

#[test]
fn binding_checks_owner_and_workspace_even_when_no_items_exist() {
    let empty = plan(&[]);
    let foreign_authority = PrincipalRef {
        authority_id: id(8),
        ..owner()
    };
    assert!(empty
        .bind_published(scope(), foreign_authority, &[])
        .is_err());
    let plan = plan(&["text"]);
    let original = content(1, b"text");
    let mut changed = vec![original.clone(); 4];
    changed[0].revision.reference.workspace.authority_id = id(8);
    changed[1].revision.reference.workspace.workspace_id = id(8);
    changed[2].revision.owner.authority_id = id(8);
    changed[3].revision.owner.principal_id = id(8);
    for content in changed {
        assert!(plan.bind_published(scope(), owner(), &[content]).is_err());
    }
    assert!(plan
        .bind_published(scope(), foreign_authority, &[original])
        .is_err());
}

#[test]
fn binding_recomputes_digest_and_checks_declared_length_not_just_equal_plan_bytes() {
    let plan = plan(&["text"]);
    let original = content(1, b"text");
    let mut forged = original.clone();
    forged.revision.reference.sha256 = "0".repeat(64).parse().unwrap();
    assert!(plan.bind_published(scope(), owner(), &[forged]).is_err());
    for length in [0, 3, 5, u64::MAX] {
        let mut changed = original.clone();
        changed.revision.byte_length = length;
        assert!(plan.bind_published(scope(), owner(), &[changed]).is_err());
    }
}

#[test]
fn same_bytes_do_not_merge_independent_refs_and_same_artifact_distinct_revisions_are_valid() {
    let plan = plan(&["same", "same"]);
    let first = content(1, b"same");
    let mut second = content(2, b"same");
    let independent = plan
        .bind_published(scope(), owner(), &[first.clone(), second.clone()])
        .unwrap();
    assert_eq!(independent.payload().len(), 2);
    assert_ne!(
        independent.payload()[0].artifact(),
        independent.payload()[1].artifact()
    );
    // IDs are chosen by the caller; equal bytes do not prove any earlier publication order.
    let swapped = plan
        .bind_published(scope(), owner(), &[second.clone(), first.clone()])
        .unwrap();
    assert_eq!(swapped.payload()[0].artifact(), &second.revision.reference);
    second.revision.reference.artifact_id = first.revision.reference.artifact_id;
    second.revision.parent = Some(first.revision.reference.clone());
    second.revision.sequence = 2.try_into().unwrap();
    assert!(plan
        .bind_published(scope(), owner(), &[first, second])
        .is_ok());
}

#[test]
fn repeated_revision_coordinates_are_rejected_even_with_different_correct_digests() {
    let plan = plan(&["first", "second"]);
    let first = content(1, b"first");
    let mut second = content(2, b"second");
    second.revision.reference.artifact_id = first.revision.reference.artifact_id;
    second.revision.reference.revision_id = first.revision.reference.revision_id;
    assert!(plan
        .bind_published(scope(), owner(), &[first, second])
        .is_err());
    let repeated = content(1, b"same");
    assert!(super::plan(&["same", "same"])
        .bind_published(scope(), owner(), &[repeated.clone(), repeated])
        .is_err());
}

#[test]
fn local_id_and_revision_are_discarded_and_never_become_artifact_identity_or_task_revision() {
    let mut value = draft(vec![item(0, "text")]);
    value["payload"][0]["id"] = json!("00000000-0000-0000-0000-000000000abc");
    value["payload"][0]["revision"] = json!(9_007_199_254_740_991_u64);
    let plan = parse(&value).ok().unwrap();
    let supplied = content(9, b"text");
    let manifest = plan
        .bind_published(scope(), owner(), &[supplied.clone()])
        .unwrap();
    assert_eq!(
        manifest.payload()[0].artifact(),
        &supplied.revision.reference
    );
    let serialized = serde_json::to_value(manifest).unwrap();
    assert!(serialized["payload"][0].get("id").is_none());
    assert!(serialized["payload"][0].get("revision").is_none());
    assert!(serialized.get("expected_revision").is_none());
}

#[test]
#[cfg(unix)]
fn publication_draft_uses_explicit_task_title_not_a_filename_and_bounds_index() {
    for filename in ["中".repeat(128), " ".into()] {
        let mut value = draft(vec![item(0, "text")]);
        value["payload"][0]["filename"] = json!(filename);
        let plan = parse(&value).ok().unwrap();
        assert!(plan.publication_draft(0).is_ok());
        assert!(plan.publication_draft(1).is_err());
        assert!(plan.publication_draft(usize::MAX).is_err());
    }
    assert!(plan(&[]).publication_draft(0).is_err());
}
