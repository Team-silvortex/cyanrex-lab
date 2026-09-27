use super::*;

impl CollaborationIdentityStore {
    pub(super) async fn access_version(connection: &mut PgConnection) -> Result<()> {
        let row = sqlx::query(
            "SELECT version FROM collaboration_access_schema WHERE singleton FOR SHARE",
        )
        .fetch_one(connection)
        .await?;
        if row.try_get::<i32, _>("version")? != 1 {
            return Err(IdentityStoreError::UnsupportedSchema);
        }
        Ok(())
    }

    /// Explicitly installs staging policy tables. Requires the identity schema to be installed;
    /// it never creates identities, imports roles or changes the active authentication backend.
    pub async fn install_access_schema(&self) -> Result<()> {
        bounded(async {
            let mut tx = self.transaction().await?;
            Self::version(&mut tx).await?;
            sqlx::query("SELECT pg_advisory_xact_lock(hashtext(current_database()), hashtext('cyanrex.access.' || current_schema()))")
                .execute(&mut *tx).await?;
            let mut statements = include_str!("../../../migrations/0007_collaboration_access.sql")
                .split(';').map(str::trim).filter(|statement| !statement.is_empty());
            sqlx::query(statements.next().ok_or(IdentityStoreError::InvalidRecord)?)
                .execute(&mut *tx).await?;
            let installed = sqlx::query("SELECT version FROM collaboration_access_schema WHERE singleton FOR SHARE")
                .fetch_optional(&mut *tx).await?;
            if installed.is_some() {
                Self::access_version(&mut tx).await?;
                sqlx::query("SELECT m.authority_id, m.workspace_id, m.principal_id, m.role_refs,
                    m.status, m.revision, m.updated_at, g.authority_id, g.principal_id,
                    g.source_workspace_id, g.status, g.revision, g.updated_at
                    FROM collaboration_memberships m JOIN collaboration_deployment_grants g
                    ON g.authority_id = m.authority_id AND g.source_workspace_id = m.workspace_id
                    AND g.principal_id = m.principal_id LIMIT 0").execute(&mut *tx).await?;
            } else {
                for statement in statements {
                    sqlx::query(statement).execute(&mut *tx).await?;
                }
            }
            Self::access_version(&mut tx).await?;
            tx.commit().await?;
            Ok(())
        }).await
    }
}
