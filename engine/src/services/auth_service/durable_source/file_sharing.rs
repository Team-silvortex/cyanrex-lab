//! Publisher-owned sharing of an exact stored publication. No download or byte permission.
use super::{
    private_work::{valid_namespace, PrivateWorkError},
    *,
};
use crate::{
    models::collaboration::{
        FileAccountRef, FileGrantId, FileGrantValidity, FileUploadId, FileVersionRef,
        RevisionNumber,
    },
    services::{
        file_publisher_grant_store::FilePublisherGrantStoreError,
        file_recipient_grant_store::{FileRecipientGrantStoreError, StoredFileRecipientGrant},
        file_upload_publication::{FileUploadPublicationStore, FileUploadPublicationStoreError},
    },
};

/// Inert trusted host configuration, never chosen by untrusted request fields.
#[derive(Clone)]
pub struct SessionFileSharingWorkspace {
    publication: FileUploadPublicationStore,
    publishers: SessionFilePublisherGrantWorkspace,
    recipients: SessionFileRecipientGrantWorkspace,
}
/// Exact selectors, not issuer attribution, a publication receipt or a permission token.
#[derive(Debug, Clone)]
pub struct SessionFileRecipientGrantIssue {
    pub grant_id: FileGrantId,
    pub publisher_grant_id: FileGrantId,
    pub upload_id: FileUploadId,
    pub expected_file: FileVersionRef,
    pub recipient: FileAccountRef,
    pub validity: FileGrantValidity,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SessionFileSharingError {
    #[error(transparent)]
    Session(#[from] SessionCommandError),
    #[error(transparent)]
    Publisher(#[from] FilePublisherGrantStoreError),
    #[error(transparent)]
    Recipient(#[from] FileRecipientGrantStoreError),
    #[error(transparent)]
    Publication(#[from] FileUploadPublicationStoreError),
    #[error("file sharing requires three distinct pinned non-system namespaces")]
    InvalidNamespace,
    #[error("current account cannot share or revoke this exact publisher's file")]
    PermissionDenied,
}
type SharingResult<T> = std::result::Result<T, SessionFileSharingError>;
impl From<PrivateWorkError> for SessionFileSharingError {
    fn from(error: PrivateWorkError) -> Self {
        match error {
            PrivateWorkError::Session(error) => Self::Session(error),
            PrivateWorkError::InvalidNamespace => Self::InvalidNamespace,
        }
    }
}
impl From<DurableAuthError> for SessionFileSharingError {
    fn from(error: DurableAuthError) -> Self {
        Self::Session(error.into())
    }
}
impl From<sqlx::Error> for SessionFileSharingError {
    fn from(_: sqlx::Error) -> Self {
        DurableAuthError::StorageUnavailable.into()
    }
}
impl DurableAuthSource {
    pub fn file_sharing_workspace(
        &self,
        publication: FileUploadPublicationStore,
        publishers: SessionFilePublisherGrantWorkspace,
        recipients: SessionFileRecipientGrantWorkspace,
    ) -> SharingResult<SessionFileSharingWorkspace> {
        let workspace = SessionFileSharingWorkspace {
            publication,
            publishers,
            recipients,
        };
        self.check_file_sharing_scope(&workspace)?;
        let names = [
            workspace.publication.namespace(),
            workspace.publishers.store.namespace(),
            workspace.recipients.store.namespace(),
        ];
        if names.iter().any(|n| !valid_namespace(n))
            || names
                .iter()
                .enumerate()
                .any(|(i, n)| names[..i].contains(n))
        {
            return Err(SessionFileSharingError::InvalidNamespace);
        }
        Ok(workspace)
    }
    /// Share only the current publisher's exact stored version with an active audited Human.
    /// All original records, target checks and final fresh permission share the source COMMIT.
    pub async fn issue_session_file_recipient_grant(
        &self,
        token: &str,
        workspace: &SessionFileSharingWorkspace,
        issue: &SessionFileRecipientGrantIssue,
    ) -> SharingResult<StoredFileRecipientGrant> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        self.check_file_sharing_scope(workspace)?;
        if issue.expected_file.workspace() != workspace.publication.scope()
            || issue.recipient.principal().authority_id != self.authority_id
        {
            return Err(FileRecipientGrantStoreError::ScopeMismatch.into());
        }
        sharing_sql(async {
            let mut tx = self.sharing_transaction().await?;
            let publishers = &workspace.publishers.store;
            let recipients = &workspace.recipients.store;
            let publication = &workspace.publication;
            let mut context = self
                .begin_private_work(&mut tx, token, publishers.namespace(), publishers.scope())
                .await?;
            let actor = FileAccountRef::new(context.owner, context.account_id());
            let grant = publishers
                .get_in_transaction(&mut tx, issue.publisher_grant_id, actor)
                .await?;
            if !publishers
                .permits_pending_in_transaction(&mut tx, &grant)
                .await?
            {
                return Err(SessionFileSharingError::PermissionDenied);
            }
            let window = grant
                .active_validity()
                .ok_or(SessionFileSharingError::PermissionDenied)?;
            context
                .select_target(&mut tx, publication.namespace())
                .await?;
            let published = publication
                .get_in_transaction(
                    &mut tx,
                    issue.upload_id,
                    actor.principal(),
                    actor.account_id(),
                )
                .await?;
            let record = published
                .record()
                .ok_or(SessionFileSharingError::PermissionDenied)?;
            let file = FileVersionRef::from_manifest(record.manifest());
            if record.owner() != actor.principal()
                || record.account_id() != actor.account_id()
                || file != issue.expected_file
            {
                return Err(SessionFileSharingError::PermissionDenied);
            }
            context
                .select_target(&mut tx, recipients.namespace())
                .await?;
            context
                .require_private_subject(
                    self,
                    &mut tx,
                    issue.recipient.principal(),
                    issue.recipient.account_id(),
                )
                .await?;
            let pending = recipients
                .issue_in_transaction(
                    &mut tx,
                    issue.grant_id,
                    actor,
                    file,
                    actor,
                    issue.recipient,
                    issue.validity,
                )
                .await?;
            sqlx::query("SET LOCAL synchronous_commit = on")
                .execute(&mut *tx)
                .await?;
            sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
                .execute(&mut *tx)
                .await?;
            recipients
                .verify_mutation_in_transaction(&mut tx, &pending)
                .await?;
            context
                .require_private_subject(
                    self,
                    &mut tx,
                    issue.recipient.principal(),
                    issue.recipient.account_id(),
                )
                .await?;
            recipients
                .verify_mutation_in_transaction(&mut tx, &pending)
                .await?;
            context
                .select_target(&mut tx, publication.namespace())
                .await?;
            publication
                .verify_pending_in_transaction(&mut tx, &published)
                .await?;
            context
                .select_target(&mut tx, publishers.namespace())
                .await?;
            publishers
                .verify_pending_in_transaction(&mut tx, &grant)
                .await?;
            sqlx::query("SET LOCAL synchronous_commit = on")
                .execute(&mut *tx)
                .await?;
            // Last clock sample covers the original publisher window and exact Session expiry
            // after every preceding SQL wait. No later SQL before requesting COMMIT.
            if !context
                .finish_in_window(self, &mut tx, window.not_before(), window.expires_at())
                .await?
            {
                return Err(SessionFileSharingError::PermissionDenied);
            }
            tx.commit().await?;
            Ok(pending.into_record())
        })
        .await
    }
    pub async fn revoke_session_file_recipient_grant(
        &self,
        token: &str,
        workspace: &SessionFileSharingWorkspace,
        id: FileGrantId,
        revision: RevisionNumber,
    ) -> SharingResult<StoredFileRecipientGrant> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        self.check_file_sharing_scope(workspace)?;
        sharing_sql(async {
            let mut tx = self.sharing_transaction().await?;
            let recipients = &workspace.recipients.store;
            let context = self
                .begin_private_work(&mut tx, token, recipients.namespace(), recipients.scope())
                .await?;
            let actor = FileAccountRef::new(context.owner, context.account_id());
            // Ownership is checked inside the original write fence, never by taking a read
            // SHARE fence and upgrading it. Issuer may differ; old recipients may be absent.
            let pending = recipients
                .revoke_owned_in_transaction(&mut tx, id, actor, revision)
                .await?
                .ok_or(SessionFileSharingError::PermissionDenied)?;
            sqlx::query("SET LOCAL synchronous_commit = on")
                .execute(&mut *tx)
                .await?;
            sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
                .execute(&mut *tx)
                .await?;
            recipients
                .verify_mutation_in_transaction(&mut tx, &pending)
                .await?;
            sqlx::query("SET LOCAL synchronous_commit = on")
                .execute(&mut *tx)
                .await?;
            // Publisher permission, Published existence and recipient lifecycle do not gate
            // stopping this current publisher's historical share.
            context.finish(self, &mut tx).await?;
            tx.commit().await?;
            Ok(pending.into_record())
        })
        .await
    }
    fn check_file_sharing_scope(
        &self,
        workspace: &SessionFileSharingWorkspace,
    ) -> SharingResult<()> {
        let scope = workspace.publication.scope();
        if scope.authority_id != self.authority_id {
            return Err(DurableAuthError::SourceMismatch.into());
        }
        if scope != workspace.publishers.store.scope() {
            return Err(FilePublisherGrantStoreError::ScopeMismatch.into());
        }
        if scope != workspace.recipients.store.scope() {
            return Err(FileRecipientGrantStoreError::ScopeMismatch.into());
        }
        Ok(())
    }

    async fn sharing_transaction(&self) -> SharingResult<Transaction<'_, Postgres>> {
        let mut tx = self.transaction().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
            .execute(&mut *tx)
            .await?;
        // Preserve configured READ ONLY policy; do not upgrade it for either command.
        sqlx::query("SET LOCAL synchronous_commit = on")
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }
}

async fn sharing_sql<T>(work: impl Future<Output = SharingResult<T>>) -> SharingResult<T> {
    tokio::time::timeout(Duration::from_secs(10), work)
        .await
        .map_err(|_| SessionFileSharingError::from(DurableAuthError::StorageUnavailable))?
}
