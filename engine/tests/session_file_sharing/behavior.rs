use super::support::*;
use crate::{fixture::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_sharing_self_and_trusted_issuer_remain_owned_without_files_or_installer_pools(
) {
    checked(Fixture::ready().await, |f| {
        Box::pin(async move {
            let old = issue(f, 1).await.unwrap();
            assert_eq!(old.issuer(), f.actor());
            let mut own = f.command(2);
            own.recipient = f.actor();
            let self_share = issue_as(
                f,
                &f.base.grants.auth.learner_token,
                &f.workspace.clone(),
                &own,
            )
            .await
            .unwrap();
            assert_eq!(self_share.grant().recipient(), f.actor());
            assert_eq!(self_share.grant().file(), &f.expected_file);
            assert_eq!(
                revoke_as(f, &f.base.grants.auth.manager_token, 1, revision(1)).await,
                Err(SessionFileSharingError::PermissionDenied)
            );
            assert_eq!(
                revoke_as(f, &f.base.grants.auth.target_token, 1, revision(1)).await,
                Err(SessionFileSharingError::PermissionDenied)
            );
            for rev in [2, 3] {
                assert_eq!(
                    revoke(f, 1, revision(rev)).await,
                    Err(recipient(FileRecipientGrantStoreError::Conflict))
                );
            }
            assert_eq!(
                revoke(f, 99, revision(1)).await,
                Err(SessionFileSharingError::PermissionDenied)
            );
            let before = f.snapshot().await;
            let trusted = f
                .store
                .issue(
                    recipient_grant(3),
                    f.base.grants.issuer(),
                    f.expected_file.clone(),
                    f.actor(),
                    f.recipient,
                    grant_fixture::validity(),
                )
                .await
                .unwrap();
            recipient_delta(&f.schema, &before, &f.snapshot().await, 3, false);
            assert_ne!(trusted.issuer(), trusted.grant().publisher());
            let retired = revoke(f, 3, revision(1)).await.unwrap();
            assert_eq!(retired.issuer(), trusted.issuer());
            assert_eq!(retired.grant().publisher(), f.actor());
            // The command is metadata-only: the original staging pathname disappears and all
            // independent installer pools close, yet the source-owned SQL transaction still works.
            let saved = f.base.directory.join("original_chunks");
            std::fs::rename(&f.base.root, &saved).unwrap();
            f.base.root = saved;
            f.base.upload_pool.close().await;
            f.base.journal_pool.close().await;
            f.base.publication_pool.close().await;
            f.base.grants.pool.close().await;
            f.pool.close().await;
            let fourth = issue(f, 4).await.unwrap();
            assert_eq!(fourth.grant().file(), &f.expected_file);
            revoke(f, 1, revision(1)).await.unwrap();
            assert_eq!(
                revoke(f, 1, revision(1)).await,
                Err(recipient(FileRecipientGrantStoreError::Conflict))
            );
            // Even unavailable publication and publisher-grant relations cannot prevent stopping
            // an old share. This says nothing about bytes being healthy or downloadable.
            sql(
                f,
                &format!(
                    "ALTER TABLE {}.{PUB_ROWS} RENAME TO retained_publications",
                    f.base.publication_schema
                ),
            )
            .await;
            sql(
                f,
                &format!(
                    "ALTER TABLE {}.{} RENAME TO retained_publisher_grants",
                    f.base.grants.schema,
                    grant_fixture::HEADS
                ),
            )
            .await;
            let stopped = revoke(f, 4, revision(1)).await.unwrap();
            assert_eq!(stopped.grant().state(), FileGrantState::Revoked);
            assert_eq!(stopped.grant().file(), fourth.grant().file());
        })
    })
    .await;
}
