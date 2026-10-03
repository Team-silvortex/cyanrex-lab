use super::*;

impl TaskStore {
    pub(super) async fn namespace(tx: &mut Tx<'_>) -> Result<()> {
        let row = sqlx::query(
            "SELECT current_schema() AS name, cardinality(current_schemas(false)) AS count",
        )
        .fetch_one(&mut **tx)
        .await?;
        let name: Option<String> = row.try_get("name")?;
        if row.try_get::<i32, _>("count")? != 1
            || name.as_deref().map_or(true, |name| {
                name == "public" || name == "information_schema" || name.starts_with("pg_")
            })
        {
            return Err(TaskStoreError::UnsupportedSchema);
        }
        Ok(())
    }

    /// Fresh dedicated namespace only; repeat install is a refusal, never implicit adoption/repair.
    pub async fn install_empty_namespace(&self) -> Result<()> {
        bounded(async {
            let mut tx = self.transaction(false).await?;
            sqlx::query("SELECT pg_advisory_xact_lock(hashtext(current_database()), hashtext('cyanrex.tasks.' || current_schema()))")
                .execute(&mut *tx).await?;
            let row = sqlx::query("SELECT EXISTS(SELECT 1 FROM pg_class WHERE relnamespace = current_schema()::regnamespace)
                OR EXISTS(SELECT 1 FROM pg_proc WHERE pronamespace = current_schema()::regnamespace)
                OR EXISTS(SELECT 1 FROM pg_type WHERE typnamespace = current_schema()::regnamespace) AS occupied")
                .fetch_one(&mut *tx).await?;
            if row.try_get::<bool, _>("occupied")? { return Err(TaskStoreError::NamespaceNotEmpty); }
            let schema = match self.storage_version {
                TASK_STORAGE_VERSION => include_str!("../../../migrations/0011_collaboration_tasks.sql"),
                TASK_CONTENT_STORAGE_VERSION => include_str!("../../../migrations/0014_collaboration_task_content.sql"),
                _ => return Err(TaskStoreError::UnsupportedSchema),
            };
            for statement in schema
                .split(';').map(str::trim).filter(|statement| !statement.is_empty()) {
                sqlx::query(statement).execute(&mut *tx).await?;
            }
            confirmed_one(sqlx::query("INSERT INTO collaboration_task_schema (singleton, version, authority_id, workspace_id)
                VALUES (TRUE, $1, $2, $3)")
                .bind(self.storage_version)
                .bind(self.scope.authority_id.as_uuid()).bind(self.scope.workspace_id.as_uuid())
                .execute(&mut *tx).await?.rows_affected())?;
            self.check_schema(&mut tx, true).await?;
            tx.commit().await?;
            Ok(())
        }).await
    }

    pub(super) async fn check_schema(&self, tx: &mut Tx<'_>, lock: bool) -> Result<()> {
        Self::namespace(tx).await?;
        // Pin relation identity/shape against ordinary concurrent DDL for the transaction.
        sqlx::query("LOCK TABLE collaboration_task_schema, collaboration_tasks, collaboration_task_outbox IN ACCESS SHARE MODE")
            .execute(&mut **tx).await?;
        let rows = sqlx::query("SELECT c.relname, c.relkind::text AS kind, c.relpersistence::text AS persistence,
            c.relrowsecurity AS rls, c.relispartition AS partition,
            to_regclass(c.relname)::oid = c.oid AS resolved,
            EXISTS(SELECT 1 FROM pg_inherits i WHERE i.inhrelid = c.oid OR i.inhparent = c.oid) AS inheritance
            FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
            WHERE n.nspname = current_schema() AND c.relname IN
            ('collaboration_task_schema', 'collaboration_tasks', 'collaboration_task_outbox')")
            .fetch_all(&mut **tx).await?;
        if rows.len() != 3 {
            return Err(TaskStoreError::UnsupportedSchema);
        }
        for row in rows {
            if row.try_get::<String, _>("kind")? != "r"
                || row.try_get::<String, _>("persistence")? != "p"
                || row.try_get::<bool, _>("rls")?
                || row.try_get::<bool, _>("partition")?
                || row.try_get::<bool, _>("inheritance")?
                || !row.try_get::<bool, _>("resolved")?
            {
                return Err(TaskStoreError::UnsupportedSchema);
            }
        }
        let statement = if lock {
            "SELECT version, authority_id, workspace_id FROM collaboration_task_schema WHERE singleton FOR SHARE"
        } else {
            "SELECT version, authority_id, workspace_id FROM collaboration_task_schema WHERE singleton"
        };
        let row = sqlx::query(statement)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(TaskStoreError::UnsupportedSchema)?;
        if row.try_get::<i32, _>("version")? != self.storage_version {
            return Err(TaskStoreError::UnsupportedSchema);
        }
        if row.try_get::<uuid::Uuid, _>("authority_id")? != self.scope.authority_id.as_uuid()
            || row.try_get::<uuid::Uuid, _>("workspace_id")? != self.scope.workspace_id.as_uuid()
        {
            return Err(TaskStoreError::ScopeMismatch);
        }
        Ok(())
    }
}
