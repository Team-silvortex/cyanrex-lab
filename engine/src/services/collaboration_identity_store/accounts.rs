use super::*;

impl CollaborationIdentityStore {
    pub(super) async fn bound_principal(
        connection: &mut PgConnection,
        scope: WorkspaceRef,
        principal_id: PrincipalId,
    ) -> Result<Option<StoredLegacyIdentity>> {
        let row = sqlx::query(
            "SELECT username, account_id FROM collaboration_legacy_identities
            WHERE authority_id = $1 AND principal_id = $2 FOR SHARE",
        )
        .bind(scope.authority_id.as_uuid())
        .bind(principal_id.as_uuid())
        .fetch_optional(&mut *connection)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let username = row.try_get::<String, _>("username")?.parse()?;
        let account_id = LegacyAccountId::try_from(row.try_get::<uuid::Uuid, _>("account_id")?)?;
        Self::identity(connection, scope, &username, account_id).await
    }

    async fn identity(
        connection: &mut PgConnection,
        scope: WorkspaceRef,
        username: &LegacyUsername,
        account_id: LegacyAccountId,
    ) -> Result<Option<StoredLegacyIdentity>> {
        let row = sqlx::query("SELECT i.authority_id, i.username, i.account_id, i.principal_id, i.retired_at,
            p.kind, p.display_name, p.status FROM collaboration_legacy_identities i
            JOIN collaboration_principals p ON p.authority_id = i.authority_id AND p.principal_id = i.principal_id
            WHERE i.authority_id = $1 AND i.username = $2 AND i.account_id = $3 FOR SHARE OF i, p")
            .bind(scope.authority_id.as_uuid()).bind(username.as_str()).bind(account_id.as_uuid())
            .fetch_optional(connection).await?;
        row.map(|row| {
            let authority_id =
                AuthorityId::try_from(row.try_get::<uuid::Uuid, _>("authority_id")?)?;
            let principal_id =
                PrincipalId::try_from(row.try_get::<uuid::Uuid, _>("principal_id")?)?;
            // Legacy account bindings must never turn into machine/Agent identities on a read.
            if row.try_get::<&str, _>("kind")? != "human" {
                return Err(IdentityStoreError::InvalidRecord);
            }
            let status = match row.try_get::<&str, _>("status")? {
                "active" => PrincipalStatus::Active,
                "disabled" => PrincipalStatus::Disabled,
                _ => return Err(IdentityStoreError::InvalidRecord),
            };
            let record = StoredLegacyIdentity {
                binding: LegacyIdentityBinding {
                    authority_id,
                    username: row.try_get::<String, _>("username")?.parse()?,
                    principal_id,
                },
                account_id: LegacyAccountId::try_from(row.try_get::<uuid::Uuid, _>("account_id")?)?,
                principal: Principal {
                    reference: PrincipalRef {
                        authority_id,
                        principal_id,
                    },
                    kind: PrincipalKind::Human,
                    display_name: row.try_get("display_name")?,
                    status,
                },
                retired_at: row.try_get("retired_at")?,
            };
            if record.retired_at.is_some() && status != PrincipalStatus::Disabled {
                return Err(IdentityStoreError::InvalidRecord);
            }
            Ok(record)
        })
        .transpose()
    }

    /// Operator/history lookup, not login authorization. Includes disabled/retired records.
    /// An absent mapping returns None, and never creates a Principal.
    pub async fn lookup_legacy_account(
        &self,
        scope: WorkspaceRef,
        username: &LegacyUsername,
        account_id: LegacyAccountId,
    ) -> Result<Option<StoredLegacyIdentity>> {
        bounded(async {
            let mut tx = self.transaction().await?;
            Self::version(&mut tx).await?;
            Self::scope(&mut tx, scope, false).await?;
            let identity = Self::identity(&mut tx, scope, username, account_id).await?;
            tx.commit().await?;
            Ok(identity)
        })
        .await
    }

    /// Bind an explicitly verified account incarnation, allocating its Principal once.
    /// Different active incarnations conflict until explicit retirement. This never issues roles
    /// or grants and never guesses account identity from the old username/credential fields.
    pub async fn bind_legacy_account(
        &self,
        scope: WorkspaceRef,
        username: &LegacyUsername,
        account_id: LegacyAccountId,
    ) -> Result<StoredLegacyIdentity> {
        bounded(async {
            let mut tx = self.transaction().await?;
            Self::version(&mut tx).await?;
            if Self::scope(&mut tx, scope, true).await?.status != WorkspaceStatus::Active {
                return Err(IdentityStoreError::ScopeInactive);
            }
            if let Some(identity) = Self::identity(&mut tx, scope, username, account_id).await? {
                if identity.retired_at.is_some() { return Err(IdentityStoreError::IdentityRetired); }
                if identity.principal.status != PrincipalStatus::Active { return Err(IdentityStoreError::IdentityInactive); }
                tx.commit().await?;
                return Ok(identity);
            }
            let conflict = sqlx::query("SELECT principal_id FROM collaboration_legacy_identities
                WHERE authority_id = $1 AND ((username = $2 AND retired_at IS NULL) OR account_id = $3)")
                .bind(scope.authority_id.as_uuid()).bind(username.as_str()).bind(account_id.as_uuid())
                .fetch_optional(&mut *tx).await?;
            if conflict.is_some() { return Err(IdentityStoreError::IdentityConflict); }
            let principal_id = PrincipalId::try_from(uuid::Uuid::new_v4())?;
            confirmed_one(sqlx::query("INSERT INTO collaboration_principals (authority_id, principal_id, kind, display_name, status)
                VALUES ($1, $2, 'human', $3, 'active')")
                .bind(scope.authority_id.as_uuid()).bind(principal_id.as_uuid()).bind(username.as_str())
                .execute(&mut *tx).await?.rows_affected())?;
            confirmed_one(sqlx::query("INSERT INTO collaboration_legacy_identities (authority_id, username, account_id, principal_id)
                VALUES ($1, $2, $3, $4)")
                .bind(scope.authority_id.as_uuid()).bind(username.as_str()).bind(account_id.as_uuid()).bind(principal_id.as_uuid())
                .execute(&mut *tx).await?.rows_affected())?;
            let identity = Self::identity(&mut tx, scope, username, account_id).await?
                .ok_or(IdentityStoreError::StorageUnavailable)?;
            if identity.binding.principal_id != principal_id || identity.retired_at.is_some()
                || identity.principal.status != PrincipalStatus::Active {
                return Err(IdentityStoreError::StorageUnavailable);
            }
            tx.commit().await?;
            Ok(identity)
        }).await
    }

    /// Retire exactly the reviewed incarnation and Principal. History is retained; new same-name
    /// accounts need a different account_id and receive a fresh Principal without inherited grants.
    /// No legacy users/sessions, memberships or grants are mutated by this staging registry.
    pub async fn retire_legacy_account(
        &self,
        scope: WorkspaceRef,
        username: &LegacyUsername,
        account_id: LegacyAccountId,
        expected_principal: PrincipalId,
    ) -> Result<StoredLegacyIdentity> {
        bounded(async {
            let mut tx = self.transaction().await?;
            Self::version(&mut tx).await?;
            Self::scope(&mut tx, scope, true).await?;
            let identity = Self::identity(&mut tx, scope, username, account_id).await?
                .ok_or(IdentityStoreError::StaleIdentity)?;
            if identity.binding.principal_id != expected_principal { return Err(IdentityStoreError::StaleIdentity); }
            if identity.retired_at.is_none() {
                confirmed_one(sqlx::query("UPDATE collaboration_legacy_identities SET retired_at = NOW()
                    WHERE authority_id = $1 AND username = $2 AND account_id = $3 AND principal_id = $4 AND retired_at IS NULL")
                    .bind(scope.authority_id.as_uuid()).bind(username.as_str()).bind(account_id.as_uuid()).bind(expected_principal.as_uuid())
                    .execute(&mut *tx).await?.rows_affected())?;
                confirmed_one(sqlx::query("UPDATE collaboration_principals SET status = 'disabled'
                    WHERE authority_id = $1 AND principal_id = $2")
                    .bind(scope.authority_id.as_uuid()).bind(expected_principal.as_uuid())
                    .execute(&mut *tx).await?.rows_affected())?;
            }
            let retired = Self::identity(&mut tx, scope, username, account_id).await?
                .ok_or(IdentityStoreError::StorageUnavailable)?;
            if retired.retired_at.is_none() || retired.principal.status != PrincipalStatus::Disabled {
                return Err(IdentityStoreError::StorageUnavailable);
            }
            tx.commit().await?;
            Ok(retired)
        }).await
    }
}
