use super::{
    fixture::*,
    publisher_fixture::{grant, validity, HEADS, MARKER},
    support::*,
    *,
};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_file_grant_management_rejects_namespace_installation_scope_shape_and_foreign_ledger_drift(
) {
    let f = Fixture::ready().await;
    let old = issue(&f, 1, f.target()).await.unwrap();
    let wrongs = [
        (
            f.base.schema.clone(),
            scope(),
            Uuid::new_v4(),
            16,
            grant_error(FilePublisherGrantStoreError::IncarnationMismatch),
        ),
        (
            f.base.schema.clone(),
            scope(),
            f.base.installation_id,
            15,
            grant_error(FilePublisherGrantStoreError::UnsupportedSchema),
        ),
        (
            f.base.auth.base.schema.clone(),
            scope(),
            f.base.installation_id,
            16,
            SessionFilePublisherGrantError::InvalidNamespace,
        ),
    ];
    for (schema, scope, installation, max, expected) in wrongs {
        let publisher = f
            .base
            .auth
            .source
            .file_publisher_grant_workspace(&schema, scope, installation, max)
            .unwrap();
        let config = f
            .base
            .auth
            .source
            .file_grant_management_workspace(publisher, f.manager())
            .unwrap();
        assert_eq!(
            issue_as(
                &f,
                &f.base.auth.learner_token,
                &config,
                2,
                f.target(),
                validity()
            )
            .await,
            Err(expected)
        );
        assert_eq!(
            revoke_as(&f, &f.base.auth.learner_token, &config, 1, revision(1)).await,
            Err(expected)
        );
    }
    let mut foreign = scope();
    foreign.workspace_id = id();
    let publisher = f
        .base
        .auth
        .source
        .file_publisher_grant_workspace(&f.base.schema, foreign, f.base.installation_id, 16)
        .unwrap();
    let config = f
        .base
        .auth
        .source
        .file_grant_management_workspace(publisher, f.manager())
        .unwrap();
    assert_eq!(
        issue_as(
            &f,
            &f.base.auth.learner_token,
            &config,
            2,
            f.target(),
            validity()
        )
        .await,
        Err(identity_error(IdentityStoreError::ScopeConflict))
    );
    sql(
        &f,
        false,
        &format!("ALTER TABLE {HEADS} ALTER COLUMN issuer_id DROP NOT NULL"),
    )
    .await;
    assert_eq!(
        issue(&f, 2, f.target()).await,
        Err(grant_error(FilePublisherGrantStoreError::UnsupportedSchema))
    );
    assert_eq!(
        revoke(&f, 1, revision(1)).await,
        Err(grant_error(FilePublisherGrantStoreError::UnsupportedSchema))
    );
    sql(
        &f,
        false,
        &format!("ALTER TABLE {HEADS} ALTER COLUMN issuer_id SET NOT NULL"),
    )
    .await;
    query(&format!(
        "UPDATE {}.{HEADS} SET grant_payload=$1 WHERE grant_id=$2",
        f.base.schema
    ))
    .bind(b"{".as_slice())
    .bind(grant(1).as_uuid())
    .execute(&f.base.pool)
    .await
    .unwrap();
    assert_eq!(
        issue(&f, 2, f.manager()).await,
        Err(grant_error(FilePublisherGrantStoreError::InvalidRecord)),
        "foreign selected subject must not hide corruption from the whole-ledger writer"
    );
    assert_eq!(
        revoke(&f, 1, revision(1)).await,
        Err(grant_error(FilePublisherGrantStoreError::InvalidRecord))
    );
    query(&format!(
        "UPDATE {}.{HEADS} SET grant_payload=$1 WHERE grant_id=$2",
        f.base.schema
    ))
    .bind(serde_json::to_vec(old.grant()).unwrap())
    .bind(grant(1).as_uuid())
    .execute(&f.base.pool)
    .await
    .unwrap();
    sql(
        &f,
        false,
        &format!("UPDATE {MARKER} SET grants_oid=grants_oid+1"),
    )
    .await;
    assert_eq!(
        issue(&f, 2, f.target()).await,
        Err(grant_error(FilePublisherGrantStoreError::UnsupportedSchema))
    );
    sql(
        &f,
        false,
        &format!("UPDATE {MARKER} SET grants_oid=grants_oid-1"),
    )
    .await;
    issue(&f, 2, f.target()).await.unwrap();
    revoke(&f, 1, revision(1)).await.unwrap();
    f.cleanup().await;
}
