use super::sql_fixture::*;
use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_publication_packages_versions_bindings_and_ordered_text_confirm_one_write_at_a_time(
) {
    let f = Fixture::ready().await;
    let entries = values();
    for (index, definition) in entries.iter().enumerate() {
        let items: Vec<_> = if index == 1 {
            vec![]
        } else {
            vec![
                ("同名.txt", "future.language", "文档\r\n\t😀"),
                ("同名.txt", "plaintext", "文档\r\n\t😀"),
                ("empty.rs", "rust", ""),
            ]
        };
        let mut attempt = prepare(&f, definition, &items);
        assert_eq!(attempt.definition(), definition);
        let allocated = attempt.allocated_artifacts().to_vec();
        let before = counts(&f).await;
        for step in 0..items.len() {
            assert_eq!(
                attempt.advance(&f.auth.learner_token).await.unwrap(),
                SessionDraftPublicationProgress::ArtifactConfirmed { index: step }
            );
            assert_eq!(attempt.state(), &SessionDraftPublicationState::Ready);
            assert!(attempt.confirmed_task().is_none());
            let n = step as i64 + 1;
            assert_eq!(
                counts(&f).await,
                [
                    before[0] + n,
                    before[1] + n,
                    before[2] + n,
                    before[3],
                    before[4],
                    before[5] + n
                ]
            );
        }
        let artifacts = attempt.confirmed_artifacts().to_vec();
        assert_eq!(
            attempt.advance(&f.auth.learner_token).await.unwrap(),
            SessionDraftPublicationProgress::TaskConfirmed
        );
        assert_eq!(attempt.state(), &SessionDraftPublicationState::Complete);
        let content = read(&f, &attempt).await;
        assert_eq!(attempt.confirmed_task(), Some(&content.snapshot));
        assert_eq!(content.snapshot.task.definition, Some(definition.clone()));
        assert_eq!(
            content.snapshot.task.owner,
            f.auth.learner.principal.reference
        );
        assert_eq!(content.snapshot.task.status, TaskStatus::Draft);
        assert_eq!(u64::from(content.snapshot.task.revision), 1);
        assert_eq!(content.snapshot.task.input_refs, allocated);
        for (i, (filename, language, text)) in items.iter().enumerate() {
            let binding = &content.snapshot.manifest.payload()[i];
            assert_eq!(binding.filename(), *filename);
            assert_eq!(binding.language(), *language);
            assert_eq!(content.contents[i].revision, artifacts[i]);
            assert_eq!(content.contents[i].bytes, text.as_bytes());
        }
        if !items.is_empty() {
            assert_eq!(allocated[0].sha256, allocated[1].sha256);
            assert_ne!(allocated[0].artifact_id, allocated[1].artifact_id);
            assert_ne!(allocated[0].revision_id, allocated[1].revision_id);
        }
        let n = items.len() as i64;
        assert_eq!(
            counts(&f).await,
            [
                before[0] + n,
                before[1] + n,
                before[2] + n,
                before[3] + 1,
                before[4] + 1,
                before[5] + n
            ]
        );
        assert!(
            matches!(
                f.get(&f.auth.learner_token, attempt.task_ref().task_id)
                    .await,
                Err(SessionTaskError::Task(_))
            ),
            "manual reader cannot adopt catalogue work"
        );
        let committed = observed(&f).await;
        assert_eq!(
            attempt.advance(&f.auth.learner_token).await,
            Err(SessionDraftPublicationError::Stopped)
        );
        assert_eq!(observed(&f).await, committed);
        drop(attempt);
    }
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_publication_unknown_coordinates_and_session_substitution_never_dispatch()
{
    let f = Fixture::ready().await;
    let configured = workspace(&f);
    let entry = values()[0].clone();
    let before = observed(&f).await;
    for coordinate in 0..4 {
        let mut reference = entry.reference.clone();
        match coordinate {
            0 => reference.package.name = "sample.unknown".parse().unwrap(),
            1 => reference.package.version = 9.try_into().unwrap(),
            2 => reference.name = "sample.document.missing".parse().unwrap(),
            3 => reference.version = 9.try_into().unwrap(),
            _ => unreachable!(),
        }
        assert!(matches!(
            f.auth.source.prepare_catalog_task_draft_publication(
                &f.auth.learner_token,
                configured.clone(),
                &reference,
                manual_fixture::plan(&[])
            ),
            Err(SessionDraftPublicationError::InvalidPreparation)
        ));
        assert_eq!(observed(&f).await, before);
    }
    for items in [vec![], vec![("a.txt", "plaintext", "Private bytes")]] {
        let before = observed(&f).await;
        let mut attempt = prepare(&f, &entry, &items);
        drop(attempt.advance(&f.auth.learner_token));
        for token in [&f.auth.manager_token, &"not-a-session".to_string()] {
            assert_eq!(
                attempt.advance(token).await,
                Err(SessionDraftPublicationError::SessionChanged)
            );
        }
        assert_eq!(attempt.state(), &SessionDraftPublicationState::Ready);
        assert_eq!(observed(&f).await, before);
        let result = attempt.advance(&f.auth.learner_token).await.unwrap();
        assert_eq!(
            result,
            if items.is_empty() {
                SessionDraftPublicationProgress::TaskConfirmed
            } else {
                SessionDraftPublicationProgress::ArtifactConfirmed { index: 0 }
            }
        );
        if !items.is_empty() {
            attempt.advance(&f.auth.learner_token).await.unwrap();
        }
        read(&f, &attempt).await;
        drop(attempt);
    }
    f.assert_pool_restored().await;
    drop(configured);
    f.cleanup().await;
}
