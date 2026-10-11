use super::*;

#[tokio::test]
async fn catalog_draft_prepare_freezes_unbound_definition_without_io_and_starts_ready() {
    let f = Offline::new().await;
    let definition = definition();
    let attempt = f
        .source
        .prepare_catalog_task_draft_publication(
            TOKEN,
            f.workspace.clone(),
            &definition.reference,
            plan(&[]),
        )
        .expect("a known frozen catalogue definition must prepare without storage");
    assert_eq!(attempt.state(), &SessionDraftPublicationState::Ready);
    assert_eq!(attempt.definition(), &definition);
    assert!(attempt.confirmed_artifacts().is_empty());
    assert!(attempt.confirmed_task().is_none());
    assert!(attempt.allocated_artifacts().is_empty());
    assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);
}

#[tokio::test]
async fn catalog_draft_preparation_rejects_changed_coordinates_token_and_source_without_io() {
    let f = Offline::new().await;
    let known = definition();
    for coordinate in 0..4 {
        let mut reference = known.reference.clone();
        match coordinate {
            0 => reference.package.name = "sample.unknown".parse().unwrap(),
            1 => reference.package.version = 9.try_into().unwrap(),
            2 => reference.name = "sample.document.missing".parse().unwrap(),
            3 => reference.version = 9.try_into().unwrap(),
            _ => unreachable!(),
        }
        assert_ne!(reference, known.reference);
        assert!(matches!(
            f.source.prepare_catalog_task_draft_publication(
                TOKEN,
                f.workspace.clone(),
                &reference,
                plan(&[])
            ),
            Err(SessionDraftPublicationError::InvalidPreparation)
        ));
    }
    for token in [
        "",
        "not-a-token",
        "00000000-0000-0000-0000-000000000000",
        "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
    ] {
        assert!(matches!(
            f.source.prepare_catalog_task_draft_publication(
                token,
                f.workspace.clone(),
                &known.reference,
                plan(&[])
            ),
            Err(SessionDraftPublicationError::InvalidPreparation)
        ));
    }
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    let foreign = DurableAuthSource::new(pool, Uuid::new_v4().try_into().unwrap());
    assert!(matches!(
        foreign.prepare_catalog_task_draft_publication(
            TOKEN,
            f.workspace.clone(),
            &known.reference,
            plan(&[])
        ),
        Err(SessionDraftPublicationError::InvalidPreparation)
    ));
    assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);
}

#[tokio::test]
async fn catalog_draft_allocates_complete_distinct_random_targets_and_retains_frozen_metadata() {
    let f = Offline::new().await;
    let mut original = definition();
    let texts = ["identical"; 32];
    let first = f
        .source
        .prepare_catalog_task_draft_publication(
            TOKEN,
            f.workspace.clone(),
            &original.reference,
            plan(&texts),
        )
        .unwrap();
    let second = f
        .source
        .prepare_catalog_task_draft_publication(
            TOKEN,
            f.workspace.clone(),
            &original.reference,
            plan(&texts),
        )
        .unwrap();
    let expected = original.clone();
    original.title = "Caller changed local metadata".into();
    original.reference.version = 4.try_into().unwrap();
    assert_eq!(first.definition(), &expected);
    assert_ne!(first.definition(), &original);
    assert_eq!(first.allocated_artifacts().len(), 32);
    assert_ne!(first.task_ref(), second.task_ref());
    let mut ids = std::collections::HashSet::new();
    for reference in first
        .allocated_artifacts()
        .iter()
        .chain(second.allocated_artifacts())
    {
        assert!(ids.insert(reference.artifact_id.as_uuid()));
        assert!(ids.insert(reference.revision_id.as_uuid()));
        assert_eq!(reference.workspace, first.task_ref().workspace);
        assert_eq!(
            reference.artifact_id.as_uuid().get_version(),
            Some(uuid::Version::Random)
        );
        assert_eq!(
            reference.revision_id.as_uuid().get_version(),
            Some(uuid::Version::Random)
        );
        assert_eq!(reference.sha256, first.allocated_artifacts()[0].sha256);
    }
    assert_eq!(
        first.task_ref().task_id.as_uuid().get_version(),
        Some(uuid::Version::Random)
    );
    assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);
}

#[tokio::test]
async fn catalog_draft_wrong_session_and_unpolled_future_preserve_ready_then_dispatched_error_stops(
) {
    let f = Offline::new().await;
    for texts in [vec![], vec!["text"]] {
        let mut attempt = f
            .source
            .prepare_catalog_task_draft_publication(
                TOKEN,
                f.workspace.clone(),
                &definition().reference,
                plan(&texts),
            )
            .unwrap();
        drop(attempt.advance(TOKEN));
        for token in ["invalid", "4d20c6c1-e5f8-48a1-9655-e8fb82ad399c"] {
            assert_eq!(
                attempt.advance(token).await,
                Err(SessionDraftPublicationError::SessionChanged)
            );
        }
        assert_eq!(attempt.state(), &SessionDraftPublicationState::Ready);
        assert!(attempt.confirmed_artifacts().is_empty());
        assert!(attempt.advance(TOKEN).await.is_err());
        let expected = if texts.is_empty() {
            SessionDraftPublicationStep::Task {
                reference: attempt.task_ref(),
            }
        } else {
            SessionDraftPublicationStep::Artifact {
                index: 0,
                reference: attempt.allocated_artifacts()[0].clone(),
            }
        };
        assert_eq!(
            attempt.state(),
            &SessionDraftPublicationState::Unconfirmed(expected)
        );
        assert_eq!(
            attempt.advance(TOKEN).await,
            Err(SessionDraftPublicationError::Stopped)
        );
        assert!(attempt.confirmed_task().is_none());
    }
    assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);
}
