use super::fixture::*;
use super::*;

#[tokio::test]
async fn session_content_index_valid_request_dispatches_to_the_explicit_closed_source() {
    let source = unavailable_source().await;
    let workspace = SessionTaskContentIndexWorkspace::new("private_index", scope()).unwrap();
    let expected = Err(SessionTaskError::Session(
        SessionCommandError::Authentication(DurableAuthError::StorageUnavailable),
    ));
    for limit in [None, Some(1), Some(20), Some(32)] {
        assert_eq!(
            source
                .list_session_content_tasks(&Uuid::new_v4().to_string(), &workspace, None, limit)
                .await,
            expected,
            "valid index shape must reach this source, never a placeholder or empty success"
        );
    }
}

#[tokio::test]
async fn session_content_index_configuration_accepts_only_exact_non_system_namespaces() {
    let definitions = catalog(&values());
    for namespace in ["private_index", &"a".repeat(63)] {
        assert!(SessionTaskContentIndexWorkspace::new(namespace, scope()).is_ok());
        assert!(SessionTaskContentIndexWorkspace::from_definitions(
            namespace,
            scope(),
            &definitions
        )
        .is_ok());
    }
    for namespace in [
        "",
        "public",
        "information_schema",
        "pg_catalog",
        "pg_private",
        "a.b",
        "has space",
        "Uppercase",
        "1prefix",
        "quoted\"",
        &"a".repeat(64),
    ] {
        for result in [
            SessionTaskContentIndexWorkspace::new(namespace, scope()),
            SessionTaskContentIndexWorkspace::from_definitions(namespace, scope(), &definitions),
        ] {
            assert!(
                matches!(result, Err(SessionTaskError::InvalidNamespace)),
                "{namespace}"
            );
        }
    }
}

#[tokio::test]
async fn session_content_index_invalid_token_limit_and_cursor_are_rejected_before_io() {
    let source = unavailable_source().await;
    let workspace = SessionTaskContentIndexWorkspace::new("private_index", scope()).unwrap();
    for token in ["", "invalid", "00000000-0000-0000-0000-000000000000"] {
        assert_eq!(
            source
                .list_session_content_tasks(token, &workspace, None, None)
                .await,
            Err(SessionTaskError::Session(
                SessionCommandError::InvalidSession
            ))
        );
    }
    let token = Uuid::new_v4().to_string();
    for limit in [0, 33, usize::MAX] {
        assert_eq!(
            source
                .list_session_content_tasks(&token, &workspace, None, Some(limit))
                .await,
            Err(SessionTaskError::Task(TaskStoreError::InvalidInput))
        );
    }
    for other in [
        WorkspaceRef {
            workspace_id: id(),
            ..scope()
        },
        WorkspaceRef {
            authority_id: id(),
            ..scope()
        },
    ] {
        let cursor = TaskRef {
            workspace: other,
            task_id: id(),
        };
        assert_eq!(
            source
                .list_session_content_tasks(&token, &workspace, Some(cursor), None)
                .await,
            Err(SessionTaskError::Task(TaskStoreError::ScopeMismatch))
        );
    }
}

#[tokio::test]
async fn session_content_index_foreign_source_authority_is_rejected_before_io() {
    let source = unavailable_source().await;
    let workspace = SessionTaskContentIndexWorkspace::new(
        "private_index",
        WorkspaceRef {
            authority_id: id(),
            ..scope()
        },
    )
    .unwrap();
    assert_eq!(
        source
            .list_session_content_tasks(&Uuid::new_v4().to_string(), &workspace, None, None)
            .await,
        Err(SessionTaskError::Session(
            SessionCommandError::Authentication(DurableAuthError::SourceMismatch,)
        ))
    );
}
