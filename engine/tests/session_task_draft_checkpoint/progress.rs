use super::*;

#[test]
fn checkpoint_progress_counts_derive_only_the_reported_unknown_target() {
    for count in [0, 1, 2] {
        for (label, state) in [
            ("ready", SessionDraftCheckpointState::Ready),
            ("unconfirmed", SessionDraftCheckpointState::Unconfirmed),
            ("complete", SessionDraftCheckpointState::Complete),
        ] {
            let mut value = wire(2);
            value["reported_state"] = json!(label);
            value["reported_confirmed_artifact_count"] = json!(count);
            let parsed = parse(&value);
            if label == "complete" && count != 2 {
                assert!(parsed.is_err());
                continue;
            }
            let parsed = parsed.unwrap();
            assert_eq!(parsed.reported_state(), state);
            assert_eq!(parsed.reported_confirmed_artifact_count(), count);
            let expected = if label != "unconfirmed" {
                None
            } else if count == 2 {
                Some(SessionDraftPublicationStep::Task {
                    reference: parsed.task_ref(),
                })
            } else {
                Some(SessionDraftPublicationStep::Artifact {
                    index: count,
                    reference: parsed.manifest().payload()[count].artifact().clone(),
                })
            };
            assert_eq!(parsed.reported_unconfirmed_step(), expected);
            let again =
                SessionDraftPublicationCheckpoint::parse_json(&parsed.to_json().unwrap()).unwrap();
            assert_eq!(again.reported_unconfirmed_step(), expected);
        }
    }
    for state in ["ready", "unconfirmed", "complete"] {
        let mut value = wire(2);
        value["reported_state"] = json!(state);
        value["reported_confirmed_artifact_count"] = json!(3);
        assert!(parse(&value).is_err());
    }
}

#[test]
fn checkpoint_complete_is_an_untrusted_claim_not_server_confirmation() {
    // These random references were never published. Structural validity is deliberately not a DB receipt.
    let mut value = wire(1);
    value["reported_state"] = json!("complete");
    value["reported_confirmed_artifact_count"] = json!(1);
    let first = parse(&value).unwrap();
    assert_eq!(
        first.reported_state(),
        SessionDraftCheckpointState::Complete
    );
    assert!(first.reported_unconfirmed_step().is_none());
    value["task"]["task_id"] = json!(Uuid::new_v4().to_string());
    value["manifest"]["title"] = json!("A different untrusted description");
    value["manifest"]["payload"][0]["artifact"]["sha256"] = json!("b".repeat(64));
    let edited = parse(&value).unwrap();
    assert_ne!(edited.task_ref(), first.task_ref());
    assert_ne!(edited.manifest(), first.manifest());
    assert_eq!(
        edited.reported_state(),
        SessionDraftCheckpointState::Complete
    );
    // There is no attempt/source/token or command capability returned by either parser call.
    assert!(edited.reported_unconfirmed_step().is_none());
}

#[test]
fn checkpoint_named_empty_task_keeps_scope_without_placeholder_owner() {
    for state in ["ready", "unconfirmed", "complete"] {
        let mut value = wire(0);
        value["reported_state"] = json!(state);
        let parsed = parse(&value).unwrap();
        assert!(parsed.manifest().payload().is_empty());
        assert!(parsed.byte_lengths().is_empty());
        assert_eq!(parsed.reported_confirmed_artifact_count(), 0);
        assert_eq!(
            parsed.task_ref().workspace.authority_id.to_string(),
            "00000000-0000-0000-0000-000000000001"
        );
        assert_eq!(
            parsed.task_ref().workspace.workspace_id.to_string(),
            "00000000-0000-0000-0000-000000000002"
        );
        assert_eq!(
            parsed.reported_unconfirmed_step(),
            if state == "unconfirmed" {
                Some(SessionDraftPublicationStep::Task {
                    reference: parsed.task_ref(),
                })
            } else {
                None
            }
        );
        let output: Value = serde_json::from_slice(&parsed.to_json().unwrap()).unwrap();
        assert!(output.get("owner").is_none());
        assert_eq!(output["task"]["workspace"], value["task"]["workspace"]);
    }
    let mut invalid = wire(0);
    invalid["reported_confirmed_artifact_count"] = json!(1);
    assert!(parse(&invalid).is_err());
}
