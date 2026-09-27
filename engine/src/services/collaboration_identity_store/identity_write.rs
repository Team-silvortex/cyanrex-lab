use super::*;

impl CollaborationIdentityStore {
    /// Caller owns the authority writer lock, workspace check, audit and commit.
    pub(super) async fn bind_identity_record(
        connection: &mut PgConnection,
        scope: WorkspaceRef,
        username: &LegacyUsername,
        account_id: LegacyAccountId,
    ) -> Result<(Option<StoredLegacyIdentity>, StoredLegacyIdentity)> {
        if let Some(identity) = Self::identity(connection, scope, username, account_id).await? {
            if identity.retired_at.is_some() {
                return Err(IdentityStoreError::IdentityRetired);
            }
            if identity.principal.status != PrincipalStatus::Active {
                return Err(IdentityStoreError::IdentityInactive);
            }
            return Ok((Some(identity.clone()), identity));
        }
        let conflict = sqlx::query("SELECT principal_id FROM collaboration_legacy_identities
            WHERE authority_id = $1 AND ((username = $2 AND retired_at IS NULL) OR account_id = $3)")
            .bind(scope.authority_id.as_uuid()).bind(username.as_str()).bind(account_id.as_uuid())
            .fetch_optional(&mut *connection).await?;
        if conflict.is_some() {
            return Err(IdentityStoreError::IdentityConflict);
        }
        let principal_id = PrincipalId::try_from(uuid::Uuid::new_v4())?;
        let expected = StoredLegacyIdentity {
            binding: LegacyIdentityBinding {
                authority_id: scope.authority_id,
                username: username.clone(),
                principal_id,
            },
            account_id,
            principal: Principal {
                reference: PrincipalRef {
                    authority_id: scope.authority_id,
                    principal_id,
                },
                kind: PrincipalKind::Human,
                display_name: username.to_string(),
                status: PrincipalStatus::Active,
            },
            retired_at: None,
        };
        confirmed_one(sqlx::query("INSERT INTO collaboration_principals (authority_id, principal_id, kind, display_name, status)
            VALUES ($1, $2, 'human', $3, 'active')")
            .bind(scope.authority_id.as_uuid()).bind(principal_id.as_uuid()).bind(username.as_str())
            .execute(&mut *connection).await?.rows_affected())?;
        confirmed_one(sqlx::query("INSERT INTO collaboration_legacy_identities (authority_id, username, account_id, principal_id)
            VALUES ($1, $2, $3, $4)")
            .bind(scope.authority_id.as_uuid()).bind(username.as_str()).bind(account_id.as_uuid()).bind(principal_id.as_uuid())
            .execute(&mut *connection).await?.rows_affected())?;
        // Audit is appended by the caller, so check raw rows until that append has completed.
        if Self::identity_record(connection, scope, username, account_id)
            .await?
            .as_ref()
            != Some(&expected)
        {
            return Err(IdentityStoreError::StorageUnavailable);
        }
        Ok((None, expected))
    }

    /// Irreversibly retire the exact incarnation; no user, credential or Session table writes.
    pub(super) async fn retire_identity_record(
        connection: &mut PgConnection,
        scope: WorkspaceRef,
        username: &LegacyUsername,
        account_id: LegacyAccountId,
        expected_principal: PrincipalId,
    ) -> Result<(Option<StoredLegacyIdentity>, StoredLegacyIdentity)> {
        let before = Self::identity(connection, scope, username, account_id)
            .await?
            .ok_or(IdentityStoreError::StaleIdentity)?;
        if before.binding.principal_id != expected_principal {
            return Err(IdentityStoreError::StaleIdentity);
        }
        if before.retired_at.is_some() {
            return Ok((Some(before.clone()), before));
        }
        let at: DateTime<Utc> = sqlx::query("SELECT statement_timestamp() AS at")
            .fetch_one(&mut *connection)
            .await?
            .try_get("at")?;
        let mut expected = before.clone();
        expected.retired_at = Some(at);
        expected.principal.status = PrincipalStatus::Disabled;
        confirmed_one(sqlx::query("UPDATE collaboration_legacy_identities SET retired_at = $5
            WHERE authority_id = $1 AND username = $2 AND account_id = $3 AND principal_id = $4 AND retired_at IS NULL")
            .bind(scope.authority_id.as_uuid()).bind(username.as_str()).bind(account_id.as_uuid()).bind(expected_principal.as_uuid()).bind(at)
            .execute(&mut *connection).await?.rows_affected())?;
        confirmed_one(
            sqlx::query(
                "UPDATE collaboration_principals SET status = 'disabled'
            WHERE authority_id = $1 AND principal_id = $2",
            )
            .bind(scope.authority_id.as_uuid())
            .bind(expected_principal.as_uuid())
            .execute(&mut *connection)
            .await?
            .rows_affected(),
        )?;
        if Self::identity_record(connection, scope, username, account_id)
            .await?
            .as_ref()
            != Some(&expected)
        {
            return Err(IdentityStoreError::StorageUnavailable);
        }
        Ok((Some(before), expected))
    }
}
