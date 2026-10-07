use super::*;

impl ReviewStore {
    pub(super) async fn namespace(tx: &mut Tx<'_>) -> Result<()> {
        if !prepared_resource_sql::namespace(tx).await? {
            return Err(ReviewStoreError::UnsupportedSchema);
        }
        Ok(())
    }
    /// Explicit fresh namespace only. A repeat/partial install is not adoption or repair.
    pub async fn install_empty_namespace(&self) -> Result<()> {
        bounded(async {
            let mut tx = self.transaction(false).await?;
            sqlx::query("SELECT pg_advisory_xact_lock(hashtext(current_database()), hashtext('cyanrex.reviews.' || current_schema()))")
                .execute(&mut *tx).await?;
            let row = sqlx::query("SELECT EXISTS(SELECT 1 FROM pg_class WHERE relnamespace = current_schema()::regnamespace)
                OR EXISTS(SELECT 1 FROM pg_proc WHERE pronamespace = current_schema()::regnamespace)
                OR EXISTS(SELECT 1 FROM pg_type WHERE typnamespace = current_schema()::regnamespace) AS occupied")
                .fetch_one(&mut *tx).await?;
            if row.try_get::<bool, _>("occupied")? { return Err(ReviewStoreError::NamespaceNotEmpty); }
            for statement in include_str!("../../../migrations/0013_collaboration_reviews.sql")
                .split(';').map(str::trim).filter(|statement| !statement.is_empty()) {
                sqlx::query(statement).execute(&mut *tx).await?;
            }
            confirmed_one(sqlx::query("INSERT INTO collaboration_review_schema (singleton, version, authority_id, workspace_id) VALUES (TRUE, 1, $1, $2)")
                .bind(self.scope.authority_id.as_uuid()).bind(self.scope.workspace_id.as_uuid())
                .execute(&mut *tx).await?.rows_affected())?;
            self.check_schema(&mut tx, true).await?;
            tx.commit().await?;
            Ok(())
        }).await
    }
    pub(super) async fn check_schema(&self, tx: &mut Tx<'_>, lock: bool) -> Result<()> {
        Self::namespace(tx).await?;
        sqlx::query(
            "LOCK TABLE collaboration_review_schema, collaboration_reviews,
            collaboration_review_revisions, collaboration_review_outbox IN ACCESS SHARE MODE",
        )
        .execute(&mut **tx)
        .await?;
        let rows = sqlx::query("SELECT c.relkind::text AS kind, c.relpersistence::text AS persistence,
            c.relrowsecurity AS rls, c.relispartition AS partition, to_regclass(c.relname)::oid = c.oid AS resolved,
            EXISTS(SELECT 1 FROM pg_inherits i WHERE i.inhrelid = c.oid OR i.inhparent = c.oid) AS inheritance
            FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
            WHERE n.nspname = current_schema() AND c.relname IN
            ('collaboration_review_schema', 'collaboration_reviews', 'collaboration_review_revisions', 'collaboration_review_outbox')")
            .fetch_all(&mut **tx).await?;
        if !prepared_resource_sql::ordinary_tables(&rows, 4)? {
            return Err(ReviewStoreError::UnsupportedSchema);
        }
        let statement = if lock {
            "SELECT version, authority_id, workspace_id FROM collaboration_review_schema WHERE singleton FOR SHARE"
        } else {
            "SELECT version, authority_id, workspace_id FROM collaboration_review_schema WHERE singleton"
        };
        let row = sqlx::query(statement)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(ReviewStoreError::UnsupportedSchema)?;
        if row.try_get::<i32, _>("version")? != 1 {
            return Err(ReviewStoreError::UnsupportedSchema);
        }
        if row.try_get::<uuid::Uuid, _>("authority_id")? != self.scope.authority_id.as_uuid()
            || row.try_get::<uuid::Uuid, _>("workspace_id")? != self.scope.workspace_id.as_uuid()
        {
            return Err(ReviewStoreError::ScopeMismatch);
        }
        Ok(())
    }
}
