use super::*;

impl StoredLegacyIdentity {
    pub(super) fn decode(row: &sqlx_postgres::PgRow) -> Result<Self> {
        // A corrupt non-NULL retirement is not an active binding. The SQL projection keeps
        // this validity bit separate from the nullable, decoder-safe timestamp.
        if !row.try_get::<bool, _>("valid_retired_at")? {
            return Err(IdentityStoreError::InvalidRecord);
        }
        let authority_id = AuthorityId::try_from(row.try_get::<uuid::Uuid, _>("authority_id")?)?;
        let principal_id = PrincipalId::try_from(row.try_get::<uuid::Uuid, _>("principal_id")?)?;
        // Shared by locked runtime reads and the separate read-only reconciliation snapshot.
        if row.try_get::<&str, _>("kind")? != "human" {
            return Err(IdentityStoreError::InvalidRecord);
        }
        let status = match row.try_get::<&str, _>("status")? {
            "active" => PrincipalStatus::Active,
            "disabled" => PrincipalStatus::Disabled,
            _ => return Err(IdentityStoreError::InvalidRecord),
        };
        let record = Self {
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
            retired_at: row
                .try_get("retired_at")
                .map_err(|_| IdentityStoreError::InvalidRecord)?,
        };
        if record.retired_at.is_some() && status != PrincipalStatus::Disabled {
            return Err(IdentityStoreError::InvalidRecord);
        }
        Ok(record)
    }
}

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

    pub(super) async fn identity_record(
        connection: &mut PgConnection,
        scope: WorkspaceRef,
        username: &LegacyUsername,
        account_id: LegacyAccountId,
    ) -> Result<Option<StoredLegacyIdentity>> {
        let row = sqlx::query("SELECT i.authority_id, i.username, i.account_id, i.principal_id,
            i.retired_at IS NULL OR (isfinite(i.retired_at) AND i.retired_at <= $4) AS valid_retired_at,
            CASE WHEN isfinite(i.retired_at) AND i.retired_at <= $4 THEN i.retired_at END AS retired_at,
            p.kind, p.display_name, p.status FROM collaboration_legacy_identities i
            JOIN collaboration_principals p ON p.authority_id = i.authority_id AND p.principal_id = i.principal_id
            WHERE i.authority_id = $1 AND i.username = $2 AND i.account_id = $3 FOR SHARE OF i, p")
            .bind(scope.authority_id.as_uuid()).bind(username.as_str()).bind(account_id.as_uuid())
            .bind(DateTime::<Utc>::MAX_UTC)
            .fetch_optional(connection).await?;
        row.as_ref().map(StoredLegacyIdentity::decode).transpose()
    }

    pub(super) async fn identity(
        connection: &mut PgConnection,
        scope: WorkspaceRef,
        username: &LegacyUsername,
        account_id: LegacyAccountId,
    ) -> Result<Option<StoredLegacyIdentity>> {
        let current = Self::identity_record(connection, scope, username, account_id).await?;
        if Self::version(connection).await? == 2 {
            Self::check_identity_audit_head(
                connection,
                scope,
                username,
                account_id,
                current.as_ref(),
            )
            .await?;
        }
        Ok(current)
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
            if Self::version(&mut tx).await? == 2 {
                // Serialize the identity + audit-head snapshot even for an absent binding key.
                // The same authority-first order is used by lifecycle and policy writers.
                sqlx::query("SELECT authority_id FROM collaboration_authorities WHERE authority_id = $1 FOR SHARE")
                    .bind(scope.authority_id.as_uuid()).fetch_optional(&mut *tx).await?
                    .ok_or(IdentityStoreError::ScopeNotFound)?;
            }
            Self::scope(&mut tx, scope, false).await?;
            let identity = Self::identity(&mut tx, scope, username, account_id).await?;
            tx.commit().await?;
            Ok(identity)
        })
        .await
    }

    /// Pre-audit binding only; identity schema 2 requires an attributed lifecycle command.
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
            if Self::version(&mut tx).await? != 1 {
                return Err(IdentityStoreError::IdentityAuditContextRequired);
            }
            if Self::scope(&mut tx, scope, true).await?.status != WorkspaceStatus::Active {
                return Err(IdentityStoreError::ScopeInactive);
            }
            let (_, identity) =
                Self::bind_identity_record(&mut tx, scope, username, account_id).await?;
            tx.commit().await?;
            Ok(identity)
        })
        .await
    }

    /// Pre-audit retirement only; identity schema 2 rejects this unattributed writer.
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
            if Self::version(&mut tx).await? != 1 {
                return Err(IdentityStoreError::IdentityAuditContextRequired);
            }
            Self::scope(&mut tx, scope, true).await?;
            let (_, retired) = Self::retire_identity_record(
                &mut tx,
                scope,
                username,
                account_id,
                expected_principal,
            )
            .await?;
            tx.commit().await?;
            Ok(retired)
        })
        .await
    }
}
