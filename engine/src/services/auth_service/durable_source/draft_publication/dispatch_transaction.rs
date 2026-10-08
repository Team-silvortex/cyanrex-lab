//! Two source-owned transactions; no detached authorization, retry or caller-built permit.
use super::super::{private_work::PrivateWorkError, DurableAuthError};
use super::*;
use crate::{
    models::collaboration::TaskContentManifest,
    services::{
        artifact_store::ArtifactDraft,
        draft_intent_journal::{DraftIntentJournalError, DraftIntentRecord},
    },
    sqlx_compat as sqlx,
};
use std::{future::Future, time::Duration};
use uuid::Uuid;

pub(super) enum DispatchOperation {
    Artifact(usize, ArtifactDraft),
    Task(TaskContentManifest),
}
pub(super) enum DispatchPending {
    Artifact(ArtifactRevision),
    Task(TaskContentSnapshot),
}
/// Created only after phase A COMMIT; consumed by one phase B call, never exported or cloned.
pub(super) struct DispatchPermit {
    record: DraftIntentRecord,
    ordinal: usize,
    nonce: Uuid,
}

impl From<DraftIntentJournalError> for SessionDraftDispatchError {
    fn from(error: DraftIntentJournalError) -> Self {
        Self::Intent(error.into())
    }
}
impl From<PrivateWorkError> for SessionDraftDispatchError {
    fn from(error: PrivateWorkError) -> Self {
        Self::Intent(error.into())
    }
}
impl From<DurableAuthError> for SessionDraftDispatchError {
    fn from(error: DurableAuthError) -> Self {
        Self::Intent(error.into())
    }
}
impl From<sqlx::Error> for SessionDraftDispatchError {
    fn from(_: sqlx::Error) -> Self {
        DraftIntentJournalError::StorageUnavailable.into()
    }
}
async fn bounded<T>(
    future: impl Future<Output = Result<T, SessionDraftDispatchError>>,
) -> Result<T, SessionDraftDispatchError> {
    tokio::time::timeout(Duration::from_secs(10), future)
        .await
        .map_err(|_| SessionDraftDispatchError::from(DraftIntentJournalError::StorageUnavailable))?
}

impl SessionJournaledTaskDraftPublication {
    pub(super) async fn reserve_dispatch(
        &self,
        token: &str,
        ordinal: usize,
    ) -> Result<DispatchPermit, SessionDraftDispatchError> {
        bounded(async {
            let source = &self.attempt.source;
            let mut tx = source.transaction().await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'")
                .execute(&mut *tx)
                .await?;
            let context = source
                .begin_private_work(
                    &mut tx,
                    token,
                    &self.workspace.journal_namespace,
                    self.attempt.task.workspace,
                )
                .await?;
            self.workspace.check_routes(context.source_namespace())?;
            let record = DraftIntentRecord {
                task: self.attempt.task,
                owner: context.owner,
                account_id: context.account_id(),
                source_namespace: context.source_namespace().into(),
                task_namespace: self.attempt.workspace.publication_namespace().into(),
                artifact_namespace: self
                    .attempt
                    .workspace
                    .publication_artifacts()
                    .namespace
                    .clone(),
                checkpoint: self.checkpoint.clone(),
            };
            let nonce = Uuid::new_v4();
            self.workspace
                .journal
                .reserve_step_in_transaction(&mut tx, &record, ordinal, nonce)
                .await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'")
                .execute(&mut *tx)
                .await?;
            context.finish(source, &mut tx).await?;
            tx.commit().await?;
            Ok(DispatchPermit {
                record,
                ordinal,
                nonce,
            })
        })
        .await
    }

    pub(super) async fn execute_dispatch(
        &self,
        token: &str,
        permit: DispatchPermit,
        operation: DispatchOperation,
    ) -> Result<DispatchPending, SessionDraftDispatchError> {
        bounded(async {
            let source = &self.attempt.source;
            let mut tx = source.transaction().await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'")
                .execute(&mut *tx)
                .await?;
            let mut context = source
                .begin_private_work(
                    &mut tx,
                    token,
                    &self.workspace.journal_namespace,
                    self.attempt.task.workspace,
                )
                .await?;
            // Phase A cannot transfer an ownership decision to a changed account or source.
            if context.owner != permit.record.owner
                || context.account_id() != permit.record.account_id
                || context.source_namespace() != permit.record.source_namespace
            {
                return Err(DraftIntentJournalError::InvalidRecord.into());
            }
            self.workspace.check_routes(context.source_namespace())?;
            let relations = self
                .workspace
                .journal
                .begin_step_in_transaction(&mut tx, &permit.record, permit.ordinal, permit.nonce)
                .await?;
            // The pending marker UPDATE/its triggers precede complete resource verification.
            // Failure rolls it back to A's already-committed Unknown; files are not transactional.
            let pending = match operation {
                DispatchOperation::Artifact(index, draft) => {
                    if index != permit.ordinal {
                        return Err(DraftIntentJournalError::InvalidInput.into());
                    }
                    let artifacts = self.attempt.workspace.publication_artifacts();
                    context.select_target(&mut tx, &artifacts.namespace).await?;
                    let target = &self.attempt.allocated[index];
                    let revision = artifacts
                        .store
                        .create_in_transaction(
                            &mut tx,
                            target.artifact_id,
                            target.revision_id,
                            context.owner,
                            draft,
                        )
                        .await
                        .map_err(SessionArtifactError::from)?;
                    if revision.owner != permit.record.owner
                        || !self
                            .attempt
                            .artifact_matches_planned_create(index, &revision)
                    {
                        return Err(SessionDraftPublicationError::InvalidConfirmation.into());
                    }
                    DispatchPending::Artifact(revision)
                }
                DispatchOperation::Task(manifest) => {
                    if permit.ordinal != self.attempt.allocated.len() {
                        return Err(DraftIntentJournalError::InvalidInput.into());
                    }
                    context
                        .select_target(&mut tx, self.attempt.workspace.publication_namespace())
                        .await?;
                    let expected = manifest.clone();
                    let snapshot = source
                        .create_content_task_in_transaction(
                            &mut tx,
                            &mut context,
                            &self.attempt.workspace,
                            self.attempt.task.task_id,
                            manifest,
                        )
                        .await?;
                    if snapshot.task.owner != permit.record.owner
                        || !self
                            .attempt
                            .task_matches_planned_create(&expected, &snapshot)
                    {
                        return Err(SessionDraftPublicationError::InvalidConfirmation.into());
                    }
                    DispatchPending::Task(snapshot)
                }
            };
            context
                .select_target(&mut tx, &self.workspace.journal_namespace)
                .await?;
            // No further business/journal DML after complete resource checks.
            self.workspace
                .journal
                .verify_step_in_transaction(
                    &mut tx,
                    &permit.record,
                    permit.ordinal,
                    permit.nonce,
                    &relations,
                )
                .await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'")
                .execute(&mut *tx)
                .await?;
            context.finish(source, &mut tx).await?;
            tx.commit().await?;
            Ok(pending)
        })
        .await
    }
}
