use crate::{fixture::Fixture, support::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_chunk_read_preserves_source_settings_and_has_no_detached_store_or_upload_journal_dependency(
) {
    checked(Fixture::ready(1).await, |f| {
        Box::pin(async move {
            configure(f, false).await;
            healthy(f).await;
            settings(f, false).await;
            let before = f.sharing.snapshot().await;
            let r = f.request();
            let held = auth(f)
                .source
                .read_session_file_chunk(&auth(f).target_token, &f.workspace, &r)
                .await
                .unwrap();
            let clone = f.workspace.clone();
            assert_eq!(
                auth(f)
                    .source
                    .read_session_file_chunk(&auth(f).target_token, &clone, &r)
                    .await
                    .err(),
                Some(SessionFileChunkReadError::CapacityExhausted)
            );
            let downloads = auth(f)
                .source
                .file_download_workspace(
                    f.sharing.base.publication.clone(),
                    f.sharing.recipient_workspace.clone(),
                )
                .unwrap();
            let independent = auth(f)
                .source
                .file_chunk_read_workspace(downloads, f.staging.clone(), 1)
                .unwrap();
            let other = auth(f)
                .source
                .read_session_file_chunk(&auth(f).target_token, &independent, &r)
                .await
                .unwrap();
            assert_eq!(other.bytes(), held.bytes());
            drop(other);
            drop(held);
            assert_eq!(f.sharing.snapshot().await, before);
            healthy(f).await;
            configure(f, true).await;
            assert_eq!(
                read(f, &r).await,
                Err(session(SessionCommandError::Authentication(
                    DurableAuthError::StorageUnavailable
                )))
            );
            settings(f, true).await;
            configure(f, false).await;
            healthy(f).await;
            settings(f, false).await;
            // Published bytes are independent of current upload/journal state. Exact persisted
            // publication keys address the original root; none of these detached pools is used.
            sql(
                f,
                &format!(
                    "DELETE FROM {}.collaboration_file_uploads",
                    f.sharing.base.upload_schema
                ),
            )
            .await;
            sql(
                f,
                &format!(
                    "DELETE FROM {}.collaboration_file_upload_chunks",
                    f.sharing.base.journal_schema
                ),
            )
            .await;
            f.sharing.pool.close().await;
            f.sharing.base.publication_pool.close().await;
            f.sharing.base.upload_pool.close().await;
            f.sharing.base.journal_pool.close().await;
            f.sharing.base.grants.pool.close().await;
            healthy(f).await;
            settings(f, false).await;
            drain(f).await;
        })
    })
    .await;
}
