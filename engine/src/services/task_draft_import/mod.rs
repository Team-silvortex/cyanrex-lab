//! Bounded portable draft planning only; no publication, persistence or authorization.
//!
//! Local IDs and revisions are checked then discarded, never promoted to server coordinates.
//! Supplied Artifact snapshots prove only consistency with this plan, not their source or a
//! successful commit. The caller still needs current-Session commands. Separate publications
//! are not atomic with a later Task manifest write; failures do not authorize retries or cleanup.
use super::task_content::validate_text_payload_content;
use crate::models::collaboration::{
    ArtifactContent, ContractError, PrincipalRef, TaskContentManifest, TextPayloadBinding,
    WorkspaceRef,
};

mod parser;

pub const MAX_TASK_DRAFT_BYTES: usize = 8 * 1024 * 1024;

/// A validated publication plan, not a saved Task or an authorization receipt.
/// Deliberately not Debug/Serialize/Deserialize: parsing is bounded and content is not log output.
pub struct TaskDraftPublicationPlan {
    title: String,
    items: Vec<PlannedTextPublication>,
}

/// Ordered text and inert display labels. Contains no server or local editing identity.
pub struct PlannedTextPublication {
    filename: String,
    language: String,
    text: String,
}

impl PlannedTextPublication {
    pub fn filename(&self) -> &str {
        &self.filename
    }

    /// Unknown bounded language hints are preserved; they do not select or execute tooling.
    pub fn language(&self) -> &str {
        &self.language
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

impl TaskDraftPublicationPlan {
    /// Imports the portable format with stricter duplicate-field and integer-token parsing.
    /// A nonblank title is required here, unlike an unfinished local browser draft.
    pub fn parse_json(bytes: &[u8]) -> Result<Self, ContractError> {
        parser::parse(bytes)
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn items(&self) -> &[PlannedTextPublication] {
        &self.items
    }

    /// Creates data for a future authorized publication, without publishing or allocating IDs.
    /// The explicit Task title is used, not the filename (which has different label bounds).
    #[cfg(unix)]
    pub fn publication_draft(
        &self,
        index: usize,
    ) -> Result<super::artifact_store::ArtifactDraft, ContractError> {
        let item = self
            .items
            .get(index)
            .ok_or(ContractError("invalid publication item index"))?;
        super::artifact_store::ArtifactDraft::new(
            "cyanrex.task-text".parse()?,
            &self.title,
            "text/plain",
            item.text.as_bytes().to_vec(),
        )
        .map_err(|_| ContractError("invalid text publication draft"))
    }

    /// Binds every supplied snapshot by position after comparing exact bytes, owner, scope,
    /// declared length and recomputed digest. No partial manifest escapes on failure.
    /// Equal bytes may be bound to distinct refs in either order: this cannot prove provenance,
    /// prior publication order, membership, current heads or a confirmed storage outcome.
    /// Artifact title/kind/media are not filename/language; existing revisions remain reusable.
    pub fn bind_published(
        &self,
        scope: WorkspaceRef,
        owner: PrincipalRef,
        contents: &[ArtifactContent],
    ) -> Result<TaskContentManifest, ContractError> {
        if owner.authority_id != scope.authority_id || contents.len() != self.items.len() {
            return Err(ContractError(
                "publication snapshots must match the complete plan scope",
            ));
        }
        let mut bindings = Vec::with_capacity(self.items.len());
        for (item, content) in self.items.iter().zip(contents) {
            if content.revision.reference.workspace != scope
                || content.bytes != item.text.as_bytes()
            {
                return Err(ContractError(
                    "publication snapshot does not match the planned text",
                ));
            }
            let binding = TextPayloadBinding::new(
                content.revision.reference.clone(),
                &item.filename,
                &item.language,
            )?;
            validate_text_payload_content(&binding, owner, content)?;
            bindings.push(binding);
        }
        let manifest = TaskContentManifest::new(&self.title, bindings)?;
        let bytes = serde_json::to_vec(&manifest)
            .map_err(|_| ContractError("invalid publication manifest"))?;
        // Re-enter the bounded canonical server parser, independently of the 8 MiB import budget.
        TaskContentManifest::parse_json(&bytes)
    }
}
