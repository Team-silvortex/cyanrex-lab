use super::{bytes::*, fixture::*, support::*};
use cyanrex_engine::{
    models::collaboration::FILE_TRANSFER_CHUNK_BYTES,
    services::{file_upload_staging::FileUploadStaging, file_upload_store::FileUploadStore},
};
use sqlx_core::{query::query, row::Row};
use std::{fs, os::unix::fs::PermissionsExt};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_cross_chunk_empty_and_readonly_observations_are_exact_nonmutating() {
    for length in [0, FILE_TRANSFER_CHUNK_BYTES + 17] {
        let fixture = Fixture::ready(length).await;
        stage_all(&fixture).await;
        let files = tree(&fixture.root);
        assert_eq!(files.len(), fixture.manifest.chunk_sha256().len());
        good(&fixture, &fixture.adapter).await;
        good(&fixture, &fixture.adapter.clone()).await;
        if length == 0 {
            fs::set_permissions(&fixture.root, fs::Permissions::from_mode(0o770)).unwrap();
            let unsafe_root = snapshot(&fixture.root);
            bad(&fixture,cyanrex_engine::services::file_upload_staging::UploadFileVerificationError::Unavailable).await;
            assert_eq!(snapshot(&fixture.root), unsafe_root);
            fs::set_permissions(&fixture.root, fs::Permissions::from_mode(0o700)).unwrap();
            good(&fixture, &fixture.adapter).await;
        }
        let url = std::env::var("CYANREX_TEST_DATABASE_URL").unwrap();
        let pool = pool_for(&url, &fixture.schema, 1).await;
        query("SET default_transaction_read_only='on'")
            .execute(&pool)
            .await
            .unwrap();
        query("SET default_transaction_isolation='repeatable read'")
            .execute(&pool)
            .await
            .unwrap();
        let source = FileUploadStore::new(
            pool.clone(),
            scope(),
            fixture.installation_id,
            &fixture.schema,
            limits(),
        )
        .unwrap();
        let readonly = FileUploadStaging::new(source, fixture.staging.clone(), 4).unwrap();
        let before_readonly: String = query("SHOW default_transaction_read_only")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        let before_isolation: String = query("SHOW default_transaction_isolation")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        good(&fixture, &readonly).await;
        let after_readonly: String = query("SHOW default_transaction_read_only")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        let after_isolation: String = query("SHOW default_transaction_isolation")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(before_readonly, after_readonly);
        assert_eq!(before_isolation, after_isolation);
        assert_eq!(tree(&fixture.root), files);
        // No journal is installed in this namespace; current-byte observations do not require one.
        let journal: Option<String> =
            query("SELECT to_regclass('collaboration_file_upload_chunks')::text AS value")
                .fetch_one(&fixture.pool)
                .await
                .unwrap()
                .get("value");
        assert_eq!(journal, None);
        pool.close().await;
        fixture.pool_restored().await;
        fixture.cleanup().await;
    }
}
