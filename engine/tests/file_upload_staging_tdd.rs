#![cfg(unix)]
#[path = "file_upload_staging/mod.rs"]
mod cases;

mod baseline {
    use super::cases::bytes::Pattern;
    use super::cases::fixture::*;
    use cyanrex_engine::services::file_upload_staging::UploadChunkState;

    #[tokio::test]
    #[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
    async fn postgres_upload_bound_chunk_has_exact_local_receipt() {
        let fixture = Fixture::ready(17).await;
        let expected = fixture.manifest.clone();
        let mut attempt = fixture
            .adapter
            .prepare(upload(1), owner(1), account(1), 0)
            .await
            .unwrap();
        assert_eq!(attempt.state(), UploadChunkState::Ready);
        let result = fixture
            .adapter
            .stage_chunk(&mut attempt, Pattern::new(0, 17))
            .await;
        let state = attempt.state();
        fixture.cleanup().await;
        let receipt =
            result.expect("valid upload-bound bytes must receive a local acknowledgement");
        assert_eq!(state, UploadChunkState::Confirmed);
        assert_eq!(receipt.upload_id(), upload(1));
        assert_eq!(receipt.owner(), owner(1));
        assert_eq!(receipt.account_id(), account(1));
        assert_eq!(receipt.chunk_index(), 0);
        assert_eq!(receipt.manifest(), &expected);
    }
}
