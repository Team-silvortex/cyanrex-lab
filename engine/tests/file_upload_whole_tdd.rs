#![cfg(unix)]
#[path = "file_upload_whole/mod.rs"]
mod cases;

mod baseline {
    use super::cases::{bytes::*, fixture::*};

    #[tokio::test]
    #[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
    async fn postgres_whole_upload_observation_verifies_real_bound_bytes() {
        let fixture = Fixture::ready(17).await;
        let mut attempt = fixture
            .adapter
            .prepare(upload(1), owner(1), account(1), 0)
            .await
            .unwrap();
        let receipt = fixture
            .adapter
            .stage_chunk(&mut attempt, Pattern::new(0, 17))
            .await
            .unwrap();
        drop(receipt);
        let rows = fixture.rows().await;
        let files = tree(&fixture.root);
        let result = fixture
            .adapter
            .verify_file(upload(1), owner(1), account(1))
            .await;
        let after_rows = fixture.rows().await;
        let after_files = tree(&fixture.root);
        let expected = fixture.manifest.clone();
        fixture.cleanup().await;
        let observed = result.expect("a whole upload observation must inspect real bound bytes");
        assert_eq!(observed.upload_id(), upload(1));
        assert_eq!(observed.owner(), owner(1));
        assert_eq!(observed.account_id(), account(1));
        assert_eq!(observed.manifest(), &expected);
        assert_eq!(after_rows, rows);
        assert_eq!(after_files, files);
    }
}
