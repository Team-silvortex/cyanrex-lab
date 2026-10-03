use super::*;
use chrono::Utc;

impl ReviewStore {
    /// Trusted attribution only, never an acceptance transition or an authenticated receipt.
    pub async fn create(
        &self,
        review_id: ReviewId,
        reviewer: PrincipalRef,
        draft: ReviewDraft,
    ) -> Result<ReviewRecord> {
        self.check_scope(self.scope, reviewer)?;
        if draft
            .targets
            .iter()
            .chain(&draft.evidence)
            .any(|reference| reference.workspace != self.scope)
        {
            return Err(ReviewStoreError::ScopeMismatch);
        }
        let record = ReviewRecord {
            schema_version: CoreSchemaVersion,
            reference: ReviewRef {
                workspace: self.scope,
                review_id,
                revision: 1.try_into().map_err(|_| ReviewStoreError::InvalidInput)?,
            },
            reviewer,
            targets: draft.targets,
            evidence: draft.evidence,
            judgment: draft.judgment,
            supersedes: None,
            created_at: Utc::now(),
        };
        self.validate_record(&record)?;
        let raw = records::encode(&record)?;
        bounded(async {
            let mut tx = self.transaction(false).await?;
            self.check_schema(&mut tx, true).await?;
            confirmed_one(sqlx::query("INSERT INTO collaboration_reviews (review_id, reviewer_id, revision) VALUES ($1, $2, 1)")
                .bind(review_id.as_uuid()).bind(reviewer.principal_id.as_uuid())
                .execute(&mut *tx).await?.rows_affected())?;
            self.append_and_confirm(&mut tx, &record, &raw).await?;
            tx.commit().await?;
            Ok(record)
        }).await
    }
    /// A new immutable human revision retains targets, evidence, policy and reviewer.
    /// Changing the subject or rerunning a rule requires a separate Review identity.
    pub async fn revise(
        &self,
        expected: ReviewRef,
        reviewer: PrincipalRef,
        edit: HumanReviewEdit,
    ) -> Result<ReviewRecord> {
        self.check_scope(expected.workspace, reviewer)?;
        bounded(async {
            let mut tx = self.transaction(false).await?;
            self.check_schema(&mut tx, true).await?;
            let mut record = self.head(&mut tx, expected.review_id, reviewer, true).await?.ok_or(ReviewStoreError::NotFound)?;
            if record.reference != expected { return Err(ReviewStoreError::StaleRevision); }
            let ReviewJudgment::Human { verdict, comment, .. } = &mut record.judgment else {
                return Err(ReviewStoreError::NotAmendable);
            };
            *verdict = edit.verdict;
            *comment = edit.comment;
            record.reference.revision = (u64::from(expected.revision) + 1).try_into().map_err(|_| ReviewStoreError::InvalidRecord)?;
            record.supersedes = Some(expected);
            record.created_at = Utc::now();
            self.validate_record(&record)?;
            let raw = records::encode(&record)?;
            confirmed_one(sqlx::query("UPDATE collaboration_reviews SET revision = $3 WHERE review_id = $1 AND reviewer_id = $2 AND revision = $4")
                .bind(expected.review_id.as_uuid()).bind(reviewer.principal_id.as_uuid())
                .bind(u64::from(record.reference.revision) as i64).bind(u64::from(expected.revision) as i64)
                .execute(&mut *tx).await?.rows_affected())?;
            self.append_and_confirm(&mut tx, &record, &raw).await?;
            tx.commit().await?;
            Ok(record)
        }).await
    }
}
