//! One borrowed private index read. Neither phase confirms COMMIT or grants later access.
use super::{private_work::PrivateWorkContext, *};
use crate::{
    models::collaboration::TaskRef,
    services::task_store::{
        read_content_index_in_transaction, validate_index_page_request, TaskContentIndexPage,
    },
};

/// Pending metadata and its original guard only; no pool, bytes, serialization or reusable grant.
pub(crate) struct PreparedTaskIndexRead {
    authority_id: AuthorityId,
    context: PrivateWorkContext,
    pending: TaskContentIndexPage,
}

impl PreparedTaskIndexRead {
    pub(crate) fn validate(
        authority_id: AuthorityId,
        token: &str,
        workspace: &SessionTaskContentIndexWorkspace,
        after: Option<TaskRef>,
        limit: Option<usize>,
    ) -> std::result::Result<(), SessionTaskError> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        let (_, scope, _) = workspace.transaction_config();
        validate_index_page_request(scope, after, limit)?;
        if scope.authority_id != authority_id {
            return Err(DurableAuthError::SourceMismatch.into());
        }
        Ok(())
    }

    /// Read under the current Session's owner and restore the original Source before a host's
    /// retained-storage checks. Those checks must precede finish's final fresh authorization.
    pub(crate) async fn prepare(
        authority_id: AuthorityId,
        tx: &mut Transaction<'_, Postgres>,
        token: &str,
        workspace: &SessionTaskContentIndexWorkspace,
        after: Option<TaskRef>,
        limit: Option<usize>,
    ) -> std::result::Result<Self, SessionTaskError> {
        Self::validate(authority_id, token, workspace, after, limit)?;
        let (namespace, scope, definitions) = workspace.transaction_config();
        let context =
            PrivateWorkContext::begin_index_for(authority_id, tx, token, namespace, scope).await?;
        let pending =
            read_content_index_in_transaction(tx, scope, definitions, context.owner, after, limit)
                .await?;
        context.finish_retained_for(authority_id, tx).await?;
        Ok(Self {
            authority_id,
            context,
            pending,
        })
    }

    /// All host guard waits must be complete. Recheck original pins, account/membership and the
    /// exact current Session with fresh expiry; the returned page stays pending until COMMIT.
    pub(crate) async fn finish(
        self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> std::result::Result<TaskContentIndexPage, SessionTaskError> {
        self.context
            .finish_from_source_for(self.authority_id, tx)
            .await?;
        Ok(self.pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        models::collaboration::{TaskId, WorkspaceId, WorkspaceRef},
        services::task_store::TaskStoreError,
    };

    const TOKEN: &str = "00000000-0000-4000-8000-000000000001";

    fn scope() -> WorkspaceRef {
        WorkspaceRef {
            authority_id: "00000000-0000-4000-8000-000000000002".parse().unwrap(),
            workspace_id: "00000000-0000-4000-8000-000000000003".parse().unwrap(),
        }
    }

    #[test]
    fn index_preparation_rejects_bad_credentials_pagination_and_authority_without_a_pool() {
        let scope = scope();
        let workspace = SessionTaskContentIndexWorkspace::new("private_tasks", scope).unwrap();
        let other = "00000000-0000-4000-8000-000000000004".parse().unwrap();
        let validate =
            |token, limit| PreparedTaskIndexRead::validate(other, token, &workspace, None, limit);
        assert_eq!(
            validate("invalid", Some(0)),
            Err(SessionCommandError::InvalidSession.into())
        );
        assert_eq!(
            validate(TOKEN, Some(0)),
            Err(TaskStoreError::InvalidInput.into())
        );
        assert_eq!(
            validate(TOKEN, None),
            Err(DurableAuthError::SourceMismatch.into())
        );
        for limit in [None, Some(1), Some(20), Some(32)] {
            assert_eq!(
                PreparedTaskIndexRead::validate(scope.authority_id, TOKEN, &workspace, None, limit),
                Ok(())
            );
        }
    }

    #[test]
    fn index_cursor_is_only_a_position_inside_the_configured_scope() {
        let scope = scope();
        let workspace = SessionTaskContentIndexWorkspace::new("private_tasks", scope).unwrap();
        let cursor = TaskRef {
            workspace: scope,
            task_id: TaskId::try_from(uuid::Uuid::new_v4()).unwrap(),
        };
        assert_eq!(
            PreparedTaskIndexRead::validate(
                scope.authority_id,
                TOKEN,
                &workspace,
                Some(cursor),
                None
            ),
            Ok(())
        );
        let foreign = TaskRef {
            workspace: WorkspaceRef {
                workspace_id: WorkspaceId::try_from(uuid::Uuid::new_v4()).unwrap(),
                ..scope
            },
            ..cursor
        };
        assert_eq!(
            PreparedTaskIndexRead::validate(
                scope.authority_id,
                TOKEN,
                &workspace,
                Some(foreign),
                None
            ),
            Err(TaskStoreError::ScopeMismatch.into())
        );
    }
}
