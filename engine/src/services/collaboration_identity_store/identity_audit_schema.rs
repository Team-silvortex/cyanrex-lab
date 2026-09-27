use super::*;

impl CollaborationIdentityStore {
    pub(super) async fn verify_identity_audit_schema(connection: &mut PgConnection) -> Result<()> {
        sqlx::query(&format!(
            "{} LIMIT 0",
            identity_audit::IDENTITY_AUDIT_SELECT
        ))
        .execute(&mut *connection)
        .await?;
        let row = sqlx::query("SELECT count(*) AS count FROM pg_trigger
            WHERE tgrelid = 'collaboration_identity_audit'::REGCLASS AND NOT tgisinternal
            AND tgenabled IN ('O', 'A') AND tgfoid = 'collaboration_reject_identity_audit_mutation()'::REGPROCEDURE
            AND ((tgname = 'collaboration_identity_audit_immutable' AND tgtype = 27)
                OR (tgname = 'collaboration_identity_audit_no_truncate' AND tgtype = 34))")
            .fetch_one(&mut *connection).await?;
        if row.try_get::<i64, _>("count")? != 2 {
            return Err(IdentityStoreError::InvalidRecord);
        }
        Ok(())
    }

    /// Explicit staging upgrade after C1-C policy audit, never a deployed account migration.
    /// Baselines acknowledge only observed identities; historical actors remain unknown.
    pub async fn upgrade_identity_audit_schema(&self) -> Result<()> {
        bounded(async {
            let mut tx = self.transaction().await?;
            sqlx::query("SELECT pg_advisory_xact_lock(hashtext(current_database()), hashtext('cyanrex.identity.' || current_schema()))")
                .execute(&mut *tx).await?;
            // First acquire exclusive identity metadata, fencing every cooperative identity and
            // policy transaction. Taking SHARE then upgrading it would deadlock concurrent upgrades.
            let row = sqlx::query("SELECT version FROM collaboration_identity_schema WHERE singleton FOR UPDATE")
                .fetch_one(&mut *tx).await?;
            let version = row.try_get::<i32, _>("version")?;
            if !matches!(version, 1 | 2) { return Err(IdentityStoreError::UnsupportedSchema); }
            Self::require_policy_audit(&mut tx).await?;
            Self::verify_audit_schema(&mut tx).await?;
            if version == 2 {
                Self::verify_identity_audit_schema(&mut tx).await?;
                tx.commit().await?; return Ok(());
            }
            sqlx::query("LOCK TABLE collaboration_authorities IN SHARE MODE").execute(&mut *tx).await?;
            let authorities = sqlx::query("SELECT authority_id FROM collaboration_authorities ORDER BY authority_id LIMIT 10001 FOR UPDATE")
                .fetch_all(&mut *tx).await?;
            if authorities.is_empty() { return Err(IdentityStoreError::AuditBootstrapRequired); }
            if authorities.len() > 10000 { return Err(IdentityStoreError::InvalidRecord); }
            for row in authorities {
                let authority = AuthorityId::try_from(row.try_get::<uuid::Uuid, _>("authority_id")?)?;
                let workspace = Self::workspace(&mut tx, authority).await?.ok_or(IdentityStoreError::ScopeNotFound)?;
                if !Self::has_policy_manager(&mut tx, workspace.reference, None).await? {
                    return Err(IdentityStoreError::AuditBootstrapRequired);
                }
            }
            let identities = sqlx::query("SELECT authority_id, username, account_id FROM collaboration_legacy_identities
                ORDER BY authority_id, username, account_id LIMIT 10001 FOR SHARE").fetch_all(&mut *tx).await?;
            if identities.len() > 10000 { return Err(IdentityStoreError::InvalidRecord); }
            for statement in include_str!("../../../migrations/0009_collaboration_identity_audit.sql")
                .split("-- cyanrex-statement").map(str::trim).filter(|value| !value.is_empty()) {
                sqlx::query(statement).execute(&mut *tx).await?;
            }
            for row in identities {
                let authority = AuthorityId::try_from(row.try_get::<uuid::Uuid, _>("authority_id")?)?;
                let workspace = Self::workspace(&mut tx, authority).await?.ok_or(IdentityStoreError::ScopeNotFound)?;
                let username = row.try_get::<String, _>("username")?.parse::<LegacyUsername>()?;
                let account_id = LegacyAccountId::try_from(row.try_get::<uuid::Uuid, _>("account_id")?)?;
                let after = Self::identity_record(&mut tx, workspace.reference, &username, account_id).await?.ok_or(IdentityStoreError::InvalidRecord)?;
                Self::append_identity_audit(&mut tx, workspace.reference, None, None, &after).await?;
            }
            Self::verify_identity_audit_schema(&mut tx).await?;
            confirmed_one(sqlx::query("UPDATE collaboration_identity_schema SET version = 2 WHERE singleton AND version = 1")
                .execute(&mut *tx).await?.rows_affected())?;
            Self::require_identity_audit(&mut tx).await?;
            tx.commit().await?;
            Ok(())
        }).await
    }
}
