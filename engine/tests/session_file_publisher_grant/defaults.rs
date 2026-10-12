use crate::{fixture::*, *};

async fn closed() -> DurableAuthSource {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    DurableAuthSource::new(pool, authority())
}

#[tokio::test]
async fn publisher_grant_workspace_configuration_is_inert_and_bounded() {
    let source = closed().await;
    for namespace in ["private_grants", &"a".repeat(63)] {
        for max in [1, 256] {
            assert!(source
                .file_publisher_grant_workspace(namespace, scope(), Uuid::new_v4(), max)
                .is_ok());
        }
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
                source.file_publisher_grant_workspace(namespace, scope(), Uuid::new_v4(), 16),
                Err(SessionFilePublisherGrantError::InvalidNamespace)
            ),
            "{namespace}"
        );
    }
    for identity in [Uuid::nil(), Uuid::from_u128(1)] {
        assert!(matches!(
            source.file_publisher_grant_workspace("private_grants", scope(), identity, 16),
            Err(SessionFilePublisherGrantError::Grant(
                FilePublisherGrantStoreError::InvalidInput
            ))
        ));
    }
    for max in [0, 257, u32::MAX] {
        assert!(matches!(
            source.file_publisher_grant_workspace("private_grants", scope(), Uuid::new_v4(), max),
            Err(SessionFilePublisherGrantError::Grant(
                FilePublisherGrantStoreError::InvalidInput
            ))
        ));
    }
}

#[tokio::test]
async fn publisher_grant_invalid_session_and_foreign_authority_fail_before_storage() {
    let source = closed().await;
    let workspace = source
        .file_publisher_grant_workspace("private_grants", scope(), Uuid::new_v4(), 16)
        .unwrap();
    for token in [
        "",
        "invalid",
        "00000000-0000-0000-0000-000000000000",
        "ABCDEF01-0000-4000-8000-000000000001",
    ] {
        assert_eq!(
            source
                .get_session_file_publisher_grant(token, &workspace, grant(1))
                .await,
            Err(SessionFilePublisherGrantError::Session(
                SessionCommandError::InvalidSession
            ))
        );
    }
    let mut foreign = scope();
    foreign.authority_id = id();
    let workspace = source
        .file_publisher_grant_workspace("private_grants", foreign, Uuid::new_v4(), 16)
        .unwrap();
    assert_eq!(
        source
            .get_session_file_publisher_grant(&Uuid::new_v4().to_string(), &workspace, grant(1))
            .await,
        Err(SessionFilePublisherGrantError::Session(
            SessionCommandError::Authentication(DurableAuthError::SourceMismatch)
        ))
    );
}

#[tokio::test]
async fn publisher_grant_valid_read_dispatches_to_the_explicit_closed_source() {
    let source = closed().await;
    let workspace = source
        .file_publisher_grant_workspace("private_grants", scope(), Uuid::new_v4(), 16)
        .unwrap();
    assert_eq!(
        source
            .get_session_file_publisher_grant(&Uuid::new_v4().to_string(), &workspace, grant(1))
            .await,
        Err(SessionFilePublisherGrantError::Session(
            SessionCommandError::Authentication(DurableAuthError::StorageUnavailable)
        ))
    );
}
