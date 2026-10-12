use crate::*;

async fn closed() -> (DurableAuthSource, PgPool) {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    (DurableAuthSource::new(pool.clone(), authority()), pool)
}

fn workspace(source: &DurableAuthSource, pool: PgPool) -> SessionFileDownloadWorkspace {
    let publication =
        FileUploadPublicationStore::new(pool, scope(), Uuid::new_v4(), "download_publications", 16)
            .unwrap();
    let recipients = source
        .file_recipient_grant_workspace("download_recipients", scope(), Uuid::new_v4(), 16)
        .unwrap();
    source
        .file_download_workspace(publication, recipients)
        .unwrap()
}

fn request() -> SessionFileDownloadRequest {
    SessionFileDownloadRequest {
        grant_id: sharing_fixture::recipient_grant(1),
        upload_id: publication_fixture::upload(1),
        expected_file: FileVersionRef::from_manifest(&bytes::manifest(17, 1)),
    }
}

#[tokio::test]
async fn download_configuration_is_inert_and_requires_two_distinct_exact_scopes() {
    let (source, pool) = closed().await;
    let _ = workspace(&source, pool.clone());
    for name in ["download_recipients", "_download_publications"] {
        let publication =
            FileUploadPublicationStore::new(pool.clone(), scope(), Uuid::new_v4(), name, 16)
                .unwrap();
        let recipients = source
            .file_recipient_grant_workspace("download_recipients", scope(), Uuid::new_v4(), 16)
            .unwrap();
        assert!(matches!(
            source.file_download_workspace(publication, recipients),
            Err(SessionFileDownloadError::InvalidNamespace)
        ));
    }
    let mut foreign = scope();
    foreign.authority_id = id();
    let publication = FileUploadPublicationStore::new(
        pool.clone(),
        foreign,
        Uuid::new_v4(),
        "download_publications",
        16,
    )
    .unwrap();
    let recipients = source
        .file_recipient_grant_workspace("download_recipients", foreign, Uuid::new_v4(), 16)
        .unwrap();
    assert!(matches!(
        source.file_download_workspace(publication, recipients),
        Err(SessionFileDownloadError::Session(
            SessionCommandError::Authentication(DurableAuthError::SourceMismatch)
        ))
    ));
    for component in ["publication", "recipient"] {
        let mut foreign = scope();
        foreign.workspace_id = id();
        let publication = FileUploadPublicationStore::new(
            pool.clone(),
            if component == "publication" {
                foreign
            } else {
                scope()
            },
            Uuid::new_v4(),
            "download_publications",
            16,
        )
        .unwrap();
        let recipients = source
            .file_recipient_grant_workspace(
                "download_recipients",
                if component == "recipient" {
                    foreign
                } else {
                    scope()
                },
                Uuid::new_v4(),
                16,
            )
            .unwrap();
        assert!(matches!(
            source.file_download_workspace(publication, recipients),
            Err(SessionFileDownloadError::Recipient(
                FileRecipientGrantStoreError::ScopeMismatch
            ))
        ));
    }
}

#[tokio::test]
async fn download_invalid_token_and_foreign_file_scope_fail_before_storage() {
    let (source, pool) = closed().await;
    let workspace = workspace(&source, pool);
    let mut request = request();
    for token in ["", "invalid", "00000000-0000-0000-0000-000000000000"] {
        assert_eq!(
            source
                .get_session_file_download_manifest(token, &workspace, &request)
                .await,
            Err(SessionFileDownloadError::Session(
                SessionCommandError::InvalidSession
            ))
        );
    }
    for field in ["authority_id", "workspace_id"] {
        let mut raw = serde_json::to_value(&request.expected_file).unwrap();
        raw["workspace"][field] = serde_json::json!(Uuid::new_v4());
        request.expected_file = serde_json::from_value(raw).unwrap();
        assert_eq!(
            source
                .get_session_file_download_manifest(
                    &Uuid::new_v4().to_string(),
                    &workspace,
                    &request,
                )
                .await,
            Err(SessionFileDownloadError::Recipient(
                FileRecipientGrantStoreError::ScopeMismatch
            ))
        );
        request = self::request();
    }
}

#[tokio::test]
async fn download_valid_request_dispatches_to_explicit_closed_source() {
    let (source, pool) = closed().await;
    let workspace = workspace(&source, pool);
    assert_eq!(source.get_session_file_download_manifest(
        &Uuid::new_v4().to_string(), &workspace, &request(),
    ).await, Err(SessionFileDownloadError::Session(SessionCommandError::Authentication(
        DurableAuthError::StorageUnavailable))));
}
