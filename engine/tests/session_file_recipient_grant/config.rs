use super::support::*;
use crate::{fixture::*, *};
use cyanrex_engine::services::collaboration_identity_store::IdentityStoreError;
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_recipient_metadata_rejects_source_target_scope_incarnation_and_schema_drift(
) {
    checked(Fixture::ready().await, |f| {
        Box::pin(async move {
            let original = f.issue(1, f.recipient()).await;
            let workspace = f
                .auth
                .source
                .file_recipient_grant_workspace(&f.auth.base.schema, scope(), Uuid::new_v4(), 16)
                .unwrap();
            denied_workspace(
                f,
                &workspace,
                SessionFileRecipientGrantError::InvalidNamespace,
            )
            .await;
            let workspace = f
                .auth
                .source
                .file_recipient_grant_workspace(&f.schema, scope(), Uuid::new_v4(), 16)
                .unwrap();
            denied_workspace(
                f,
                &workspace,
                SessionFileRecipientGrantError::Grant(
                    FileRecipientGrantStoreError::IncarnationMismatch,
                ),
            )
            .await;
            let workspace = f
                .auth
                .source
                .file_recipient_grant_workspace(&f.schema, scope(), f.installation_id, 15)
                .unwrap();
            denied_workspace(
                f,
                &workspace,
                SessionFileRecipientGrantError::Grant(
                    FileRecipientGrantStoreError::UnsupportedSchema,
                ),
            )
            .await;
            for authority_change in [true, false] {
                let mut foreign = scope();
                if authority_change {
                    foreign.authority_id = id();
                } else {
                    foreign.workspace_id = id();
                }
                let workspace = f
                    .auth
                    .source
                    .file_recipient_grant_workspace(&f.schema, foreign, f.installation_id, 16)
                    .unwrap();
                let expected = if authority_change {
                    SessionCommandError::Authentication(DurableAuthError::SourceMismatch)
                } else {
                    SessionCommandError::Identity(IdentityStoreError::ScopeConflict)
                };
                denied_workspace(
                    f,
                    &workspace,
                    SessionFileRecipientGrantError::Session(expected),
                )
                .await;
            }
            for column in ["namespace_oid", "schema_oid", "grants_oid", "changes_oid"] {
                let old: i64 = query(&format!("SELECT {column} AS n FROM {}.{MARKER}", f.schema))
                    .fetch_one(&f.auth.base.admin)
                    .await
                    .unwrap()
                    .get("n");
                query(&format!("UPDATE {}.{MARKER} SET {column}=$1", f.schema))
                    .bind(old + 1)
                    .execute(&f.auth.base.admin)
                    .await
                    .unwrap();
                denied_workspace(
                    f,
                    &f.workspace,
                    SessionFileRecipientGrantError::Grant(
                        FileRecipientGrantStoreError::UnsupportedSchema,
                    ),
                )
                .await;
                query(&format!("UPDATE {}.{MARKER} SET {column}=$1", f.schema))
                    .bind(old)
                    .execute(&f.auth.base.admin)
                    .await
                    .unwrap();
                assert_eq!(
                    read(f, &f.auth.learner_token, 1).await.unwrap(),
                    Some(original.clone())
                );
            }
            query(&format!(
                "ALTER TABLE {}.{HEADS} ALTER COLUMN recipient_account_id DROP NOT NULL",
                f.schema
            ))
            .execute(&f.auth.base.admin)
            .await
            .unwrap();
            denied_workspace(
                f,
                &f.workspace,
                SessionFileRecipientGrantError::Grant(
                    FileRecipientGrantStoreError::UnsupportedSchema,
                ),
            )
            .await;
            query(&format!(
                "ALTER TABLE {}.{HEADS} ALTER COLUMN recipient_account_id SET NOT NULL",
                f.schema
            ))
            .execute(&f.auth.base.admin)
            .await
            .unwrap();
            assert_eq!(
                read(f, &f.auth.learner_token, 1).await.unwrap(),
                Some(original)
            );
            drain(f).await;
        })
    })
    .await;
}
