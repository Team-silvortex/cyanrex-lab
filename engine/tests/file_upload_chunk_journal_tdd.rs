#![cfg(unix)]
#[path = "file_upload_chunk_journal/mod.rs"]
mod cases;

mod baseline {
    use super::cases::fixture::Fixture;

    #[tokio::test]
    #[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
    async fn postgres_fresh_chunk_journal_install_is_acknowledged() {
        let fixture = Fixture::new(4, 2, 16).await;
        let result = fixture.journal.install_empty_namespace().await;
        fixture.cleanup().await;
        result.expect("a fresh explicitly selected chunk journal must confirm its installation");
    }
}
