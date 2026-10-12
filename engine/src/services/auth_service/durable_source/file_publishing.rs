//! Explicit Session-authorized publication of a trusted, opaque prepared attempt.
//! No SQL fence spans file work; final insert and current permission share one source transaction.
use super::{
    private_work::{valid_namespace, PrivateWorkError},
    *,
};
use crate::{
    models::collaboration::{FileAccountRef, FileGrantId},
    services::{
        file_publisher_grant_store::FilePublisherGrantStoreError,
        file_upload_staging::{
            FileUploadPublicationAttempt, FileUploadPublicationError, FileUploadPublicationReceipt,
            FileUploadPublisher,
        },
    },
};

/// Trusted host configuration. Request data cannot select namespaces, roots, scopes or an owner.
#[derive(Clone)]
pub struct SessionFilePublishingWorkspace {
    publisher: FileUploadPublisher,
    grants: SessionFilePublisherGrantWorkspace,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SessionFilePublishingError {
    #[error(transparent)]
    Session(#[from] SessionCommandError),
    #[error(transparent)]
    Grant(#[from] FilePublisherGrantStoreError),
    #[error(transparent)]
    Publisher(#[from] FileUploadPublicationError),
    #[error("file publishing requires distinct pinned non-system namespaces")]
    InvalidNamespace,
    #[error("current account lacks file publishing permission")]
    PermissionDenied,
}
type PublishingResult<T> = std::result::Result<T, SessionFilePublishingError>;
impl From<PrivateWorkError> for SessionFilePublishingError {
    fn from(error: PrivateWorkError) -> Self {
        match error {
            PrivateWorkError::Session(error) => Self::Session(error),
            PrivateWorkError::InvalidNamespace => Self::InvalidNamespace,
        }
    }
}
impl From<DurableAuthError> for SessionFilePublishingError {
    fn from(error: DurableAuthError) -> Self {
        Self::Session(error.into())
    }
}
impl From<sqlx::Error> for SessionFilePublishingError {
    fn from(_: sqlx::Error) -> Self {
        DurableAuthError::StorageUnavailable.into()
    }
}

impl DurableAuthSource {
    /// Freeze an existing publisher and permission ledger. No I/O, installation or role mapping.
    pub fn file_publishing_workspace(
        &self,
        publisher: FileUploadPublisher,
        grants: SessionFilePublisherGrantWorkspace,
    ) -> PublishingResult<SessionFilePublishingWorkspace> {
        if publisher.scope().authority_id != self.authority_id {
            return Err(DurableAuthError::SourceMismatch.into());
        }
        if publisher.scope() != grants.store.scope() {
            return Err(FilePublisherGrantStoreError::ScopeMismatch.into());
        }
        let names = [
            publisher.upload_namespace(),
            publisher.journal_namespace(),
            publisher.publication_namespace(),
            grants.store.namespace(),
        ];
        if names.iter().any(|name| !valid_namespace(name))
            || names
                .iter()
                .enumerate()
                .any(|(index, name)| names[..index].contains(name))
        {
            return Err(SessionFilePublishingError::InvalidNamespace);
        }
        Ok(SessionFilePublishingWorkspace { publisher, grants })
    }

    /// A trusted host prepares the opaque attempt separately, without claiming Session authority.
    /// This command derives the exact actor, checks a current grant before any file read, releases
    /// SQL for fresh bytes, then rechecks ORIGINAL source/Registry/grant pins in a fresh final Tx.
    /// Every admitted outcome is terminal. Timeout/cancellation/lost ACK is not rollback or retry.
    pub async fn publish_session_file_upload(
        &self,
        token: &str,
        workspace: &SessionFilePublishingWorkspace,
        grant_id: FileGrantId,
        attempt: &mut FileUploadPublicationAttempt,
    ) -> PublishingResult<FileUploadPublicationReceipt> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        if workspace.publisher.scope().authority_id != self.authority_id {
            return Err(DurableAuthError::SourceMismatch.into());
        }
        let publisher = &workspace.publisher;
        let store = &workspace.grants.store;
        let run = publisher.start(attempt)?;
        let (mut context, grant) = publication_sql(async {
            let mut tx = self.publication_transaction().await?;
            let context = self
                .begin_private_work(&mut tx, token, store.namespace(), store.scope())
                .await?;
            let subject = FileAccountRef::new(context.owner, context.account_id());
            if subject != FileAccountRef::new(run.owner(), run.account_id()) {
                return Err(SessionFilePublishingError::PermissionDenied);
            }
            let grant = store.get_in_transaction(&mut tx, grant_id, subject).await?;
            if !store
                .permits_pending_in_transaction(&mut tx, &grant)
                .await?
            {
                return Err(SessionFilePublishingError::PermissionDenied);
            }
            context.finish_retained(self, &mut tx).await?;
            tx.commit().await?;
            Ok((context, grant))
        })
        .await?;

        // No source connection or SQL fence remains held while queued/running file work executes.
        let observed = publisher.observe(run).await?;
        if observed.scope() != store.scope()
            || FileAccountRef::new(observed.owner(), observed.account_id())
                != FileAccountRef::new(context.owner, context.account_id())
        {
            return Err(SessionFilePublishingError::PermissionDenied);
        }
        let pending = publication_sql(async {
            let mut tx = self.publication_transaction().await?;
            context.resume(self, &mut tx).await?;
            store
                .refence_pending_in_transaction(&mut tx, &grant)
                .await?;
            if !store
                .permits_pending_in_transaction(&mut tx, &grant)
                .await?
            {
                return Err(SessionFilePublishingError::PermissionDenied);
            }
            let validity = grant
                .active_validity()
                .ok_or(SessionFilePublishingError::PermissionDenied)?;
            context
                .select_target(&mut tx, observed.upload_namespace())
                .await?;
            publisher
                .verify_source_in_transaction(&mut tx, &observed)
                .await?;
            context
                .select_target(&mut tx, observed.journal_namespace())
                .await?;
            publisher
                .verify_journal_in_transaction(&mut tx, &observed)
                .await?;
            context
                .select_target(&mut tx, observed.publication_namespace())
                .await?;
            let pending = publisher.publish_in_transaction(&mut tx, &observed).await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'")
                .execute(&mut *tx)
                .await?;
            sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
                .execute(&mut *tx)
                .await?;
            publisher
                .verify_publication_in_transaction(&mut tx, &pending)
                .await?;
            context
                .select_target(&mut tx, observed.upload_namespace())
                .await?;
            publisher
                .verify_source_in_transaction(&mut tx, &observed)
                .await?;
            context
                .select_target(&mut tx, observed.journal_namespace())
                .await?;
            publisher
                .verify_journal_in_transaction(&mut tx, &observed)
                .await?;
            context
                .select_target(&mut tx, observed.publication_namespace())
                .await?;
            publisher
                .verify_publication_in_transaction(&mut tx, &pending)
                .await?;
            context.select_target(&mut tx, store.namespace()).await?;
            store.verify_pending_in_transaction(&mut tx, &grant).await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'")
                .execute(&mut *tx)
                .await?;
            // Last clock sample covers the grant AND Session after every target/policy/row wait.
            if !context
                .finish_in_window(self, &mut tx, validity.not_before(), validity.expires_at())
                .await?
            {
                return Err(SessionFilePublishingError::PermissionDenied);
            }
            tx.commit().await?;
            Ok(pending)
        })
        .await?;
        // Only a confirmed source COMMIT can mint the private publication ACK. Original-byte and
        // metadata checks still run afterwards; failure can retain a row with an Unknown attempt.
        Ok(publisher.finish(observed, pending).await?)
    }

    async fn publication_transaction(&self) -> PublishingResult<Transaction<'_, Postgres>> {
        let mut tx = self.transaction().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
            .execute(&mut *tx)
            .await?;
        sqlx::query("SET LOCAL synchronous_commit = 'on'")
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }
}

async fn publication_sql<T>(
    operation: impl Future<Output = PublishingResult<T>>,
) -> PublishingResult<T> {
    tokio::time::timeout(Duration::from_secs(10), operation)
        .await
        .map_err(|_| SessionFilePublishingError::from(DurableAuthError::StorageUnavailable))?
}
