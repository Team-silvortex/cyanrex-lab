use super::*;

impl CollaborationIdentityStore {
    pub(super) async fn verify_audit_schema(connection: &mut PgConnection) -> Result<()> {
        sqlx::query(&format!("{} LIMIT 0", policy_audit::AUDIT_SELECT))
            .execute(&mut *connection)
            .await?;
        let row = sqlx::query(
            "SELECT count(*) AS count FROM pg_trigger
            WHERE tgrelid = 'collaboration_policy_audit'::REGCLASS AND NOT tgisinternal
            AND tgenabled IN ('O', 'A')
            AND tgfoid = 'collaboration_reject_policy_audit_mutation()'::REGPROCEDURE
            AND ((tgname = 'collaboration_policy_audit_immutable' AND tgtype = 27)
                OR (tgname = 'collaboration_policy_audit_no_truncate' AND tgtype = 34))",
        )
        .fetch_one(&mut *connection)
        .await?;
        if row.try_get::<i64, _>("count")? != 2 {
            return Err(IdentityStoreError::InvalidRecord);
        }
        Ok(())
    }

    /// Explicit staging upgrade, never Engine startup or a live account migration. Verified
    /// managers must already be bootstrapped per legacy authority. Records only observed
    /// baselines, with no invented historical actors; old unattributed writers are then fenced.
    /// Small migration only (at most 10,000 authorities/policies within the operation deadline).
    pub async fn upgrade_policy_audit_schema(&self) -> Result<()> {
        bounded(async {
            let mut tx = self.transaction().await?;
            Self::version(&mut tx).await?;
            sqlx::query("SELECT pg_advisory_xact_lock(hashtext(current_database()), hashtext('cyanrex.access.' || current_schema()))")
                .execute(&mut *tx).await?;
            // Exclusive metadata lock fences all v1/v2 policy operations before DDL. Do not
            // acquire a shared version lock first: concurrent upgrades must not upgrade locks.
            let row = sqlx::query("SELECT version FROM collaboration_access_schema WHERE singleton FOR UPDATE")
                .fetch_one(&mut *tx).await?;
            match row.try_get::<i32, _>("version")? {
                2 => { Self::verify_audit_schema(&mut tx).await?; tx.commit().await?; return Ok(()); }
                1 => (),
                _ => return Err(IdentityStoreError::UnsupportedSchema),
            }
            // Also fence C1-A identity mutations and insertion of a new authority during the
            // bootstrap/baseline snapshot. Existing mutations lock authority rows first.
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
            let policies = sqlx::query("SELECT authority_id, workspace_id, principal_id FROM collaboration_memberships
                ORDER BY authority_id, workspace_id, principal_id LIMIT 10001 FOR SHARE")
                .fetch_all(&mut *tx).await?;
            if policies.len() > 10000 { return Err(IdentityStoreError::InvalidRecord); }
            for statement in include_str!("../../../migrations/0008_collaboration_policy_audit.sql")
                .split("-- cyanrex-statement").map(str::trim).filter(|value| !value.is_empty()) {
                sqlx::query(statement).execute(&mut *tx).await?;
            }
            for row in policies {
                let scope = WorkspaceRef {
                    authority_id: AuthorityId::try_from(row.try_get::<uuid::Uuid, _>("authority_id")?)?,
                    workspace_id: WorkspaceId::try_from(row.try_get::<uuid::Uuid, _>("workspace_id")?)?,
                };
                let principal = PrincipalId::try_from(row.try_get::<uuid::Uuid, _>("principal_id")?)?;
                Self::scope(&mut tx, scope, false).await?;
                Self::bound_principal(&mut tx, scope, principal).await?.ok_or(IdentityStoreError::StaleIdentity)?;
                let after = Self::access_record(&mut tx, scope, principal).await?.ok_or(IdentityStoreError::InvalidRecord)?;
                Self::append_policy_audit(&mut tx, None, None, &after).await?;
            }
            Self::verify_audit_schema(&mut tx).await?;
            confirmed_one(sqlx::query("UPDATE collaboration_access_schema SET version = 2 WHERE singleton AND version = 1")
                .execute(&mut *tx).await?.rows_affected())?;
            Self::require_policy_audit(&mut tx).await?;
            tx.commit().await?;
            Ok(())
        }).await
    }
}
