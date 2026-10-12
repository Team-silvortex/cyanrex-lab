//! Explicit trusted-host file-grant management; role labels never select a manager.
use super::{file_publisher_commands::GrantResult, *};
use crate::{
    models::collaboration::{FileAccountRef, FileGrantId, FileGrantValidity, RevisionNumber},
    services::file_publisher_grant_store::{
        PendingFilePublisherGrantMutation, StoredFilePublisherGrant,
    },
};

/// Fixed host policy, not a request value or persistent permission record.
/// Replacing a configuration does not invalidate its surviving clones or in-flight work.
#[derive(Clone)]
pub struct SessionFileGrantManagementWorkspace {
    publisher: SessionFilePublisherGrantWorkspace,
    manager: FileAccountRef,
}

impl DurableAuthSource {
    /// Inert host-only configuration. Never construct this from an untrusted request's manager.
    pub fn file_grant_management_workspace(
        &self,
        publisher: SessionFilePublisherGrantWorkspace,
        manager: FileAccountRef,
    ) -> GrantResult<SessionFileGrantManagementWorkspace> {
        if publisher.store.scope().authority_id != self.authority_id
            || manager.principal().authority_id != self.authority_id
        {
            return Err(DurableAuthError::SourceMismatch.into());
        }
        Ok(SessionFileGrantManagementWorkspace { publisher, manager })
    }

    pub async fn issue_session_file_publisher_grant(
        &self,
        token: &str,
        workspace: &SessionFileGrantManagementWorkspace,
        grant_id: FileGrantId,
        subject: FileAccountRef,
        validity: FileGrantValidity,
    ) -> GrantResult<StoredFilePublisherGrant> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        self.check_file_manager_scope(workspace)?;
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut tx = self.file_manager_transaction().await?;
            let store = &workspace.publisher.store;
            let context = self
                .begin_private_work(&mut tx, token, store.namespace(), store.scope())
                .await?;
            let actor = FileAccountRef::new(context.owner, context.account_id());
            if actor != workspace.manager {
                return Err(SessionFilePublisherGrantError::PermissionDenied);
            }
            context
                .require_private_subject(self, &mut tx, subject.principal(), subject.account_id())
                .await?;
            let pending: PendingFilePublisherGrantMutation = store
                .issue_in_transaction(&mut tx, grant_id, actor, subject, validity)
                .await?;
            // Deferred triggers can alter source, Registry or grant records in this transaction.
            sqlx::query("SET LOCAL synchronous_commit = on")
                .execute(&mut *tx)
                .await?;
            sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
                .execute(&mut *tx)
                .await?;
            store
                .verify_mutation_in_transaction(&mut tx, &pending)
                .await?;
            context
                .require_private_subject(self, &mut tx, subject.principal(), subject.account_id())
                .await?;
            store
                .verify_mutation_in_transaction(&mut tx, &pending)
                .await?;
            sqlx::query("SET LOCAL synchronous_commit = on")
                .execute(&mut *tx)
                .await?;
            context.finish(self, &mut tx).await?;
            tx.commit().await?;
            Ok(pending.into_record())
        })
        .await
        .map_err(|_| SessionFilePublisherGrantError::from(DurableAuthError::StorageUnavailable))?
    }

    pub async fn revoke_session_file_publisher_grant(
        &self,
        token: &str,
        workspace: &SessionFileGrantManagementWorkspace,
        grant_id: FileGrantId,
        expected_revision: RevisionNumber,
    ) -> GrantResult<StoredFilePublisherGrant> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        self.check_file_manager_scope(workspace)?;
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut tx = self.file_manager_transaction().await?;
            let store = &workspace.publisher.store;
            let context = self
                .begin_private_work(&mut tx, token, store.namespace(), store.scope())
                .await?;
            let actor = FileAccountRef::new(context.owner, context.account_id());
            if actor != workspace.manager {
                return Err(SessionFilePublisherGrantError::PermissionDenied);
            }
            // Old subjects may be retired or absent. Requiring them to be active would block revocation.
            let pending = store
                .revoke_in_transaction(&mut tx, grant_id, actor, expected_revision)
                .await?;
            sqlx::query("SET LOCAL synchronous_commit = on")
                .execute(&mut *tx)
                .await?;
            sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
                .execute(&mut *tx)
                .await?;
            store
                .verify_mutation_in_transaction(&mut tx, &pending)
                .await?;
            sqlx::query("SET LOCAL synchronous_commit = on")
                .execute(&mut *tx)
                .await?;
            context.finish(self, &mut tx).await?;
            tx.commit().await?;
            Ok(pending.into_record())
        })
        .await
        .map_err(|_| SessionFilePublisherGrantError::from(DurableAuthError::StorageUnavailable))?
    }

    fn check_file_manager_scope(
        &self,
        workspace: &SessionFileGrantManagementWorkspace,
    ) -> GrantResult<()> {
        if workspace.publisher.store.scope().authority_id != self.authority_id
            || workspace.manager.principal().authority_id != self.authority_id
        {
            return Err(DurableAuthError::SourceMismatch.into());
        }
        Ok(())
    }

    async fn file_manager_transaction(&self) -> GrantResult<Transaction<'_, Postgres>> {
        let mut tx = self.transaction().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
            .execute(&mut *tx)
            .await?;
        // Do not override a host's READ ONLY setting; writes then fail closed.
        sqlx::query("SET LOCAL synchronous_commit = on")
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }
}
