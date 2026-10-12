use super::support::*;
use crate::{fixture::*, *};
use cyanrex_engine::services::collaboration_identity_store::IdentityStoreError;

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_publisher_grant_rejects_scope_installation_limits_and_shape_drift() {
    let f = Fixture::ready().await;
    let original = f.issue(1, f.subject()).await;
    let source_target = f
        .auth
        .source
        .file_publisher_grant_workspace(&f.auth.base.schema, scope(), Uuid::new_v4(), 16)
        .unwrap();
    denied_workspace(
        &f,
        &source_target,
        SessionFilePublisherGrantError::InvalidNamespace,
    )
    .await;
    let wrong_install = f
        .auth
        .source
        .file_publisher_grant_workspace(&f.schema, scope(), Uuid::new_v4(), 16)
        .unwrap();
    denied_workspace(
        &f,
        &wrong_install,
        SessionFilePublisherGrantError::Grant(FilePublisherGrantStoreError::IncarnationMismatch),
    )
    .await;
    let wrong_max = f
        .auth
        .source
        .file_publisher_grant_workspace(&f.schema, scope(), f.installation_id, 15)
        .unwrap();
    denied_workspace(
        &f,
        &wrong_max,
        SessionFilePublisherGrantError::Grant(FilePublisherGrantStoreError::UnsupportedSchema),
    )
    .await;
    let mut wrong_scope = scope();
    wrong_scope.authority_id = id();
    let w = f
        .auth
        .source
        .file_publisher_grant_workspace(&f.schema, wrong_scope, f.installation_id, 16)
        .unwrap();
    denied_workspace(
        &f,
        &w,
        SessionFilePublisherGrantError::Session(SessionCommandError::Authentication(
            DurableAuthError::SourceMismatch,
        )),
    )
    .await;
    wrong_scope = scope();
    wrong_scope.workspace_id = id();
    let w = f
        .auth
        .source
        .file_publisher_grant_workspace(&f.schema, wrong_scope, f.installation_id, 16)
        .unwrap();
    denied_workspace(
        &f,
        &w,
        SessionFilePublisherGrantError::Session(SessionCommandError::Identity(
            IdentityStoreError::ScopeConflict,
        )),
    )
    .await;
    for column in ["namespace_oid", "schema_oid", "grants_oid", "changes_oid"] {
        let old: i64 = query(&format!("SELECT {column} AS n FROM {}.{MARKER}", f.schema))
            .fetch_one(&f.auth.base.admin)
            .await
            .unwrap()
            .get("n");
        assert_eq!(
            query(&format!("UPDATE {}.{MARKER} SET {column}=$1", f.schema))
                .bind(old + 1)
                .execute(&f.auth.base.admin)
                .await
                .unwrap()
                .rows_affected(),
            1
        );
        denied_workspace(
            &f,
            &f.workspace,
            SessionFilePublisherGrantError::Grant(FilePublisherGrantStoreError::UnsupportedSchema),
        )
        .await;
        query(&format!("UPDATE {}.{MARKER} SET {column}=$1", f.schema))
            .bind(old)
            .execute(&f.auth.base.admin)
            .await
            .unwrap();
        assert_eq!(
            read(&f, &f.auth.learner_token, 1).await.unwrap(),
            Some(original.clone())
        );
    }
    query(&format!(
        "ALTER TABLE {}.{HEADS} ALTER COLUMN subject_account_id DROP NOT NULL",
        f.schema
    ))
    .execute(&f.auth.base.admin)
    .await
    .unwrap();
    denied_workspace(
        &f,
        &f.workspace,
        SessionFilePublisherGrantError::Grant(FilePublisherGrantStoreError::UnsupportedSchema),
    )
    .await;
    query(&format!(
        "ALTER TABLE {}.{HEADS} ALTER COLUMN subject_account_id SET NOT NULL",
        f.schema
    ))
    .execute(&f.auth.base.admin)
    .await
    .unwrap();
    assert_eq!(
        read(&f, &f.auth.learner_token, 1).await.unwrap(),
        Some(original)
    );
    f.auth.base.drain().await;
    f.cleanup().await;
}
