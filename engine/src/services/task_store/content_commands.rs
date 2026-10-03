use super::*;

impl TaskContentStore {
    /// Creates manual work with metadata-derived title and pins; no Artifact read or publication.
    pub async fn create(
        &self,
        task_id: TaskId,
        owner: PrincipalRef,
        manifest: TaskContentManifest,
    ) -> Result<TaskContentSnapshot> {
        let reference = TaskRef {
            workspace: self.store.scope,
            task_id,
        };
        self.store.check_scope(reference, owner)?;
        self.check_manifest_scope(&manifest)?;
        bounded(async {
            let mut tx = self.store.transaction(false).await?;
            let snapshot = self
                .create_in_transaction(&mut tx, task_id, owner, manifest)
                .await?;
            tx.commit().await?;
            Ok(snapshot)
        })
        .await
    }

    /// Pending result only. The source owns authorization, deadlines and the eventual commit.
    pub(crate) async fn create_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        task_id: TaskId,
        owner: PrincipalRef,
        manifest: TaskContentManifest,
    ) -> Result<TaskContentSnapshot> {
        let reference = TaskRef {
            workspace: self.store.scope,
            task_id,
        };
        self.store.check_scope(reference, owner)?;
        self.check_manifest_scope(&manifest)?;
        self.store.check_schema(tx, true).await?;
        let now = Utc::now();
        let snapshot = TaskContentSnapshot {
            task: TaskSnapshot {
                schema_version: CoreSchemaVersion,
                reference,
                owner,
                title: manifest.title().into(),
                definition: None,
                input_refs: manifest.input_refs(),
                status: TaskStatus::Draft,
                revision: 1.try_into().map_err(|_| TaskStoreError::InvalidInput)?,
                created_at: now,
                updated_at: now,
            },
            manifest,
        };
        self.validate_record(&snapshot)?;
        let (raw, metadata) = records::encode(&snapshot)?;
        confirmed_one(
            sqlx::query(
                "INSERT INTO collaboration_tasks
            (task_id, owner_id, revision, snapshot, manifest) VALUES ($1, $2, 1, $3, $4)",
            )
            .bind(task_id.as_uuid())
            .bind(owner.principal_id.as_uuid())
            .bind(&raw)
            .bind(&metadata)
            .execute(&mut **tx)
            .await?
            .rows_affected(),
        )?;
        self.append_and_confirm(tx, &snapshot, &raw, &metadata)
            .await?;
        Ok(snapshot)
    }

    /// Replaces the whole Draft metadata pair at one expected revision. Publication is separate.
    /// An uncertain result never authorizes retry, rollback inference or Artifact deletion.
    pub async fn replace(
        &self,
        reference: TaskRef,
        owner: PrincipalRef,
        expected_revision: RevisionNumber,
        manifest: TaskContentManifest,
    ) -> Result<TaskContentSnapshot> {
        self.store.check_scope(reference, owner)?;
        self.check_manifest_scope(&manifest)?;
        bounded(async {
            let mut tx = self.store.transaction(false).await?;
            let snapshot = self
                .replace_in_transaction(&mut tx, reference, owner, expected_revision, manifest)
                .await?;
            tx.commit().await?;
            Ok(snapshot)
        })
        .await
    }

    /// Pending whole-content edit. The caller must check old/new bytes on this same transaction.
    pub(crate) async fn replace_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        reference: TaskRef,
        owner: PrincipalRef,
        expected_revision: RevisionNumber,
        manifest: TaskContentManifest,
    ) -> Result<TaskContentSnapshot> {
        self.store.check_scope(reference, owner)?;
        self.check_manifest_scope(&manifest)?;
        self.store.check_schema(tx, true).await?;
        let current = self
            .read(tx, reference, owner, true)
            .await?
            .ok_or(TaskStoreError::NotFound)?;
        let snapshot = prepare_replacement(&current, expected_revision, manifest, Utc::now())?;
        self.update_and_confirm(tx, &snapshot, expected_revision)
            .await?;
        Ok(snapshot)
    }

    /// Preserves all metadata while advancing only the existing Task lifecycle.
    pub async fn transition(
        &self,
        reference: TaskRef,
        owner: PrincipalRef,
        expected_revision: RevisionNumber,
        status: TaskStatus,
    ) -> Result<TaskContentSnapshot> {
        self.store.check_scope(reference, owner)?;
        bounded(async {
            let mut tx = self.store.transaction(false).await?;
            let snapshot = self
                .transition_in_transaction(&mut tx, reference, owner, expected_revision, status)
                .await?;
            tx.commit().await?;
            Ok(snapshot)
        })
        .await
    }

    /// Pending lifecycle edit, without a detached grant, second connection or independent commit.
    pub(crate) async fn transition_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        reference: TaskRef,
        owner: PrincipalRef,
        expected_revision: RevisionNumber,
        status: TaskStatus,
    ) -> Result<TaskContentSnapshot> {
        self.store.check_scope(reference, owner)?;
        self.store.check_schema(tx, true).await?;
        let current = self
            .read(tx, reference, owner, true)
            .await?
            .ok_or(TaskStoreError::NotFound)?;
        let snapshot = prepare_transition(&current, expected_revision, status, Utc::now())?;
        self.update_and_confirm(tx, &snapshot, expected_revision)
            .await?;
        Ok(snapshot)
    }
}
