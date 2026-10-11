use super::*;

#[tokio::test]
async fn catalog_observation_unknown_artifact_calls_existing_reader_and_preserves_typed_error() {
    let f = Offline::new().await;
    let mut attempt = f
        .source
        .prepare_catalog_task_draft_publication(
            TOKEN,
            f.workspace.clone(),
            &definition().reference,
            offline_fixture::plan(&["exact private bytes"]),
        )
        .unwrap();
    let failed = attempt.advance(TOKEN).await.unwrap_err();
    assert!(
        matches!(failed, SessionDraftPublicationError::Artifact(_)),
        "the real closed-pool dispatch must mark an Artifact unknown"
    );
    let state = attempt.state().clone();
    assert!(matches!(
        state,
        SessionDraftPublicationState::Unconfirmed(_)
    ));
    let frozen = attempt.definition().clone();
    assert_eq!(
        attempt.observe_unconfirmed(TOKEN).await,
        Err(failed),
        "the current reader error is not replaced with NoUnconfirmedStep"
    );
    assert_eq!(attempt.state(), &state);
    assert_eq!(attempt.definition(), &frozen);
    assert!(attempt.confirmed_artifacts().is_empty());
    assert!(attempt.confirmed_task().is_none());
    assert_eq!(
        attempt.advance(TOKEN).await,
        Err(SessionDraftPublicationError::Stopped)
    );
    assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);
}

#[tokio::test]
async fn catalog_observation_unknown_empty_task_calls_catalog_reader_and_keeps_definition() {
    let f = Offline::new().await;
    let mut attempt = f
        .source
        .prepare_catalog_task_draft_publication(
            TOKEN,
            f.workspace.clone(),
            &definition().reference,
            offline_fixture::plan(&[]),
        )
        .unwrap();
    let failed = attempt.advance(TOKEN).await.unwrap_err();
    assert!(matches!(failed, SessionDraftPublicationError::Task(_)));
    let saved = fixture::progress(&attempt);
    assert_eq!(attempt.observe_unconfirmed(TOKEN).await, Err(failed));
    assert_eq!(fixture::progress(&attempt), saved);
    assert_eq!(
        attempt.advance(TOKEN).await,
        Err(SessionDraftPublicationError::Stopped)
    );
    assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);
}

#[tokio::test]
async fn catalog_observation_state_and_original_token_preflight_do_not_dispatch_or_rebind() {
    let f = Offline::new().await;
    let mut attempt = f
        .source
        .prepare_catalog_task_draft_publication(
            TOKEN,
            f.workspace.clone(),
            &definition().reference,
            offline_fixture::plan(&["private"]),
        )
        .unwrap();
    let ready = fixture::progress(&attempt);
    for token in [TOKEN, "invalid", "4d20c6c1-e5f8-48a1-9655-e8fb82ad399c"] {
        assert_eq!(
            attempt.observe_unconfirmed(token).await,
            Err(SessionDraftPublicationError::NoUnconfirmedStep)
        );
    }
    assert_eq!(fixture::progress(&attempt), ready);
    assert!(attempt.advance(TOKEN).await.is_err());
    let unknown = fixture::progress(&attempt);
    for token in [
        "",
        "invalid",
        "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
        "4d20c6c1-e5f8-48a1-9655-e8fb82ad399c",
    ] {
        assert_eq!(
            attempt.observe_unconfirmed(token).await,
            Err(SessionDraftPublicationError::SessionChanged)
        );
    }
    assert_eq!(fixture::progress(&attempt), unknown);
    assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);
}

#[tokio::test]
async fn catalog_observation_unpolled_read_retains_complete_unknown_target_and_all_receipts() {
    let f = Offline::new().await;
    for texts in [vec![], vec!["private"]] {
        let mut attempt = f
            .source
            .prepare_catalog_task_draft_publication(
                TOKEN,
                f.workspace.clone(),
                &definition().reference,
                offline_fixture::plan(&texts),
            )
            .unwrap();
        assert!(attempt.advance(TOKEN).await.is_err());
        let saved = fixture::progress(&attempt);
        drop(attempt.observe_unconfirmed(TOKEN));
        assert_eq!(fixture::progress(&attempt), saved);
        assert_eq!(
            attempt.advance(TOKEN).await,
            Err(SessionDraftPublicationError::Stopped)
        );
    }
    assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);
}
