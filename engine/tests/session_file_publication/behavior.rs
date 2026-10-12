use super::{fixture::*, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_file_publication_observes_real_cross_chunk_and_empty_bytes_with_one_shot_issuer_state(
) {
    for length in [FILE_TRANSFER_CHUNK_BYTES + 17, 0] {
        let f = Fixture::ready(length).await;
        f.grants.issue(1, f.actor()).await;
        let mut a = prepare(&f, 1).await;
        let before = f.snapshot().await;
        for token in [
            "",
            "bad",
            "00000000-0000-0000-0000-000000000000",
            "ABCDEF01-0000-4000-8000-000000000001",
        ] {
            let result = publish_as(&f, token, &f.workspace, 1, &mut a).await;
            assert_eq!(
                result.err(),
                Some(session_error(SessionCommandError::InvalidSession))
            );
            assert_eq!(a.state(), FileUploadPublicationState::Ready);
            assert_eq!(f.snapshot().await, before);
        }
        let foreign =
            FileUploadPublisher::new(f.local.clone(), f.journal.clone(), f.publication.clone())
                .unwrap();
        let wrong = f
            .grants
            .auth
            .source
            .file_publishing_workspace(foreign, f.grants.workspace.clone())
            .unwrap();
        let denied = publish_as(&f, &f.grants.auth.learner_token, &wrong, 1, &mut a).await;
        assert_eq!(
            denied.err(),
            Some(publisher_error(FileUploadPublicationError::InvalidInput))
        );
        assert_eq!(a.state(), FileUploadPublicationState::Ready);
        assert_eq!(f.snapshot().await, before);
        let clone = f.workspace.clone();
        let result = publish_as(&f, &f.grants.auth.learner_token, &clone, 1, &mut a).await;
        let state = a.state();
        let after = f.snapshot().await;
        let repeated = publish(&f, &mut a).await;
        let retained = f.snapshot().await;
        let mut fresh = prepare(&f, 1).await;
        let occupied = publish(&f, &mut fresh).await;
        let occupied_state = fresh.state();
        let final_snapshot = f.snapshot().await;
        let observed = f
            .publication
            .get(upload(1), f.actor().principal(), f.actor().account_id())
            .await;
        let actor = f.actor();
        let manifest = f.manifest.clone();
        if result.is_ok() {
            delta(&f, &before, &after, 1);
        }
        f.cleanup().await;
        let result = result.unwrap();
        receipt(&result, 1, actor, &manifest);
        assert_eq!(state, FileUploadPublicationState::Confirmed);
        assert_eq!(
            repeated.err(),
            Some(publisher_error(FileUploadPublicationError::InvalidInput))
        );
        assert_eq!(after, retained);
        assert_eq!(occupied.err(),Some(publisher_error(FileUploadPublicationError::Publication(cyanrex_engine::services::file_upload_publication::FileUploadPublicationStoreError::Conflict))));
        assert_eq!(occupied_state, FileUploadPublicationState::Unknown);
        assert_eq!(final_snapshot, after);
        assert_eq!(observed.unwrap().unwrap().manifest(), &manifest);
    }
}
