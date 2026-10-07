use super::*;

#[tokio::test]
async fn checkpoint_ready_export_round_trips_metadata_without_local_authority() {
    let f = Fixture::ready().await;
    let texts = [SECRET_TEXT, "", SECRET_TEXT];
    let attempt = f.attempt(&texts);
    let refs = attempt.allocated_artifacts().to_vec();
    let checkpoint = attempt.checkpoint().unwrap();
    let encoded = checkpoint.to_json().unwrap();
    let decoded = SessionDraftPublicationCheckpoint::parse_json(&encoded).unwrap();
    assert_eq!(decoded.task_ref(), attempt.task_ref());
    assert_eq!(decoded.manifest().input_refs(), refs);
    assert_eq!(decoded.manifest().title(), TITLE);
    assert_eq!(
        decoded.byte_lengths(),
        &[SECRET_TEXT.len() as u64, 0, SECRET_TEXT.len() as u64]
    );
    assert_eq!(decoded.reported_state(), SessionDraftCheckpointState::Ready);
    assert_eq!(decoded.reported_confirmed_artifact_count(), 0);
    assert_eq!(decoded.reported_unconfirmed_step(), None);
    assert_eq!(decoded.to_json().unwrap(), encoded);
    assert_eq!(attempt.state(), &SessionDraftPublicationState::Ready);
    assert!(attempt.confirmed_artifacts().is_empty());
    assert!(attempt.confirmed_task().is_none());
    assert_eq!(refs[0].sha256, refs[2].sha256);
    assert_ne!(refs[0].artifact_id, refs[2].artifact_id);
    let value: Value = serde_json::from_slice(&encoded).unwrap();
    let mut keys: Vec<_> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "byte_lengths",
            "format",
            "manifest",
            "reported_confirmed_artifact_count",
            "reported_state",
            "task",
            "version"
        ]
    );
    let text = String::from_utf8(encoded).unwrap();
    for forbidden in [
        f.token.as_str(),
        "private-code-sentinel-993c",
        "local_only_",
        "9007199254740991",
        "checkpoint_artifacts",
        "checkpoint_tasks",
    ] {
        assert!(
            !text.contains(forbidden),
            "checkpoint leaked excluded source material"
        );
    }
    assert!(!text.contains(f.root.to_str().unwrap()));
    assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);
}

#[tokio::test]
async fn checkpoint_unconfirmed_export_reports_derived_target_without_receipt_or_recovery() {
    let f = Fixture::ready().await;
    for texts in [vec![], vec![SECRET_TEXT]] {
        let mut attempt = f.attempt(&texts);
        assert!(attempt.advance(&f.token).await.is_err());
        let state = attempt.state().clone();
        let checkpoint = attempt.checkpoint().unwrap();
        let decoded =
            SessionDraftPublicationCheckpoint::parse_json(&checkpoint.to_json().unwrap()).unwrap();
        assert_eq!(
            decoded.reported_state(),
            SessionDraftCheckpointState::Unconfirmed
        );
        assert_eq!(decoded.reported_confirmed_artifact_count(), 0);
        let SessionDraftPublicationState::Unconfirmed(step) = &state else {
            panic!("closed pool must stop after dispatch");
        };
        assert_eq!(decoded.reported_unconfirmed_step(), Some(step.clone()));
        assert_eq!(attempt.state(), &state);
        assert!(attempt.confirmed_artifacts().is_empty());
        assert!(attempt.confirmed_task().is_none());
        assert!(attempt.advance(&f.token).await.is_err());
    }
    assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);
}

#[tokio::test]
async fn checkpoint_maximum_metadata_export_is_bounded_without_inline_text() {
    let f = Fixture::ready().await;
    // One maximum-size body plus 31 empty items fits C2-O's whole-draft budget. Bodies are omitted.
    let large = "x".repeat(MAX_TEXT_PAYLOAD_BYTES);
    let mut texts = vec![""; MAX_TASK_CONTENT_ITEMS];
    texts[0] = &large;
    let filename = "中".repeat(128);
    let language = "x".repeat(64);
    let title = "x".repeat(256);
    let attempt = f.attempt_with_labels(&title, &filename, &language, &texts);
    let checkpoint = attempt.checkpoint().unwrap();
    let encoded = checkpoint.to_json().unwrap();
    assert!(encoded.len() <= MAX_SESSION_DRAFT_CHECKPOINT_BYTES);
    assert!(encoded.len() < large.len());
    let parsed = SessionDraftPublicationCheckpoint::parse_json(&encoded).unwrap();
    assert_eq!(parsed.manifest().payload().len(), MAX_TASK_CONTENT_ITEMS);
    assert_eq!(parsed.byte_lengths()[0], MAX_TEXT_PAYLOAD_BYTES as u64);
    assert_eq!(parsed.manifest().payload()[31].filename(), filename);
    assert_eq!(parsed.manifest().payload()[31].language(), language);
    assert_eq!(parsed.manifest().title(), title);
    assert_eq!(parsed.to_json().unwrap(), encoded);
    assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);
}
