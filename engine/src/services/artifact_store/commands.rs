use super::*;
use chrono::Utc;

impl ArtifactStore {
    /// Duplicate IDs/paths are conflicts, not replay or permission to adopt an unpublished blob.
    pub async fn create(
        &self,
        artifact_id: ArtifactId,
        revision_id: ArtifactRevisionId,
        owner: PrincipalRef,
        draft: ArtifactDraft,
    ) -> Result<ArtifactRevision> {
        self.check_scope(self.scope, owner)?;
        bounded(async {
            let mut tx = self.transaction(false).await?;
            let revision = self
                .create_in_transaction(&mut tx, artifact_id, revision_id, owner, draft)
                .await?;
            tx.commit().await?;
            Ok(revision)
        })
        .await
    }

    /// Pending publication only: the trusted caller owns authorization, deadlines and commit.
    pub(crate) async fn create_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        artifact_id: ArtifactId,
        revision_id: ArtifactRevisionId,
        owner: PrincipalRef,
        draft: ArtifactDraft,
    ) -> Result<ArtifactRevision> {
        self.check_scope(self.scope, owner)?;
        self.check_schema(tx, true).await?;
        let revision = ArtifactRevision {
            schema_version: CoreSchemaVersion,
            reference: ArtifactRef {
                workspace: self.scope,
                artifact_id,
                revision_id,
                sha256: blob::digest(&draft.bytes),
            },
            owner,
            sequence: 1.try_into().map_err(|_| ArtifactStoreError::InvalidInput)?,
            parent: None,
            kind: draft.kind,
            title: draft.title,
            media_type: draft.media_type,
            byte_length: draft.bytes.len() as u64,
            created_at: Utc::now(),
        };
        confirmed_one(sqlx::query("INSERT INTO collaboration_artifacts (artifact_id, owner_id, head_revision_id, sequence) VALUES ($1, $2, $3, 1)")
            .bind(artifact_id.as_uuid()).bind(owner.principal_id.as_uuid()).bind(revision_id.as_uuid())
            .execute(&mut **tx).await?.rows_affected())?;
        self.publish(tx, &revision, draft.bytes).await?;
        Ok(revision)
    }

    /// A new immutable revision with one exact current parent; no branching, retyping or owner changes.
    pub async fn revise(
        &self,
        expected: &ArtifactRef,
        revision_id: ArtifactRevisionId,
        owner: PrincipalRef,
        draft: ArtifactDraft,
    ) -> Result<ArtifactRevision> {
        self.check_scope(expected.workspace, owner)?;
        if revision_id == expected.revision_id {
            return Err(ArtifactStoreError::InvalidInput);
        }
        bounded(async {
            let mut tx = self.transaction(false).await?;
            let revision = self
                .revise_in_transaction(&mut tx, expected, revision_id, owner, draft)
                .await?;
            tx.commit().await?;
            Ok(revision)
        })
        .await
    }

    /// Pending publication only; failure leaves any written blob for explicit reconciliation.
    pub(crate) async fn revise_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        expected: &ArtifactRef,
        revision_id: ArtifactRevisionId,
        owner: PrincipalRef,
        draft: ArtifactDraft,
    ) -> Result<ArtifactRevision> {
        self.check_scope(expected.workspace, owner)?;
        if revision_id == expected.revision_id {
            return Err(ArtifactStoreError::InvalidInput);
        }
        self.check_schema(tx, true).await?;
        let current = self
            .head(tx, expected.artifact_id, owner, true)
            .await?
            .ok_or(ArtifactStoreError::NotFound)?;
        if current.reference != *expected {
            return Err(ArtifactStoreError::StaleRevision);
        }
        if current.kind != draft.kind {
            return Err(ArtifactStoreError::InvalidInput);
        }
        self.read_blob(&current).await?;
        let revision = ArtifactRevision {
            schema_version: CoreSchemaVersion,
            reference: ArtifactRef {
                revision_id,
                sha256: blob::digest(&draft.bytes),
                ..expected.clone()
            },
            owner,
            sequence: (u64::from(current.sequence) + 1)
                .try_into()
                .map_err(|_| ArtifactStoreError::InvalidRecord)?,
            parent: Some(expected.clone()),
            kind: draft.kind,
            title: draft.title,
            media_type: draft.media_type,
            byte_length: draft.bytes.len() as u64,
            created_at: Utc::now(),
        };
        confirmed_one(
            sqlx::query(
                "UPDATE collaboration_artifacts SET head_revision_id = $3, sequence = $4
                WHERE artifact_id = $1 AND owner_id = $2 AND head_revision_id = $5",
            )
            .bind(expected.artifact_id.as_uuid())
            .bind(owner.principal_id.as_uuid())
            .bind(revision_id.as_uuid())
            .bind(u64::from(revision.sequence) as i64)
            .bind(expected.revision_id.as_uuid())
            .execute(&mut **tx)
            .await?
            .rows_affected(),
        )?;
        self.publish(tx, &revision, draft.bytes).await?;
        Ok(revision)
    }
}
