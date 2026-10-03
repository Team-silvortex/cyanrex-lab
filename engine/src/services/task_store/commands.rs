use super::*;
use chrono::Utc;

impl TaskStore {
    /// Stable task ID is supplied explicitly. Duplicate create is a conflict, not success replay.
    pub async fn create(
        &self,
        task_id: TaskId,
        owner: PrincipalRef,
        draft: TaskDraft,
    ) -> Result<TaskSnapshot> {
        self.check_create_scope(task_id, owner, &draft)?;
        bounded(async {
            let mut tx = self.transaction(false).await?;
            let snapshot = self
                .create_in_transaction(&mut tx, task_id, owner, draft)
                .await?;
            tx.commit().await?;
            Ok(snapshot)
        })
        .await
    }

    fn check_create_scope(
        &self,
        task_id: TaskId,
        owner: PrincipalRef,
        draft: &TaskDraft,
    ) -> Result<TaskRef> {
        let reference = TaskRef {
            workspace: self.scope,
            task_id,
        };
        self.check_scope(reference, owner)?;
        if draft
            .inputs
            .iter()
            .any(|input| input.workspace != self.scope)
        {
            return Err(TaskStoreError::ScopeMismatch);
        }
        Ok(reference)
    }

    /// Pending result only: the trusted caller owns authorization, deadlines and final commit.
    pub(crate) async fn create_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        task_id: TaskId,
        owner: PrincipalRef,
        draft: TaskDraft,
    ) -> Result<TaskSnapshot> {
        let reference = self.check_create_scope(task_id, owner, &draft)?;
        let now = Utc::now();
        let snapshot = TaskSnapshot {
            schema_version: CoreSchemaVersion,
            reference,
            owner,
            title: draft.title,
            definition: draft.definition,
            input_refs: draft.inputs,
            status: TaskStatus::Draft,
            revision: 1.try_into().map_err(|_| TaskStoreError::InvalidInput)?,
            created_at: now,
            updated_at: now,
        };
        self.validate_snapshot(&snapshot)?;
        let raw = records::encode(&snapshot)?;
        self.check_schema(tx, true).await?;
        confirmed_one(sqlx::query("INSERT INTO collaboration_tasks (task_id, owner_id, revision, snapshot) VALUES ($1, $2, 1, $3)")
            .bind(task_id.as_uuid()).bind(owner.principal_id.as_uuid()).bind(&raw)
            .execute(&mut **tx).await?.rows_affected())?;
        self.append_and_confirm(tx, &snapshot, &raw).await?;
        Ok(snapshot)
    }

    /// Owner-only lifecycle command. No execution outcome or automatic assessment can invoke acceptance.
    pub async fn transition(
        &self,
        reference: TaskRef,
        owner: PrincipalRef,
        expected_revision: RevisionNumber,
        status: TaskStatus,
    ) -> Result<TaskSnapshot> {
        self.check_scope(reference, owner)?;
        bounded(async {
            let mut tx = self.transaction(false).await?;
            let snapshot = self
                .transition_in_transaction(&mut tx, reference, owner, expected_revision, status)
                .await?;
            tx.commit().await?;
            Ok(snapshot)
        })
        .await
    }

    /// Pending result only: no independent transaction, authorization check or commit.
    pub(crate) async fn transition_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        reference: TaskRef,
        owner: PrincipalRef,
        expected_revision: RevisionNumber,
        status: TaskStatus,
    ) -> Result<TaskSnapshot> {
        self.check_scope(reference, owner)?;
        self.check_schema(tx, true).await?;
        let mut snapshot = self
            .read(tx, reference, owner, true)
            .await?
            .ok_or(TaskStoreError::NotFound)?;
        if snapshot.revision != expected_revision {
            return Err(TaskStoreError::StaleRevision);
        }
        if !snapshot.status.can_transition_to(status) {
            return Err(TaskStoreError::InvalidTransition);
        }
        snapshot.revision = (u64::from(snapshot.revision) + 1)
            .try_into()
            .map_err(|_| TaskStoreError::InvalidRecord)?;
        snapshot.status = status;
        snapshot.updated_at = Utc::now();
        self.update_and_confirm(tx, &snapshot, expected_revision)
            .await?;
        Ok(snapshot)
    }

    /// Shared pending row/outbox write; the caller already holds the verified Task row lock.
    pub(super) async fn update_and_confirm(
        &self,
        tx: &mut Tx<'_>,
        snapshot: &TaskSnapshot,
        expected_revision: RevisionNumber,
    ) -> Result<()> {
        let raw = records::encode(snapshot)?;
        confirmed_one(
            sqlx::query(
                "UPDATE collaboration_tasks SET revision = $3, snapshot = $4
                WHERE task_id = $1 AND owner_id = $2 AND revision = $5",
            )
            .bind(snapshot.reference.task_id.as_uuid())
            .bind(snapshot.owner.principal_id.as_uuid())
            .bind(u64::from(snapshot.revision) as i64)
            .bind(&raw)
            .bind(u64::from(expected_revision) as i64)
            .execute(&mut **tx)
            .await?
            .rows_affected(),
        )?;
        self.append_and_confirm(tx, snapshot, &raw).await
    }
}
