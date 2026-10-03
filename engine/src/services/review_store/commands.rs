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
        let (record, raw) = self.prepare_create(review_id, reviewer, draft)?;
        bounded(async {
            let mut tx = self.transaction(false).await?;
            let record = self
                .create_prepared_in_transaction(&mut tx, record, raw)
                .await?;
            tx.commit().await?;
            Ok(record)
        })
        .await
    }

    /// Pending record only: the trusted caller owns authorization, deadlines and commit.
    pub(crate) async fn create_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        review_id: ReviewId,
        reviewer: PrincipalRef,
        draft: ReviewDraft,
    ) -> Result<ReviewRecord> {
        let (record, raw) = self.prepare_create(review_id, reviewer, draft)?;
        self.create_prepared_in_transaction(tx, record, raw).await
    }

    fn prepare_create(
        &self,
        review_id: ReviewId,
        reviewer: PrincipalRef,
        draft: ReviewDraft,
    ) -> Result<(ReviewRecord, String)> {
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
        Ok((record, raw))
    }

    async fn create_prepared_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        record: ReviewRecord,
        raw: String,
    ) -> Result<ReviewRecord> {
        self.check_schema(tx, true).await?;
        confirmed_one(sqlx::query("INSERT INTO collaboration_reviews (review_id, reviewer_id, revision) VALUES ($1, $2, 1)")
            .bind(record.reference.review_id.as_uuid()).bind(record.reviewer.principal_id.as_uuid())
            .execute(&mut **tx).await?.rows_affected())?;
        self.append_and_confirm(tx, &record, &raw).await?;
        Ok(record)
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
            let record = self
                .revise_in_transaction(&mut tx, expected, reviewer, edit)
                .await?;
            tx.commit().await?;
            Ok(record)
        })
        .await
    }

    /// Pending amendment only; no result is a confirmed receipt before the caller commits.
    pub(crate) async fn revise_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        expected: ReviewRef,
        reviewer: PrincipalRef,
        edit: HumanReviewEdit,
    ) -> Result<ReviewRecord> {
        self.check_scope(expected.workspace, reviewer)?;
        self.check_schema(tx, true).await?;
        let mut record = self
            .head(tx, expected.review_id, reviewer, true)
            .await?
            .ok_or(ReviewStoreError::NotFound)?;
        if record.reference != expected {
            return Err(ReviewStoreError::StaleRevision);
        }
        let ReviewJudgment::Human {
            verdict, comment, ..
        } = &mut record.judgment
        else {
            return Err(ReviewStoreError::NotAmendable);
        };
        *verdict = edit.verdict;
        *comment = edit.comment;
        record.reference.revision = (u64::from(expected.revision) + 1)
            .try_into()
            .map_err(|_| ReviewStoreError::InvalidRecord)?;
        record.supersedes = Some(expected);
        record.created_at = Utc::now();
        self.validate_record(&record)?;
        let raw = records::encode(&record)?;
        confirmed_one(sqlx::query("UPDATE collaboration_reviews SET revision = $3 WHERE review_id = $1 AND reviewer_id = $2 AND revision = $4")
            .bind(expected.review_id.as_uuid()).bind(reviewer.principal_id.as_uuid())
            .bind(u64::from(record.reference.revision) as i64).bind(u64::from(expected.revision) as i64)
            .execute(&mut **tx).await?.rows_affected())?;
        self.append_and_confirm(tx, &record, &raw).await?;
        Ok(record)
    }
}
