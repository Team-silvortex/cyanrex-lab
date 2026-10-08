//! Immutable pre-dispatch intent registration only. Never restores or gates the old advance path.
use super::super::{private_work::PrivateWorkError, DurableAuthError};
use super::*;
use crate::services::auth_service::durable_source::SessionCommandError;
use crate::{
    models::collaboration::{LegacyAccountId, PrincipalRef, TaskId},
    services::draft_intent_journal::{DraftIntentJournal, DraftIntentJournalError},
};
use crate::{services::draft_intent_journal::DraftIntentRecord, sqlx_compat as sqlx};

#[derive(Clone)]
pub struct SessionDraftIntentWorkspace {
    pub(super) journal_namespace: String,
    pub(super) journal: DraftIntentJournal,
    pub(super) content: SessionTaskContentWorkspace,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SessionDraftIntentError {
    #[error(transparent)]
    Journal(#[from] DraftIntentJournalError),
    #[error(transparent)]
    Session(#[from] SessionCommandError),
    #[error("draft intent requires separate pinned namespaces")]
    InvalidNamespace,
    #[error("draft intent requires an undispatched original attempt")]
    InvalidAttempt,
    #[error("draft intent requires its original Session token")]
    SessionChanged,
}
/// Loaded private metadata, not a reusable access grant or acknowledgement of a resource write.
pub struct SessionDraftPublicationIntent {
    owner: PrincipalRef,
    account_id: LegacyAccountId,
    checkpoint: SessionDraftPublicationCheckpoint,
}
impl SessionDraftPublicationIntent {
    pub fn task_ref(&self) -> TaskRef {
        self.checkpoint.task_ref()
    }
    pub fn owner(&self) -> PrincipalRef {
        self.owner
    }
    pub fn account_id(&self) -> LegacyAccountId {
        self.account_id
    }
    pub fn checkpoint(&self) -> &SessionDraftPublicationCheckpoint {
        &self.checkpoint
    }
}
impl From<PrivateWorkError> for SessionDraftIntentError {
    fn from(error: PrivateWorkError) -> Self {
        match error {
            PrivateWorkError::Session(error) => Self::Session(error),
            PrivateWorkError::InvalidNamespace => Self::InvalidNamespace,
        }
    }
}
impl From<DurableAuthError> for SessionDraftIntentError {
    fn from(error: DurableAuthError) -> Self {
        Self::Session(error.into())
    }
}
impl From<sqlx::Error> for SessionDraftIntentError {
    fn from(_: sqlx::Error) -> Self {
        DraftIntentJournalError::StorageUnavailable.into()
    }
}
type IntentResult<T> = Result<T, SessionDraftIntentError>;
pub(super) async fn bounded<T>(
    operation: impl std::future::Future<Output = IntentResult<T>>,
) -> IntentResult<T> {
    // Cancellation/unknown commit never upgrades this intent to a dispatch or resource receipt.
    tokio::time::timeout(std::time::Duration::from_secs(10), operation)
        .await
        .map_err(|_| {
            SessionDraftIntentError::Journal(DraftIntentJournalError::StorageUnavailable)
        })?
}

pub(super) fn loaded(record: DraftIntentRecord) -> IntentResult<SessionDraftPublicationIntent> {
    Ok(SessionDraftPublicationIntent {
        owner: record.owner,
        account_id: record.account_id,
        checkpoint: record.validate()?,
    })
}
impl SessionDraftIntentWorkspace {
    pub fn new(
        namespace: &str,
        journal: DraftIntentJournal,
        content: SessionTaskContentWorkspace,
    ) -> Result<Self, SessionDraftIntentError> {
        if !crate::services::auth_service::durable_source::private_work::valid_namespace(namespace)
        {
            return Err(SessionDraftIntentError::InvalidNamespace);
        }
        if namespace == content.publication_namespace()
            || namespace == content.publication_artifacts().namespace
        {
            return Err(SessionDraftIntentError::InvalidNamespace);
        }
        if journal.scope != content.publication_scope() {
            return Err(DraftIntentJournalError::ScopeMismatch.into());
        }
        Ok(Self {
            journal_namespace: namespace.into(),
            journal,
            content,
        })
    }

    pub(super) fn matches_attempt(&self, attempt: &SessionTaskDraftPublication) -> bool {
        self.content.publication_scope() == attempt.task.workspace
            && self.content.publication_namespace() == attempt.workspace.publication_namespace()
            && self.content.publication_artifacts().namespace
                == attempt.workspace.publication_artifacts().namespace
    }

    pub(super) fn check_routes(&self, source_namespace: &str) -> IntentResult<()> {
        if source_namespace == self.content.publication_namespace()
            || source_namespace == self.content.publication_artifacts().namespace
        {
            return Err(SessionDraftIntentError::InvalidNamespace);
        }
        Ok(())
    }

    pub(super) fn check_record_routes(
        &self,
        record: &DraftIntentRecord,
        source_namespace: &str,
    ) -> IntentResult<()> {
        if record.source_namespace != source_namespace
            || record.task_namespace != self.content.publication_namespace()
            || record.artifact_namespace != self.content.publication_artifacts().namespace
        {
            return Err(DraftIntentJournalError::InvalidRecord.into());
        }
        Ok(())
    }
}
impl SessionTaskDraftPublication {
    /// Capture a pristine real attempt only. This does not mutate/gate advance or persist text.
    /// A duplicate is a conflict, never adopted as success, even after an uncertain commit.
    pub async fn register_intent(
        &self,
        token: &str,
        workspace: &SessionDraftIntentWorkspace,
    ) -> IntentResult<SessionDraftPublicationIntent> {
        if self.state != SessionDraftPublicationState::Ready
            || !self.confirmed.is_empty()
            || self.completed.is_some()
            || !workspace.matches_attempt(self)
        {
            return Err(SessionDraftIntentError::InvalidAttempt);
        }
        if !valid_token(token) || fingerprint(token) != self.fingerprint {
            return Err(SessionDraftIntentError::SessionChanged);
        }
        let checkpoint = self
            .checkpoint()
            .and_then(|checkpoint| checkpoint.to_json())
            .map_err(|_| SessionDraftIntentError::InvalidAttempt)?;
        bounded(async {
            let mut tx = self.source.transaction().await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'")
                .execute(&mut *tx)
                .await?;
            let context = self
                .source
                .begin_private_work(
                    &mut tx,
                    token,
                    &workspace.journal_namespace,
                    self.task.workspace,
                )
                .await?;
            workspace.check_routes(context.source_namespace())?;
            let record = DraftIntentRecord {
                task: self.task,
                owner: context.owner,
                account_id: context.account_id(),
                source_namespace: context.source_namespace().into(),
                task_namespace: self.workspace.publication_namespace().into(),
                artifact_namespace: self.workspace.publication_artifacts().namespace.clone(),
                checkpoint,
            };
            workspace
                .journal
                .insert_in_transaction(&mut tx, &record)
                .await?;
            let pending = loaded(record)?;
            // Reassert after journal triggers, before the final fresh Session check.
            sqlx::query("SET LOCAL synchronous_commit = 'on'")
                .execute(&mut *tx)
                .await?;
            context.finish(&self.source, &mut tx).await?;
            tx.commit().await?;
            Ok(pending)
        })
        .await
    }
}
impl DurableAuthSource {
    /// Current authorization of an immutable registered intent, not original Session continuity
    /// or a restored attempt. New Sessions require the same Principal AND account generation.
    pub async fn get_session_draft_intent(
        &self,
        token: &str,
        workspace: &SessionDraftIntentWorkspace,
        task: TaskId,
    ) -> IntentResult<Option<SessionDraftPublicationIntent>> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        bounded(async {
            let mut tx = self.transaction().await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'")
                .execute(&mut *tx)
                .await?;
            let context = self
                .begin_private_work(
                    &mut tx,
                    token,
                    &workspace.journal_namespace,
                    workspace.content.publication_scope(),
                )
                .await?;
            workspace.check_routes(context.source_namespace())?;
            let reference = TaskRef {
                workspace: workspace.content.publication_scope(),
                task_id: task,
            };
            let record = workspace
                .journal
                .read_in_transaction(&mut tx, reference, context.owner, context.account_id())
                .await?;
            let pending = if let Some(record) = record {
                workspace.check_record_routes(&record, context.source_namespace())?;
                Some(loaded(record)?)
            } else {
                None
            };
            context.finish(self, &mut tx).await?;
            tx.commit().await?;
            Ok(pending)
        })
        .await
    }
}
