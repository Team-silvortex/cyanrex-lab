use super::{publisher_fixture::*, *};

async fn closed() -> DurableAuthSource {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    DurableAuthSource::new(pool, authority())
}
fn account(seed: u128) -> FileAccountRef {
    FileAccountRef::new(
        PrincipalRef {
            authority_id: authority(),
            principal_id: Uuid::from_u128(seed + 100).try_into().unwrap(),
        },
        Uuid::from_u128(seed + 200).try_into().unwrap(),
    )
}
fn unavailable() -> SessionFilePublisherGrantError {
    SessionFilePublisherGrantError::Session(SessionCommandError::Authentication(
        DurableAuthError::StorageUnavailable,
    ))
}

#[tokio::test]
async fn file_grant_management_configuration_is_inert_and_exactly_source_bound() {
    let source = closed().await;
    let publisher = source
        .file_publisher_grant_workspace("private_grants", scope(), Uuid::new_v4(), 16)
        .unwrap();
    for manager in [
        account(1),
        FileAccountRef::new(account(1).principal(), account(2).account_id()),
    ] {
        assert!(
            source
                .file_grant_management_workspace(publisher.clone(), manager)
                .is_ok(),
            "typed generations configure inertly; only current Session commands can authorize them"
        );
    }
    let mut foreign_scope = scope();
    foreign_scope.authority_id = id();
    let foreign_publisher = source
        .file_publisher_grant_workspace("private_grants", foreign_scope, Uuid::new_v4(), 16)
        .unwrap();
    let foreign_manager = FileAccountRef::new(
        PrincipalRef {
            authority_id: foreign_scope.authority_id,
            ..account(1).principal()
        },
        account(1).account_id(),
    );
    for (publisher, manager) in [
        (publisher, foreign_manager),
        (foreign_publisher, account(1)),
    ] {
        assert!(matches!(
            source.file_grant_management_workspace(publisher, manager),
            Err(SessionFilePublisherGrantError::Session(
                SessionCommandError::Authentication(DurableAuthError::SourceMismatch)
            ))
        ));
    }
}

#[tokio::test]
async fn file_grant_management_invalid_sessions_are_denied_before_storage() {
    let source = closed().await;
    let publisher = source
        .file_publisher_grant_workspace("private_grants", scope(), Uuid::new_v4(), 16)
        .unwrap();
    let workspace = source
        .file_grant_management_workspace(publisher, account(1))
        .unwrap();
    for token in [
        "",
        "invalid",
        "00000000-0000-0000-0000-000000000000",
        "ABCDEF01-0000-4000-8000-000000000001",
    ] {
        assert_eq!(
            source
                .issue_session_file_publisher_grant(
                    token,
                    &workspace,
                    grant(1),
                    account(2),
                    validity()
                )
                .await,
            Err(SessionFilePublisherGrantError::Session(
                SessionCommandError::InvalidSession
            ))
        );
        assert_eq!(
            source
                .revoke_session_file_publisher_grant(
                    token,
                    &workspace,
                    grant(1),
                    RevisionNumber::try_from(1).unwrap()
                )
                .await,
            Err(SessionFilePublisherGrantError::Session(
                SessionCommandError::InvalidSession
            ))
        );
    }
}

#[tokio::test]
async fn file_grant_management_valid_commands_dispatch_to_explicit_closed_source() {
    let source = closed().await;
    let publisher = source
        .file_publisher_grant_workspace("private_grants", scope(), Uuid::new_v4(), 16)
        .unwrap();
    let workspace = source
        .file_grant_management_workspace(publisher, account(1))
        .unwrap();
    let token = Uuid::new_v4().to_string();
    assert_eq!(
        source
            .issue_session_file_publisher_grant(
                &token,
                &workspace,
                grant(1),
                account(2),
                validity()
            )
            .await,
        Err(unavailable())
    );
    assert_eq!(
        source
            .revoke_session_file_publisher_grant(
                &token,
                &workspace,
                grant(1),
                RevisionNumber::try_from(1).unwrap()
            )
            .await,
        Err(unavailable())
    );
}
