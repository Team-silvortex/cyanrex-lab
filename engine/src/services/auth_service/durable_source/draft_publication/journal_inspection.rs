//! Current-authority journal metadata only, never resource confirmation or resumption.
use super::*;
use crate::models::collaboration::TaskId;
use crate::sqlx_compat as sqlx;

/// Private recorded progress; intentionally no Clone, Debug, Serde or dispatch conversion.
pub struct SessionDraftJournalInspection {
    intent: SessionDraftPublicationIntent,
    committed_steps: usize,
    unknown_step: Option<SessionDraftPublicationStep>,
}
impl SessionDraftJournalInspection {
    pub fn intent(&self) -> &SessionDraftPublicationIntent {
        &self.intent
    }
    /// Includes the final Task if recorded; not a receipt or current-resource health check.
    pub fn recorded_committed_step_count(&self) -> usize {
        self.committed_steps
    }
    pub fn unknown_step(&self) -> Option<&SessionDraftPublicationStep> {
        self.unknown_step.as_ref()
    }
    pub fn all_steps_recorded_committed(&self) -> bool {
        self.committed_steps == self.intent.checkpoint().manifest().payload().len() + 1
    }
}
impl DurableAuthSource {
    /// A separately authorized metadata read, including through a new Session for the same
    /// owner/account generation. None and Unknown never prove rollback or grant retry authority.
    pub async fn inspect_session_draft_journal(
        &self,
        token: &str,
        workspace: &SessionDraftIntentWorkspace,
        task: TaskId,
    ) -> Result<Option<SessionDraftJournalInspection>, SessionDraftIntentError> {
        if !valid_token(token) {
            return Err(super::super::SessionCommandError::InvalidSession.into());
        }
        intent::bounded(async {
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
            let observed = workspace
                .journal
                .inspect_in_transaction(&mut tx, reference, context.owner, context.account_id())
                .await?;
            let pending = if let Some((record, steps)) = observed {
                workspace.check_record_routes(&record, context.source_namespace())?;
                let intent = intent::loaded(record)?;
                let unknown = steps.last().filter(|step| !step.committed);
                let unknown_step = unknown.map(|step| {
                    match intent.checkpoint().manifest().payload().get(step.ordinal) {
                        Some(item) => SessionDraftPublicationStep::Artifact {
                            index: step.ordinal,
                            reference: item.artifact().clone(),
                        },
                        None => SessionDraftPublicationStep::Task {
                            reference: intent.task_ref(),
                        },
                    }
                });
                Some(SessionDraftJournalInspection {
                    committed_steps: steps.len() - usize::from(unknown.is_some()),
                    intent,
                    unknown_step,
                })
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
