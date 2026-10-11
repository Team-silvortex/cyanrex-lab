use super::{fixture::*, *};

async fn closed() -> DurableAuthSource {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    DurableAuthSource::new(pool, authority())
}
#[tokio::test]
async fn valid_file_metadata_commands_dispatch_to_explicit_closed_source() {
    let source = closed().await;
    let upload_workspace = source
        .file_upload_workspace("private_uploads", scope(), Uuid::new_v4(), limits())
        .unwrap();
    let publication_workspace = source
        .file_publication_workspace("private_publications", scope(), Uuid::new_v4(), 8)
        .unwrap();
    let token = Uuid::new_v4().to_string();
    let expected = SessionFileMetadataError::Session(SessionCommandError::Authentication(
        DurableAuthError::StorageUnavailable,
    ));
    assert_eq!(
        source
            .reserve_session_file_upload(&token, &upload_workspace, upload(1), &manifest(17, 1))
            .await,
        Err(expected)
    );
    assert_eq!(
        source
            .get_session_file_upload(&token, &upload_workspace, upload(1))
            .await,
        Err(expected)
    );
    assert_eq!(
        source
            .get_session_file_publication(&token, &publication_workspace, upload(1))
            .await,
        Err(expected)
    );
}
#[tokio::test]
async fn file_metadata_configuration_validates_without_connecting_or_installing() {
    let source = closed().await;
    for namespace in ["private_files", &"a".repeat(63)] {
        assert!(source
            .file_upload_workspace(namespace, scope(), Uuid::new_v4(), limits())
            .is_ok());
        assert!(source
            .file_publication_workspace(namespace, scope(), Uuid::new_v4(), 1)
            .is_ok());
        assert!(source
            .file_publication_workspace(namespace, scope(), Uuid::new_v4(), 256)
            .is_ok());
    }
    for namespace in [
        "",
        "public",
        "information_schema",
        "pg_private",
        "pg_catalog",
        "Uppercase",
        "1prefix",
        "_prefix",
        "a.b",
        "has space",
        "quote\"",
        &"a".repeat(64),
    ] {
        assert!(
            matches!(
                source.file_upload_workspace(namespace, scope(), Uuid::new_v4(), limits()),
                Err(SessionFileMetadataError::InvalidNamespace)
            ),
            "{namespace}"
        );
        assert!(
            matches!(
                source.file_publication_workspace(namespace, scope(), Uuid::new_v4(), 8),
                Err(SessionFileMetadataError::InvalidNamespace)
            ),
            "{namespace}"
        );
    }
    for identity in [Uuid::nil(), Uuid::from_u128(1)] {
        assert!(matches!(
            source.file_upload_workspace("private_files", scope(), identity, limits()),
            Err(SessionFileMetadataError::Upload(
                FileUploadStoreError::InvalidInput
            ))
        ));
        assert!(matches!(
            source.file_publication_workspace("private_files", scope(), identity, 8),
            Err(SessionFileMetadataError::Publication(
                FileUploadPublicationStoreError::InvalidInput
            ))
        ));
    }
    for max in [0, 257, u32::MAX] {
        assert!(matches!(
            source.file_publication_workspace("private_files", scope(), Uuid::new_v4(), max),
            Err(SessionFileMetadataError::Publication(
                FileUploadPublicationStoreError::InvalidInput
            ))
        ));
    }
}
#[tokio::test]
async fn invalid_token_manifest_scope_and_source_authority_fail_before_storage() {
    let source = closed().await;
    let uploads = source
        .file_upload_workspace("private_uploads", scope(), Uuid::new_v4(), limits())
        .unwrap();
    let publications = source
        .file_publication_workspace("private_publications", scope(), Uuid::new_v4(), 8)
        .unwrap();
    for token in ["", "invalid", "00000000-0000-0000-0000-000000000000"] {
        let expected = SessionFileMetadataError::Session(SessionCommandError::InvalidSession);
        assert_eq!(
            source
                .reserve_session_file_upload(token, &uploads, upload(1), &manifest(17, 1))
                .await,
            Err(expected)
        );
        assert_eq!(
            source
                .get_session_file_upload(token, &uploads, upload(1))
                .await,
            Err(expected)
        );
        assert_eq!(
            source
                .get_session_file_publication(token, &publications, upload(1))
                .await,
            Err(expected)
        );
    }
    let token = Uuid::new_v4().to_string();
    for authority_diff in [false, true] {
        let mut foreign = scope();
        if authority_diff {
            foreign.authority_id = id();
        } else {
            foreign.workspace_id = id();
        }
        let original = manifest(17, 1);
        let m = FileTransferManifest::new(
            foreign,
            original.file_id(),
            original.version_id(),
            original.display_name(),
            17,
            original.chunk_sha256().to_vec(),
            original.sha256().clone(),
        )
        .unwrap();
        assert_eq!(
            source
                .reserve_session_file_upload(&token, &uploads, upload(1), &m)
                .await,
            Err(SessionFileMetadataError::Upload(
                FileUploadStoreError::ScopeMismatch
            ))
        );
        if authority_diff {
            let uploads = source
                .file_upload_workspace("private_uploads", foreign, Uuid::new_v4(), limits())
                .unwrap();
            let publications = source
                .file_publication_workspace("private_publications", foreign, Uuid::new_v4(), 8)
                .unwrap();
            let expected = SessionFileMetadataError::Session(SessionCommandError::Authentication(
                DurableAuthError::SourceMismatch,
            ));
            assert_eq!(
                source
                    .reserve_session_file_upload(&token, &uploads, upload(1), &m)
                    .await,
                Err(expected)
            );
            assert_eq!(
                source
                    .get_session_file_upload(&token, &uploads, upload(1))
                    .await,
                Err(expected)
            );
            assert_eq!(
                source
                    .get_session_file_publication(&token, &publications, upload(1))
                    .await,
                Err(expected)
            );
        }
    }
}
