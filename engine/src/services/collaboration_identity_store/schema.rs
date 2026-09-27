use super::*;

impl CollaborationIdentityStore {
    /// Explicit additive installation for a selected, trusted schema. Never called by reads,
    /// construction or Engine startup. Existing legacy tables and rows are not queried or changed.
    pub async fn install_schema(&self) -> Result<()> {
        bounded(async {
            let mut tx = self.transaction().await?;
            sqlx::query("SELECT pg_advisory_xact_lock(hashtext(current_database()), hashtext('cyanrex.identity.' || current_schema()))")
                .execute(&mut *tx).await?;
            let mut statements = include_str!("../../../migrations/0006_collaboration_identity.sql")
                .split(';').map(str::trim).filter(|statement| !statement.is_empty());
            let metadata = statements.next().ok_or(IdentityStoreError::InvalidRecord)?;
            sqlx::query(metadata).execute(&mut *tx).await?;
            let installed = sqlx::query("SELECT version FROM collaboration_identity_schema WHERE singleton FOR SHARE")
                .fetch_optional(&mut *tx).await?;
            if installed.is_some() {
                if Self::version(&mut tx).await? == 2 {
                    Self::verify_identity_audit_schema(&mut tx).await?;
                }
                // Do not recreate disappeared tables under an already-installed version.
                sqlx::query("SELECT a.authority_id, a.created_at, w.workspace_id, w.title, w.status,
                    p.principal_id, p.kind, p.display_name, p.status, p.created_at,
                    i.username, i.account_id, i.created_at, i.retired_at, m.workspace_id
                    FROM collaboration_authorities a
                    JOIN collaboration_workspaces w ON w.authority_id = a.authority_id
                    JOIN collaboration_legacy_workspaces m ON m.authority_id = w.authority_id AND m.workspace_id = w.workspace_id
                    JOIN collaboration_principals p ON p.authority_id = a.authority_id
                    JOIN collaboration_legacy_identities i ON i.authority_id = p.authority_id AND i.principal_id = p.principal_id
                    LIMIT 0").execute(&mut *tx).await?;
            } else {
                for statement in statements {
                    sqlx::query(statement).execute(&mut *tx).await?;
                }
            }
            Self::version(&mut tx).await?;
            tx.commit().await?;
            Ok(())
        }).await
    }
}
