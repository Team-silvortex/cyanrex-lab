use super::*;

// Only source-owned parameter numbers reach this builder. Every caller binds chrono's maximum
// at that slot, including LIMIT 0 schema checks. Invalid times remain rows, projected as NULL.
pub(super) fn identity_audit_select(max_parameter: u8) -> String {
    format!("SELECT sequence, authority_id, workspace_id,
    principal_id, username, account_id, kind, operation, command_id, actor_principal_id,
    request_digest, CASE WHEN isfinite(recorded_at) AND recorded_at <= ${max_parameter}
    THEN recorded_at END AS recorded_at, before_state::TEXT AS before_json, after_state::TEXT AS after_json
    FROM collaboration_identity_audit")
}

impl CollaborationIdentityStore {
    pub(super) async fn check_identity_audit_head(
        connection: &mut PgConnection,
        scope: WorkspaceRef,
        username: &LegacyUsername,
        account_id: LegacyAccountId,
        current: Option<&StoredLegacyIdentity>,
    ) -> Result<()> {
        let row = sqlx::query(&format!(
            "{} WHERE authority_id = $1 AND workspace_id = $2
            AND username = $3 AND account_id = $4 ORDER BY sequence DESC LIMIT 1 FOR SHARE",
            identity_audit_select(5)
        ))
        .bind(scope.authority_id.as_uuid())
        .bind(scope.workspace_id.as_uuid())
        .bind(username.as_str())
        .bind(account_id.as_uuid())
        .bind(DateTime::<Utc>::MAX_UTC)
        .fetch_optional(&mut *connection)
        .await?;
        let head = row
            .as_ref()
            .map(LegacyIdentityAuditEntry::decode)
            .transpose()?;
        if head.as_ref().map(|entry| &entry.after) != current {
            return Err(IdentityStoreError::InvalidRecord);
        }
        Ok(())
    }

    pub(super) async fn append_identity_audit(
        connection: &mut PgConnection,
        scope: WorkspaceRef,
        command: Option<&LegacyIdentityCommand>,
        before: Option<&StoredLegacyIdentity>,
        after: &StoredLegacyIdentity,
    ) -> Result<LegacyIdentityAuditEntry> {
        let principal = after.binding.principal_id;
        let kind = if command.is_none() {
            "baseline"
        } else if before == Some(after) {
            "unchanged"
        } else if before.is_none() {
            "bound"
        } else {
            "retired"
        };
        let digest = command.map(LegacyIdentityCommand::digest).transpose()?;
        let before_json = before
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| IdentityStoreError::InvalidRecord)?;
        let after_json =
            serde_json::to_string(after).map_err(|_| IdentityStoreError::InvalidRecord)?;
        let row = sqlx::query("INSERT INTO collaboration_identity_audit (authority_id, workspace_id, principal_id,
            username, account_id, kind, operation, command_id, actor_principal_id, request_digest, before_state, after_state)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::TEXT::JSONB, $12::TEXT::JSONB) RETURNING sequence")
            .bind(scope.authority_id.as_uuid()).bind(scope.workspace_id.as_uuid()).bind(principal.as_uuid())
            .bind(after.binding.username.as_str()).bind(after.account_id.as_uuid()).bind(kind)
            .bind(command.map(|value| value.action.operation())).bind(command.map(|value| value.command_id.as_uuid()))
            .bind(command.map(|value| value.actor.principal_id.as_uuid())).bind(digest.as_ref().map(Sha256Digest::as_str))
            .bind(before_json).bind(after_json).fetch_optional(&mut *connection).await?.ok_or(IdentityStoreError::StorageUnavailable)?;
        let row = sqlx::query(&format!(
            "{} WHERE sequence = $1 FOR SHARE",
            identity_audit_select(2)
        ))
        .bind(row.try_get::<i64, _>("sequence")?)
        .bind(DateTime::<Utc>::MAX_UTC)
        .fetch_one(&mut *connection)
        .await?;
        let entry = LegacyIdentityAuditEntry::decode(&row)?;
        if entry.workspace != scope
            || entry.principal_id != principal
            || entry.command_id != command.map(|value| value.command_id)
            || entry.actor != command.map(|value| value.actor)
            || entry.action.as_ref() != command.map(|value| &value.action)
            || entry.request_digest != digest
            || entry.before.as_ref() != before
            || entry.after != *after
        {
            return Err(IdentityStoreError::StorageUnavailable);
        }
        if Self::identity_record(connection, scope, &after.binding.username, after.account_id)
            .await?
            .as_ref()
            != Some(after)
        {
            return Err(IdentityStoreError::StorageUnavailable);
        }
        Ok(entry)
    }

    /// Current-manager-only internal history. Actor must be authenticated by a trusted adapter.
    /// Bounded keyset cursor; sequence gaps are permitted, not a general event commit offset.
    pub async fn legacy_identity_audit(
        &self,
        scope: WorkspaceRef,
        actor: PrincipalRef,
        subject: PrincipalId,
        after_sequence: Option<u64>,
        limit: usize,
    ) -> Result<Vec<LegacyIdentityAuditEntry>> {
        if !(1..=100).contains(&limit) {
            return Err(IdentityStoreError::InvalidRecord);
        }
        if let Some(sequence) = after_sequence {
            RevisionNumber::try_from(sequence)?;
        }
        bounded(async {
            let mut tx = self.transaction().await?;
            Self::access_scope(&mut tx, scope, false).await?;
            Self::require_identity_audit(&mut tx).await?;
            Self::require_policy_manager(&mut tx, scope, actor).await?;
            Self::bound_principal(&mut tx, scope, subject).await?;
            let rows = sqlx::query(&format!(
                "{} WHERE authority_id = $1 AND workspace_id = $2
                AND principal_id = $3 AND sequence > $4 ORDER BY sequence LIMIT $5 FOR SHARE",
                identity_audit_select(6)
            ))
            .bind(scope.authority_id.as_uuid())
            .bind(scope.workspace_id.as_uuid())
            .bind(subject.as_uuid())
            .bind(after_sequence.unwrap_or(0) as i64)
            .bind(limit as i64)
            .bind(DateTime::<Utc>::MAX_UTC)
            .fetch_all(&mut *tx)
            .await?;
            let entries = rows
                .iter()
                .map(LegacyIdentityAuditEntry::decode)
                .collect::<Result<_>>()?;
            tx.commit().await?;
            Ok(entries)
        })
        .await
    }
}
