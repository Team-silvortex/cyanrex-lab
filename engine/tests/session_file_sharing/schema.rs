use super::support::*;
use crate::{fixture::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_sharing_rejects_namespace_scope_schema_and_incarnation_drift() {
    checked(Fixture::ready().await, |f| {
        Box::pin(async move {
            let source = &f.base.grants.auth.source;
            let publication_store = |installation, max| {
                FileUploadPublicationStore::new(
                    f.base.publication_pool.clone(),
                    scope(),
                    installation,
                    &f.base.publication_schema,
                    max,
                )
                .unwrap()
            };
            let pub_id: Uuid = query(&format!(
                "SELECT installation_id FROM {}.{PUB_MARKER}",
                f.base.publication_schema
            ))
            .fetch_one(&f.base.grants.auth.base.admin)
            .await
            .unwrap()
            .get("installation_id");
            let wrong = source
                .file_sharing_workspace(
                    publication_store(Uuid::new_v4(), 8),
                    f.base.grants.workspace.clone(),
                    f.recipient_workspace.clone(),
                )
                .unwrap();
            assert_eq!(
                issue_as(f, &f.base.grants.auth.learner_token, &wrong, &f.command(1)).await,
                Err(publication(
                    FileUploadPublicationStoreError::IncarnationMismatch
                ))
            );
            let wrong_grants = source
                .file_publisher_grant_workspace(&f.base.grants.schema, scope(), Uuid::new_v4(), 16)
                .unwrap();
            let wrong = source
                .file_sharing_workspace(
                    publication_store(pub_id, 8),
                    wrong_grants,
                    f.recipient_workspace.clone(),
                )
                .unwrap();
            assert_eq!(
                issue_as(f, &f.base.grants.auth.learner_token, &wrong, &f.command(1)).await,
                Err(publisher(FilePublisherGrantStoreError::IncarnationMismatch))
            );
            let wrong_rec = source
                .file_recipient_grant_workspace(&f.schema, scope(), Uuid::new_v4(), f.max_grants)
                .unwrap();
            let wrong = source
                .file_sharing_workspace(
                    publication_store(pub_id, 8),
                    f.base.grants.workspace.clone(),
                    wrong_rec,
                )
                .unwrap();
            assert_eq!(
                issue_as(f, &f.base.grants.auth.learner_token, &wrong, &f.command(1)).await,
                Err(recipient(FileRecipientGrantStoreError::IncarnationMismatch))
            );
            let wrong_rec = source
                .file_recipient_grant_workspace(
                    &f.schema,
                    scope(),
                    f.installation_id,
                    f.max_grants - 1,
                )
                .unwrap();
            let wrong = source
                .file_sharing_workspace(
                    publication_store(pub_id, 8),
                    f.base.grants.workspace.clone(),
                    wrong_rec,
                )
                .unwrap();
            assert_eq!(
                issue_as(f, &f.base.grants.auth.learner_token, &wrong, &f.command(1)).await,
                Err(recipient(FileRecipientGrantStoreError::UnsupportedSchema))
            );
            for (schema, table, column, error) in [
                (
                    &f.base.publication_schema,
                    PUB_MARKER,
                    "publications_oid",
                    publication(FileUploadPublicationStoreError::UnsupportedSchema),
                ),
                (
                    &f.base.grants.schema,
                    grant_fixture::MARKER,
                    "grants_oid",
                    publisher(FilePublisherGrantStoreError::UnsupportedSchema),
                ),
                (
                    &f.schema,
                    MARKER,
                    "changes_oid",
                    recipient(FileRecipientGrantStoreError::UnsupportedSchema),
                ),
            ] {
                sql(
                    f,
                    &format!("UPDATE {schema}.{table} SET {column}={column}+1"),
                )
                .await;
                assert_eq!(issue(f, 1).await, Err(error));
                sql(
                    f,
                    &format!("UPDATE {schema}.{table} SET {column}={column}-1"),
                )
                .await;
            }
            sql(
                f,
                &format!(
                    "ALTER TABLE {}.{HEADS} ALTER COLUMN recipient_account_id DROP NOT NULL",
                    f.schema
                ),
            )
            .await;
            assert_eq!(
                issue(f, 1).await,
                Err(recipient(FileRecipientGrantStoreError::UnsupportedSchema))
            );
            sql(
                f,
                &format!(
                    "ALTER TABLE {}.{HEADS} ALTER COLUMN recipient_account_id SET NOT NULL",
                    f.schema
                ),
            )
            .await;
            let same_source = source
                .file_recipient_grant_workspace(
                    &f.base.grants.auth.base.schema,
                    scope(),
                    Uuid::new_v4(),
                    16,
                )
                .unwrap();
            let wrong = source
                .file_sharing_workspace(
                    publication_store(pub_id, 8),
                    f.base.grants.workspace.clone(),
                    same_source,
                )
                .unwrap();
            assert_eq!(
                issue_as(f, &f.base.grants.auth.learner_token, &wrong, &f.command(1)).await,
                Err(SessionFileSharingError::InvalidNamespace)
            );
            issue(f, 1).await.unwrap();
        })
    })
    .await;
}
