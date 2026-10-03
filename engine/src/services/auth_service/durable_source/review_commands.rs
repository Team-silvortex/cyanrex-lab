//! Session-owned private human opinions, not cross-user review authority or Task acceptance.
use super::{
    private_work::{valid_namespace, PrivateWorkError},
    *,
};
use crate::{
    models::collaboration::{
        ArtifactRef, PrincipalRef, ReviewId, ReviewJudgment, ReviewRecord, ReviewRef,
        VersionedName, WorkspaceRef,
    },
    services::{
        artifact_store::ArtifactStoreError,
        review_store::{HumanReviewEdit, ReviewDraft, ReviewStore, ReviewStoreError},
    },
};

/// Trusted configuration only. The policy pin labels an opinion; it does not install a policy.
#[derive(Clone)]
pub struct SessionReviewWorkspace {
    namespace: String,
    scope: WorkspaceRef,
    artifacts: SessionArtifactWorkspace,
    policy: VersionedName,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SessionReviewError {
    #[error(transparent)]
    Session(#[from] SessionCommandError),
    #[error(transparent)]
    Review(#[from] ReviewStoreError),
    #[error(transparent)]
    Artifact(#[from] ArtifactStoreError),
    #[error("session reviews require separate pinned non-system namespaces")]
    InvalidNamespace,
    #[error("only private human opinions with the configured policy are supported")]
    UnsupportedReview,
}
type ReviewResult<T> = std::result::Result<T, SessionReviewError>;
impl From<PrivateWorkError> for SessionReviewError {
    fn from(error: PrivateWorkError) -> Self {
        match error {
            PrivateWorkError::Session(error) => Self::Session(error),
            PrivateWorkError::InvalidNamespace => Self::InvalidNamespace,
        }
    }
}
impl From<DurableAuthError> for SessionReviewError {
    fn from(error: DurableAuthError) -> Self {
        Self::Session(error.into())
    }
}
impl From<sqlx::Error> for SessionReviewError {
    fn from(_: sqlx::Error) -> Self {
        DurableAuthError::StorageUnavailable.into()
    }
}

impl SessionReviewWorkspace {
    pub fn new(
        namespace: &str,
        scope: WorkspaceRef,
        artifacts: SessionArtifactWorkspace,
        policy: VersionedName,
    ) -> ReviewResult<Self> {
        if !valid_namespace(namespace) || namespace == artifacts.namespace {
            return Err(SessionReviewError::InvalidNamespace);
        }
        if scope != artifacts.scope {
            return Err(ReviewStoreError::ScopeMismatch.into());
        }
        Ok(Self {
            namespace: namespace.into(),
            scope,
            artifacts,
            policy,
        })
    }

    fn require_human(&self, record: &ReviewRecord) -> ReviewResult<()> {
        match &record.judgment {
            ReviewJudgment::Human { policy, .. } if *policy == self.policy => Ok(()),
            _ => Err(SessionReviewError::UnsupportedReview),
        }
    }

    /// Both lists share one global head-lock order. Exact cross-list duplicates are legal.
    /// Discard each verified blob before reading the next; none is exposed as a detached grant.
    async fn verify_references(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        owner: PrincipalRef,
        references: &[ArtifactRef],
    ) -> ReviewResult<()> {
        let mut ordered: Vec<_> = references.iter().collect();
        ordered.sort_by_key(|reference| {
            (
                reference.artifact_id.as_uuid(),
                reference.revision_id.as_uuid(),
            )
        });
        ordered.dedup();
        for reference in ordered {
            self.artifacts
                .store
                .read_in_transaction(tx, reference, owner)
                .await?
                .ok_or(ArtifactStoreError::NotFound)?;
        }
        Ok(())
    }
}

enum Operation {
    Create(ReviewDraft, Vec<ArtifactRef>),
    Read(ReviewRef),
    Revise(ReviewRef, HumanReviewEdit),
}

impl DurableAuthSource {
    /// The Session supplies reviewer and human attribution; no domain or teaching grant is implied.
    pub async fn create_session_review(
        &self,
        token: &str,
        workspace: &SessionReviewWorkspace,
        id: ReviewId,
        targets: Vec<ArtifactRef>,
        evidence: Vec<ArtifactRef>,
        edit: HumanReviewEdit,
    ) -> ReviewResult<ReviewRecord> {
        // Bound the copies needed for post-write checks before allocating them.
        if targets.is_empty() || targets.len() > 32 || evidence.len() > 32 {
            return Err(ReviewStoreError::InvalidInput.into());
        }
        let references: Vec<_> = targets.iter().chain(&evidence).cloned().collect();
        let draft = ReviewDraft::human(targets, evidence, workspace.policy.clone(), edit)?;
        if references
            .iter()
            .any(|item| item.workspace != workspace.scope)
        {
            return Err(ReviewStoreError::ScopeMismatch.into());
        }
        self.session_review(token, workspace, id, Operation::Create(draft, references))
            .await?
            .ok_or(ReviewStoreError::InvalidRecord.into())
    }

    /// Exact history, not a current approval or evidence grant for a later command.
    pub async fn read_session_review(
        &self,
        token: &str,
        workspace: &SessionReviewWorkspace,
        reference: ReviewRef,
    ) -> ReviewResult<Option<ReviewRecord>> {
        if reference.workspace != workspace.scope {
            return Err(ReviewStoreError::ScopeMismatch.into());
        }
        self.session_review(
            token,
            workspace,
            reference.review_id,
            Operation::Read(reference),
        )
        .await
    }

    pub async fn revise_session_review(
        &self,
        token: &str,
        workspace: &SessionReviewWorkspace,
        expected: ReviewRef,
        edit: HumanReviewEdit,
    ) -> ReviewResult<ReviewRecord> {
        if expected.workspace != workspace.scope {
            return Err(ReviewStoreError::ScopeMismatch.into());
        }
        self.session_review(
            token,
            workspace,
            expected.review_id,
            Operation::Revise(expected, edit),
        )
        .await?
        .ok_or(ReviewStoreError::InvalidRecord.into())
    }

    async fn session_review(
        &self,
        token: &str,
        workspace: &SessionReviewWorkspace,
        id: ReviewId,
        operation: Operation,
    ) -> ReviewResult<Option<ReviewRecord>> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut tx = self.transaction().await?;
            let mut context = self
                .begin_private_work(&mut tx, token, &workspace.namespace, workspace.scope)
                .await?;
            let owner = context.owner;
            let store = ReviewStore::new(self.pool.clone(), workspace.scope);
            // Review metadata/head precede every Artifact head, including exact historical reads.
            let head = store.head_in_transaction(&mut tx, id, owner).await?;
            let mut selected = None;
            let references = match &operation {
                Operation::Create(_, references) => {
                    if head.is_some() {
                        return Err(ReviewStoreError::Conflict.into());
                    }
                    references.clone()
                }
                Operation::Read(reference) => {
                    selected = store
                        .read_in_transaction(&mut tx, *reference, owner)
                        .await?;
                    let Some(record) = &selected else {
                        // Do not resolve or reveal an absent/other reviewer's content.
                        context.finish(self, &mut tx).await?;
                        tx.commit().await?;
                        return Ok(None);
                    };
                    workspace.require_human(record)?;
                    record
                        .targets
                        .iter()
                        .chain(&record.evidence)
                        .cloned()
                        .collect()
                }
                Operation::Revise(expected, _) => {
                    let record = head.as_ref().ok_or(ReviewStoreError::NotFound)?;
                    workspace.require_human(record)?;
                    if record.reference != *expected {
                        return Err(ReviewStoreError::StaleRevision.into());
                    }
                    record
                        .targets
                        .iter()
                        .chain(&record.evidence)
                        .cloned()
                        .collect()
                }
            };
            // Pin both resource namespaces before mutations; never recapture a replacement.
            context
                .select_target(&mut tx, &workspace.artifacts.namespace)
                .await?;
            workspace
                .verify_references(&mut tx, owner, &references)
                .await?;
            context.select_target(&mut tx, &workspace.namespace).await?;
            let read = matches!(&operation, Operation::Read(_));
            let pending = match operation {
                Operation::Create(draft, _) => {
                    store
                        .create_in_transaction(&mut tx, id, owner, draft)
                        .await?
                }
                Operation::Read(_) => selected.ok_or(ReviewStoreError::InvalidRecord)?,
                Operation::Revise(expected, edit) => {
                    store
                        .revise_in_transaction(&mut tx, expected, owner, edit)
                        .await?
                }
            };
            if !read {
                // Locks exclude other writers, not the current transaction's trigger side effects.
                context
                    .select_target(&mut tx, &workspace.artifacts.namespace)
                    .await?;
                workspace
                    .verify_references(&mut tx, owner, &references)
                    .await?;
                context.select_target(&mut tx, &workspace.namespace).await?;
            }
            let confirmed = if read {
                store
                    .read_in_transaction(&mut tx, pending.reference, owner)
                    .await?
            } else {
                store.head_in_transaction(&mut tx, id, owner).await?
            };
            if confirmed.as_ref() != Some(&pending) {
                return Err(ReviewStoreError::InvalidRecord.into());
            }
            context.finish(self, &mut tx).await?;
            tx.commit().await?;
            Ok(Some(pending))
        })
        .await
        .map_err(|_| SessionReviewError::from(DurableAuthError::StorageUnavailable))?
    }
}
