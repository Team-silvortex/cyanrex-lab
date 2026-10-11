use super::{fixture::*, support::*, *};

async fn rejected_upload(f: &Fixture, w: &SessionFileUploadWorkspace, e: SessionFileMetadataError) {
    let before = snapshot(f).await;
    let read = f
        .auth
        .source
        .get_session_file_upload(&f.auth.learner_token, w, upload(1))
        .await;
    let write = f
        .auth
        .source
        .reserve_session_file_upload(&f.auth.learner_token, w, upload(2), &manifest(17, 2))
        .await;
    let after = snapshot(f).await;
    assert_eq!(read, Err(e));
    assert_eq!(write, Err(e));
    assert_eq!(after, before);
}
async fn rejected_publication(
    f: &Fixture,
    w: &SessionFilePublicationWorkspace,
    e: SessionFileMetadataError,
) {
    let before = snapshot(f).await;
    let read = f
        .auth
        .source
        .get_session_file_publication(&f.auth.learner_token, w, upload(1))
        .await;
    assert_eq!(snapshot(f).await, before);
    assert_eq!(read, Err(e));
}
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_metadata_rejects_namespace_installation_limits_and_relation_drift() {
    let f = Fixture::ready().await;
    reserve(&f, &f.auth.learner_token, 1, &manifest(17, 1))
        .await
        .unwrap();
    let source_upload = f
        .auth
        .source
        .file_upload_workspace(&f.auth.base.schema, scope(), Uuid::new_v4(), limits())
        .unwrap();
    rejected_upload(
        &f,
        &source_upload,
        SessionFileMetadataError::InvalidNamespace,
    )
    .await;
    let source_pub = f
        .auth
        .source
        .file_publication_workspace(&f.auth.base.schema, scope(), Uuid::new_v4(), 8)
        .unwrap();
    rejected_publication(&f, &source_pub, SessionFileMetadataError::InvalidNamespace).await;
    let wrong_install = f
        .auth
        .source
        .file_upload_workspace(&f.upload_schema, scope(), Uuid::new_v4(), limits())
        .unwrap();
    rejected_upload(
        &f,
        &wrong_install,
        SessionFileMetadataError::Upload(FileUploadStoreError::IncarnationMismatch),
    )
    .await;
    let wrong_limits = f
        .auth
        .source
        .file_upload_workspace(
            &f.upload_schema,
            scope(),
            f.upload_installation_id,
            FileUploadLimits::new(1023, 1023, 16, 8).unwrap(),
        )
        .unwrap();
    rejected_upload(
        &f,
        &wrong_limits,
        SessionFileMetadataError::Upload(FileUploadStoreError::UnsupportedSchema),
    )
    .await;
    let wrong_install = f
        .auth
        .source
        .file_publication_workspace(&f.publication_schema, scope(), Uuid::new_v4(), 8)
        .unwrap();
    rejected_publication(
        &f,
        &wrong_install,
        SessionFileMetadataError::Publication(FileUploadPublicationStoreError::IncarnationMismatch),
    )
    .await;
    let wrong_limit = f
        .auth
        .source
        .file_publication_workspace(
            &f.publication_schema,
            scope(),
            f.publication_installation_id,
            7,
        )
        .unwrap();
    rejected_publication(
        &f,
        &wrong_limit,
        SessionFileMetadataError::Publication(FileUploadPublicationStoreError::UnsupportedSchema),
    )
    .await;
    for (schema, marker, column, is_upload) in [
        (&f.upload_schema, UPLOAD_MARKER, "uploads_oid", true),
        (
            &f.publication_schema,
            PUBLICATION_MARKER,
            "publications_oid",
            false,
        ),
    ] {
        let original: i64 = query(&format!("SELECT {column} AS n FROM {schema}.{marker}"))
            .fetch_one(&f.auth.base.admin)
            .await
            .unwrap()
            .get("n");
        query(&format!(
            "UPDATE {schema}.{marker} SET {column}={}",
            original + 1
        ))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
        if is_upload {
            rejected_upload(
                &f,
                &f.upload_workspace,
                SessionFileMetadataError::Upload(FileUploadStoreError::UnsupportedSchema),
            )
            .await;
        } else {
            rejected_publication(
                &f,
                &f.publication_workspace,
                SessionFileMetadataError::Publication(
                    FileUploadPublicationStoreError::UnsupportedSchema,
                ),
            )
            .await;
        }
        query(&format!("UPDATE {schema}.{marker} SET {column}={original}"))
            .execute(&f.auth.base.admin)
            .await
            .unwrap();
        assert!(get_upload(&f, &f.auth.learner_token, 1)
            .await
            .unwrap()
            .is_some());
        assert_eq!(
            get_publication(&f, &f.auth.learner_token, 99)
                .await
                .unwrap(),
            None
        );
    }
    // A same-name ordinary table without the original required NOT NULL shape is not adopted.
    query(&format!(
        "ALTER TABLE {}.{UPLOAD_ROWS} ALTER COLUMN account_id DROP NOT NULL",
        f.upload_schema
    ))
    .execute(&f.auth.base.admin)
    .await
    .unwrap();
    rejected_upload(
        &f,
        &f.upload_workspace,
        SessionFileMetadataError::Upload(FileUploadStoreError::UnsupportedSchema),
    )
    .await;
    query(&format!(
        "ALTER TABLE {}.{UPLOAD_ROWS} ALTER COLUMN account_id SET NOT NULL",
        f.upload_schema
    ))
    .execute(&f.auth.base.admin)
    .await
    .unwrap();
    reserve(&f, &f.auth.learner_token, 3, &manifest(17, 3))
        .await
        .unwrap();
    f.pool_restored().await;
    f.cleanup().await;
}
