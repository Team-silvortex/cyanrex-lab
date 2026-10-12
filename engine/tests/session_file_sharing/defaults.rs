use crate::{fixture::*, *};
async fn closed() -> (DurableAuthSource, PgPool) {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    (DurableAuthSource::new(pool.clone(), authority()), pool)
}
fn workspace(source: &DurableAuthSource, pool: PgPool) -> SessionFileSharingWorkspace {
    let publication =
        FileUploadPublicationStore::new(pool, scope(), Uuid::new_v4(), "sharing_publications", 16)
            .unwrap();
    let publishers = source
        .file_publisher_grant_workspace("sharing_publishers", scope(), Uuid::new_v4(), 16)
        .unwrap();
    let recipients = source
        .file_recipient_grant_workspace("sharing_recipients", scope(), Uuid::new_v4(), 16)
        .unwrap();
    source
        .file_sharing_workspace(publication, publishers, recipients)
        .unwrap()
}
fn command() -> SessionFileRecipientGrantIssue {
    SessionFileRecipientGrantIssue {
        grant_id: recipient_grant(1),
        publisher_grant_id: grant_fixture::grant(1),
        upload_id: publication_fixture::upload(1),
        expected_file: FileVersionRef::from_manifest(&bytes::manifest(17, 1)),
        recipient: FileAccountRef::new(
            PrincipalRef {
                authority_id: authority(),
                principal_id: id(),
            },
            id(),
        ),
        validity: grant_fixture::validity(),
    }
}
#[tokio::test]
async fn sharing_configuration_is_inert_and_requires_three_distinct_exact_scopes() {
    let (source, pool) = closed().await;
    let _ = workspace(&source, pool.clone());
    for (pub_name, grant_name, rec_name) in [
        ("same", "same", "other"),
        ("same", "other", "same"),
        ("other", "same", "same"),
    ] {
        let publication =
            FileUploadPublicationStore::new(pool.clone(), scope(), Uuid::new_v4(), pub_name, 16)
                .unwrap();
        let publishers = source
            .file_publisher_grant_workspace(grant_name, scope(), Uuid::new_v4(), 16)
            .unwrap();
        let recipients = source
            .file_recipient_grant_workspace(rec_name, scope(), Uuid::new_v4(), 16)
            .unwrap();
        assert!(matches!(
            source.file_sharing_workspace(publication, publishers, recipients),
            Err(SessionFileSharingError::InvalidNamespace)
        ));
    }
    let publication = FileUploadPublicationStore::new(
        pool.clone(),
        scope(),
        Uuid::new_v4(),
        "_sharing_publications",
        16,
    )
    .unwrap();
    let publishers = source
        .file_publisher_grant_workspace("sharing_publishers", scope(), Uuid::new_v4(), 16)
        .unwrap();
    let recipients = source
        .file_recipient_grant_workspace("sharing_recipients", scope(), Uuid::new_v4(), 16)
        .unwrap();
    assert!(matches!(
        source.file_sharing_workspace(publication, publishers, recipients),
        Err(SessionFileSharingError::InvalidNamespace)
    ));
    let mut foreign = scope();
    foreign.authority_id = id();
    let publication = FileUploadPublicationStore::new(
        pool.clone(),
        foreign,
        Uuid::new_v4(),
        "sharing_publications",
        16,
    )
    .unwrap();
    let publishers = source
        .file_publisher_grant_workspace("sharing_publishers", foreign, Uuid::new_v4(), 16)
        .unwrap();
    let recipients = source
        .file_recipient_grant_workspace("sharing_recipients", foreign, Uuid::new_v4(), 16)
        .unwrap();
    assert!(matches!(
        source.file_sharing_workspace(publication, publishers, recipients),
        Err(SessionFileSharingError::Session(
            SessionCommandError::Authentication(DurableAuthError::SourceMismatch)
        ))
    ));
    for component in ["publication", "publisher", "recipient"] {
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
            "sharing_publications",
            16,
        )
        .unwrap();
        let publishers = source
            .file_publisher_grant_workspace(
                "sharing_publishers",
                if component == "publisher" {
                    foreign
                } else {
                    scope()
                },
                Uuid::new_v4(),
                16,
            )
            .unwrap();
        let recipients = source
            .file_recipient_grant_workspace(
                "sharing_recipients",
                if component == "recipient" {
                    foreign
                } else {
                    scope()
                },
                Uuid::new_v4(),
                16,
            )
            .unwrap();
        let error = if component == "recipient" {
            SessionFileSharingError::Recipient(FileRecipientGrantStoreError::ScopeMismatch)
        } else {
            SessionFileSharingError::Publisher(FilePublisherGrantStoreError::ScopeMismatch)
        };
        assert!(
            matches!(source.file_sharing_workspace(publication,publishers,recipients),Err(e) if e==error)
        );
    }
}
#[tokio::test]
async fn sharing_invalid_token_and_foreign_selectors_fail_before_storage() {
    let (source, pool) = closed().await;
    let workspace = workspace(&source, pool);
    let mut command = command();
    for token in ["", "invalid", "00000000-0000-0000-0000-000000000000"] {
        assert_eq!(
            source
                .issue_session_file_recipient_grant(token, &workspace, &command)
                .await,
            Err(SessionFileSharingError::Session(
                SessionCommandError::InvalidSession
            ))
        );
        assert_eq!(
            source
                .revoke_session_file_recipient_grant(
                    token,
                    &workspace,
                    recipient_grant(1),
                    revision(1)
                )
                .await,
            Err(SessionFileSharingError::Session(
                SessionCommandError::InvalidSession
            ))
        );
    }
    let mut raw = serde_json::to_value(&command.expected_file).unwrap();
    raw["workspace"]["workspace_id"] = serde_json::json!(Uuid::new_v4());
    command.expected_file = serde_json::from_value(raw).unwrap();
    assert_eq!(
        source
            .issue_session_file_recipient_grant(&Uuid::new_v4().to_string(), &workspace, &command)
            .await,
        Err(SessionFileSharingError::Recipient(
            FileRecipientGrantStoreError::ScopeMismatch
        ))
    );
    command = command_with_foreign_recipient();
    assert_eq!(
        source
            .issue_session_file_recipient_grant(&Uuid::new_v4().to_string(), &workspace, &command)
            .await,
        Err(SessionFileSharingError::Recipient(
            FileRecipientGrantStoreError::ScopeMismatch
        ))
    );
}
fn command_with_foreign_recipient() -> SessionFileRecipientGrantIssue {
    let mut c = command();
    c.recipient = FileAccountRef::new(
        PrincipalRef {
            authority_id: id(),
            ..c.recipient.principal()
        },
        c.recipient.account_id(),
    );
    c
}
#[tokio::test]
async fn sharing_valid_commands_dispatch_to_explicit_closed_source() {
    let (source, pool) = closed().await;
    let workspace = workspace(&source, pool);
    let expected = Err(SessionFileSharingError::Session(
        SessionCommandError::Authentication(DurableAuthError::StorageUnavailable),
    ));
    assert_eq!(
        source
            .issue_session_file_recipient_grant(&Uuid::new_v4().to_string(), &workspace, &command())
            .await,
        expected
    );
    assert_eq!(
        source
            .revoke_session_file_recipient_grant(
                &Uuid::new_v4().to_string(),
                &workspace,
                recipient_grant(1),
                revision(1)
            )
            .await,
        expected
    );
}
