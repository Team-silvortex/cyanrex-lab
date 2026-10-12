use crate::{fixture::Fixture, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_download_uses_source_transaction_and_rejects_pinned_configuration_drift() {
    checked(Fixture::ready().await, |f| {
        Box::pin(async move {
            let pub_id: Uuid = query(&format!(
                "SELECT installation_id FROM {}.{PUB_MARKER}",
                f.sharing.base.publication_schema
            ))
            .fetch_one(&auth(f).base.admin)
            .await
            .unwrap()
            .get("installation_id");
            for (component, installation, max) in [
                ("publication", Uuid::new_v4(), 8),
                ("publication", pub_id, 7),
                ("recipient", Uuid::new_v4(), 16),
                ("recipient", f.sharing.installation_id, 15),
            ] {
                let publications = FileUploadPublicationStore::new(
                    f.sharing.base.publication_pool.clone(),
                    scope(),
                    if component == "publication" {
                        installation
                    } else {
                        pub_id
                    },
                    &f.sharing.base.publication_schema,
                    if component == "publication" { max } else { 8 },
                )
                .unwrap();
                let recipients = auth(f)
                    .source
                    .file_recipient_grant_workspace(
                        &f.sharing.schema,
                        scope(),
                        if component == "recipient" {
                            installation
                        } else {
                            f.sharing.installation_id
                        },
                        if component == "recipient" { max } else { 16 },
                    )
                    .unwrap();
                let wrong = auth(f)
                    .source
                    .file_download_workspace(publications, recipients)
                    .unwrap();
                let error = if component == "publication" {
                    publication(if installation != pub_id {
                        FileUploadPublicationStoreError::IncarnationMismatch
                    } else {
                        FileUploadPublicationStoreError::UnsupportedSchema
                    })
                } else {
                    recipient(if installation != f.sharing.installation_id {
                        FileRecipientGrantStoreError::IncarnationMismatch
                    } else {
                        FileRecipientGrantStoreError::UnsupportedSchema
                    })
                };
                assert_eq!(
                    read_as(f, &auth(f).target_token, &wrong, &f.request()).await,
                    Err(error)
                );
                healthy(f).await;
            }
            for component in ["publication", "recipient"] {
                let publications = FileUploadPublicationStore::new(
                    f.sharing.base.publication_pool.clone(),
                    scope(),
                    pub_id,
                    if component == "publication" {
                        &auth(f).base.schema
                    } else {
                        &f.sharing.base.publication_schema
                    },
                    8,
                )
                .unwrap();
                let recipients = auth(f)
                    .source
                    .file_recipient_grant_workspace(
                        if component == "recipient" {
                            &auth(f).base.schema
                        } else {
                            &f.sharing.schema
                        },
                        scope(),
                        f.sharing.installation_id,
                        16,
                    )
                    .unwrap();
                let wrong = auth(f)
                    .source
                    .file_download_workspace(publications, recipients)
                    .unwrap();
                assert_eq!(
                    read_as(f, &auth(f).target_token, &wrong, &f.request()).await,
                    Err(SessionFileDownloadError::InvalidNamespace)
                );
            }
            for (schema, table, column, error) in [
                (
                    f.sharing.schema.clone(),
                    MARKER,
                    "grants_oid",
                    recipient(FileRecipientGrantStoreError::UnsupportedSchema),
                ),
                (
                    f.sharing.schema.clone(),
                    MARKER,
                    "changes_oid",
                    recipient(FileRecipientGrantStoreError::UnsupportedSchema),
                ),
                (
                    f.sharing.base.publication_schema.clone(),
                    PUB_MARKER,
                    "publications_oid",
                    publication(FileUploadPublicationStoreError::UnsupportedSchema),
                ),
            ] {
                sql(
                    f,
                    &format!("UPDATE {schema}.{table} SET {column}={column}+1"),
                )
                .await;
                assert_eq!(read(f, &f.request()).await, Err(error));
                sql(
                    f,
                    &format!("UPDATE {schema}.{table} SET {column}={column}-1"),
                )
                .await;
                healthy(f).await;
            }
            for (schema, table, column, error) in [
                (
                    f.sharing.schema.clone(),
                    HEADS,
                    "recipient_account_id",
                    recipient(FileRecipientGrantStoreError::UnsupportedSchema),
                ),
                (
                    f.sharing.base.publication_schema.clone(),
                    PUB_ROWS,
                    "source_key",
                    publication(FileUploadPublicationStoreError::UnsupportedSchema),
                ),
            ] {
                sql(
                    f,
                    &format!("ALTER TABLE {schema}.{table} ALTER COLUMN {column} DROP NOT NULL"),
                )
                .await;
                assert_eq!(read(f, &f.request()).await, Err(error));
                sql(
                    f,
                    &format!("ALTER TABLE {schema}.{table} ALTER COLUMN {column} SET NOT NULL"),
                )
                .await;
                healthy(f).await;
            }
            configure(f, true).await;
            assert_eq!(
                read(f, &f.request()).await,
                Err(session(SessionCommandError::Authentication(
                    DurableAuthError::StorageUnavailable
                ))),
                "source read-only policy cannot be weakened to permit locking authorization"
            );
            settings(f, true).await;
            configure(f, false).await;
            healthy(f).await;
            settings(f, false).await;
            // Kill every detached store connection and make the cached original staging pathname
            // unavailable. Only the current SOURCE transaction may perform this metadata observation.
            f.sharing.pool.close().await;
            f.sharing.base.publication_pool.close().await;
            f.sharing.base.upload_pool.close().await;
            f.sharing.base.journal_pool.close().await;
            f.sharing.base.grants.pool.close().await;
            let moved = f
                .sharing
                .base
                .directory
                .join("retained-but-not-configured-root");
            std::fs::rename(&f.sharing.base.root, &moved).unwrap();
            f.sharing.base.root = moved;
            healthy(f).await;
            settings(f, false).await;
            drain(f).await;
        })
    })
    .await;
}
