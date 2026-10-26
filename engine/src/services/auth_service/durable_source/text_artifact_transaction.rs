//! Retained text publication; original authority and pending data stay inside one transaction.
use super::{
    private_work::{valid_namespace, PrivateWorkContext},
    *,
};
use crate::{
    models::collaboration::{ArtifactId, ArtifactRevision, ArtifactRevisionId, WorkspaceRef},
    services::{
        artifact_store::{ArtifactContentWriter, ArtifactDraft, ArtifactStoreError},
        task_content::validate_text_bytes,
    },
};

type Result<T> = std::result::Result<T, SessionArtifactError>;

pub(crate) struct TextArtifactPublishWorkspace {
    namespace: String,
    scope: WorkspaceRef,
    writer: ArtifactContentWriter,
}

impl TextArtifactPublishWorkspace {
    pub(crate) fn new(
        namespace: &str,
        scope: WorkspaceRef,
        writer: ArtifactContentWriter,
    ) -> Result<Self> {
        if !valid_namespace(namespace) {
            return Err(SessionArtifactError::InvalidNamespace);
        }
        if writer.scope() != scope {
            return Err(ArtifactStoreError::ScopeMismatch.into());
        }
        Ok(Self {
            namespace: namespace.into(),
            scope,
            writer,
        })
    }

    /// Pure validation before connecting. A title or text never selects an owner or path.
    pub(crate) fn draft(title: String, text: String) -> Result<ArtifactDraft> {
        validate_text_bytes(text.as_bytes()).map_err(|_| ArtifactStoreError::InvalidInput)?;
        Ok(ArtifactDraft::new(
            "cyanrex.task-text"
                .parse()
                .map_err(|_| ArtifactStoreError::InvalidInput)?,
            title,
            "text/plain",
            text.into_bytes(),
        )?)
    }

    pub(crate) async fn verify_root(&self) -> Result<()> {
        Ok(self.writer.verify_root().await?)
    }
}

/// Not a receipt, permission token, retry handle or independently committable operation.
pub(crate) struct PreparedTextArtifactPublish {
    authority_id: AuthorityId,
    context: PrivateWorkContext,
    pending: ArtifactRevision,
}

impl PreparedTextArtifactPublish {
    pub(crate) async fn prepare(
        authority_id: AuthorityId,
        tx: &mut Transaction<'_, Postgres>,
        token: &str,
        workspace: &TextArtifactPublishWorkspace,
        artifact: ArtifactId,
        revision: ArtifactRevisionId,
        draft: ArtifactDraft,
    ) -> Result<Self> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        if authority_id != workspace.scope.authority_id {
            return Err(DurableAuthError::SourceMismatch.into());
        }
        let context = PrivateWorkContext::begin_index_for(
            authority_id,
            tx,
            token,
            &workspace.namespace,
            workspace.scope,
        )
        .await?;
        let pending = workspace
            .writer
            .create_in_transaction(tx, artifact, revision, context.owner, draft)
            .await?;
        // A deferred trigger can replace a perfectly valid head. Exact historical readability
        // is not confirmation of this create: compare the full current head and actual bytes.
        sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
            .execute(&mut **tx)
            .await?;
        workspace.writer.verify_in_transaction(tx, &pending).await?;
        context.finish_retained_for(authority_id, tx).await?;
        Ok(Self {
            authority_id,
            context,
            pending,
        })
    }

    pub(crate) async fn finish(
        self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<ArtifactRevision> {
        self.context
            .finish_from_source_for(self.authority_id, tx)
            .await?;
        Ok(self.pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_profile_accepts_empty_notes_unicode_and_exact_utf8_byte_limit() {
        for text in [
            "".into(),
            "讨论记录😀\n不需要代码\t或 Run\r\n".into(),
            "文".repeat(87381) + "x",
        ] {
            assert!(TextArtifactPublishWorkspace::draft("讨论笔记".into(), text).is_ok());
        }
    }

    #[test]
    fn text_profile_rejects_oversize_controls_and_invalid_titles_before_io() {
        for text in [
            "文".repeat(87382),
            "x".repeat(262145),
            "notes\0".into(),
            "notes\u{7f}".into(),
        ] {
            assert!(matches!(
                TextArtifactPublishWorkspace::draft("Notes".into(), text),
                Err(SessionArtifactError::Artifact(
                    ArtifactStoreError::InvalidInput
                ))
            ));
        }
        for title in ["".into(), " \n".into(), "文".repeat(86), "title\t".into()] {
            assert!(matches!(
                TextArtifactPublishWorkspace::draft(title, "".into()),
                Err(SessionArtifactError::Artifact(
                    ArtifactStoreError::InvalidInput
                ))
            ));
        }
    }
}
