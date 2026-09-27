use super::*;
use sqlx_postgres::PgRow;

pub(super) const AUDIT_SELECT: &str = "SELECT sequence, authority_id, workspace_id, principal_id,
    revision, kind, command_id, actor_principal_id, request_digest, recorded_at,
    before_state::TEXT AS before_json, after_state::TEXT AS after_json
    FROM collaboration_policy_audit";

/// Baselines describe only the state observed at activation, not fabricated historical commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyAuditKind {
    Baseline,
    Changed,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyPolicyAuditEntry {
    pub sequence: u64,
    pub workspace: WorkspaceRef,
    pub principal_id: PrincipalId,
    pub kind: PolicyAuditKind,
    pub command_id: Option<PolicyCommandId>,
    pub actor: Option<PrincipalRef>,
    pub request_digest: Option<Sha256Digest>,
    pub before: Option<StoredLegacyAccessPolicy>,
    pub after: StoredLegacyAccessPolicy,
    pub recorded_at: DateTime<Utc>,
}

/// A replay is a historical acknowledgement, not a claim that the policy remains effective.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyPolicyReceipt {
    pub entry: LegacyPolicyAuditEntry,
    pub replayed: bool,
}

impl LegacyPolicyAuditEntry {
    pub(super) fn decode(row: &PgRow) -> Result<Self> {
        let sequence = u64::try_from(row.try_get::<i64, _>("sequence")?)
            .map_err(|_| IdentityStoreError::InvalidRecord)?;
        RevisionNumber::try_from(sequence)?;
        let workspace = WorkspaceRef {
            authority_id: AuthorityId::try_from(row.try_get::<uuid::Uuid, _>("authority_id")?)?,
            workspace_id: WorkspaceId::try_from(row.try_get::<uuid::Uuid, _>("workspace_id")?)?,
        };
        let principal_id = PrincipalId::try_from(row.try_get::<uuid::Uuid, _>("principal_id")?)?;
        let kind = match row.try_get::<&str, _>("kind")? {
            "baseline" => PolicyAuditKind::Baseline,
            "changed" => PolicyAuditKind::Changed,
            "unchanged" => PolicyAuditKind::Unchanged,
            _ => return Err(IdentityStoreError::InvalidRecord),
        };
        let command_id = row
            .try_get::<Option<uuid::Uuid>, _>("command_id")?
            .map(PolicyCommandId::try_from)
            .transpose()?;
        let actor = row
            .try_get::<Option<uuid::Uuid>, _>("actor_principal_id")?
            .map(PrincipalId::try_from)
            .transpose()?
            .map(|principal_id| PrincipalRef {
                authority_id: workspace.authority_id,
                principal_id,
            });
        let request_digest = row
            .try_get::<Option<String>, _>("request_digest")?
            .map(|value| value.parse::<Sha256Digest>())
            .transpose()?;
        let before: Option<StoredLegacyAccessPolicy> = row
            .try_get::<Option<String>, _>("before_json")?
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(|_| IdentityStoreError::InvalidRecord)?;
        let after: StoredLegacyAccessPolicy =
            serde_json::from_str(row.try_get::<&str, _>("after_json")?)
                .map_err(|_| IdentityStoreError::InvalidRecord)?;
        for snapshot in before.iter().chain(std::iter::once(&after)) {
            if snapshot.policy.membership.workspace != workspace
                || snapshot.policy.membership.principal_id != principal_id
                || snapshot.policy.canonical()? != snapshot.policy
            {
                return Err(IdentityStoreError::InvalidRecord);
            }
        }
        if u64::from(after.revision) as i64 != row.try_get::<i64, _>("revision")? {
            return Err(IdentityStoreError::InvalidRecord);
        }
        match kind {
            PolicyAuditKind::Baseline => {
                if before.is_some()
                    || actor.is_some()
                    || command_id.is_some()
                    || request_digest.is_some()
                {
                    return Err(IdentityStoreError::InvalidRecord);
                }
            }
            PolicyAuditKind::Changed | PolicyAuditKind::Unchanged => {
                let command = LegacyPolicyCommand {
                    command_id: command_id.ok_or(IdentityStoreError::InvalidRecord)?,
                    actor: actor.ok_or(IdentityStoreError::InvalidRecord)?,
                    expected_revision: before.as_ref().map(|value| value.revision),
                    desired: after.policy.clone(),
                };
                if Some(command.digest()?) != request_digest {
                    return Err(IdentityStoreError::InvalidRecord);
                }
                let valid = if kind == PolicyAuditKind::Unchanged {
                    before.as_ref() == Some(&after)
                } else {
                    before
                        .as_ref()
                        .is_none_or(|value| value.policy != after.policy)
                        && u64::from(after.revision)
                            == before
                                .as_ref()
                                .map_or(1, |value| u64::from(value.revision) + 1)
                };
                if !valid {
                    return Err(IdentityStoreError::InvalidRecord);
                }
            }
        }
        Ok(Self {
            sequence,
            workspace,
            principal_id,
            kind,
            command_id,
            actor,
            request_digest,
            before,
            after,
            recorded_at: row.try_get("recorded_at")?,
        })
    }
}

impl CollaborationIdentityStore {
    pub(super) async fn checked_access_record(
        connection: &mut PgConnection,
        scope: WorkspaceRef,
        principal: PrincipalId,
    ) -> Result<Option<StoredLegacyAccessPolicy>> {
        let current = Self::access_record(connection, scope, principal).await?;
        if Self::access_version(connection).await? == 2 {
            let row = sqlx::query(&format!("{AUDIT_SELECT} WHERE authority_id = $1
                AND workspace_id = $2 AND principal_id = $3 ORDER BY sequence DESC LIMIT 1 FOR SHARE"))
                .bind(scope.authority_id.as_uuid()).bind(scope.workspace_id.as_uuid())
                .bind(principal.as_uuid()).fetch_optional(&mut *connection).await?;
            let head = row
                .as_ref()
                .map(LegacyPolicyAuditEntry::decode)
                .transpose()?;
            if head.as_ref().map(|entry| &entry.after) != current.as_ref() {
                return Err(IdentityStoreError::InvalidRecord);
            }
        }
        Ok(current)
    }

    pub(super) async fn append_policy_audit(
        connection: &mut PgConnection,
        command: Option<&LegacyPolicyCommand>,
        before: Option<&StoredLegacyAccessPolicy>,
        after: &StoredLegacyAccessPolicy,
    ) -> Result<LegacyPolicyAuditEntry> {
        let scope = after.policy.membership.workspace;
        let principal = after.policy.membership.principal_id;
        let kind = if command.is_none() {
            "baseline"
        } else if before == Some(after) {
            "unchanged"
        } else {
            "changed"
        };
        let digest = command.map(LegacyPolicyCommand::digest).transpose()?;
        let before_json = before
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| IdentityStoreError::InvalidRecord)?;
        let after_json =
            serde_json::to_string(after).map_err(|_| IdentityStoreError::InvalidRecord)?;
        let row = sqlx::query("INSERT INTO collaboration_policy_audit (authority_id, workspace_id,
            principal_id, revision, kind, command_id, actor_principal_id, request_digest, before_state, after_state)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::TEXT::JSONB, $10::TEXT::JSONB) RETURNING sequence")
            .bind(scope.authority_id.as_uuid()).bind(scope.workspace_id.as_uuid()).bind(principal.as_uuid())
            .bind(u64::from(after.revision) as i64).bind(kind)
            .bind(command.map(|value| value.command_id.as_uuid()))
            .bind(command.map(|value| value.actor.principal_id.as_uuid()))
            .bind(digest.as_ref().map(Sha256Digest::as_str)).bind(before_json).bind(after_json)
            .fetch_optional(&mut *connection).await?.ok_or(IdentityStoreError::StorageUnavailable)?;
        let row = sqlx::query(&format!("{AUDIT_SELECT} WHERE sequence = $1 FOR SHARE"))
            .bind(row.try_get::<i64, _>("sequence")?)
            .fetch_one(&mut *connection)
            .await?;
        let entry = LegacyPolicyAuditEntry::decode(&row)?;
        if entry.workspace != scope
            || entry.principal_id != principal
            || entry.command_id != command.map(|value| value.command_id)
            || entry.actor != command.map(|value| value.actor)
            || entry.request_digest != digest
            || entry.before.as_ref() != before
            || entry.after != *after
        {
            return Err(IdentityStoreError::StorageUnavailable);
        }
        // Catch receipt triggers that also alter the durable policy rows after writer readback.
        if Self::access_record(connection, scope, principal)
            .await?
            .as_ref()
            != Some(after)
        {
            return Err(IdentityStoreError::StorageUnavailable);
        }
        Ok(entry)
    }

    /// Privileged, bounded history; caller supplies an authenticated actor, not a client claim.
    /// Cursor orders this subject's committed receipts; gaps are allowed, not a C3 event offset.
    pub async fn legacy_policy_audit(
        &self,
        scope: WorkspaceRef,
        actor: PrincipalRef,
        subject: PrincipalId,
        after_sequence: Option<u64>,
        limit: usize,
    ) -> Result<Vec<LegacyPolicyAuditEntry>> {
        if !(1..=100).contains(&limit) {
            return Err(IdentityStoreError::InvalidRecord);
        }
        if let Some(sequence) = after_sequence {
            RevisionNumber::try_from(sequence)?;
        }
        bounded(async {
            let mut tx = self.transaction().await?;
            Self::access_scope(&mut tx, scope, false).await?;
            Self::require_policy_audit(&mut tx).await?;
            Self::require_policy_manager(&mut tx, scope, actor).await?;
            Self::checked_access_record(&mut tx, scope, subject).await?;
            let rows = sqlx::query(&format!(
                "{AUDIT_SELECT} WHERE authority_id = $1 AND workspace_id = $2
                AND principal_id = $3 AND sequence > $4 ORDER BY sequence LIMIT $5 FOR SHARE"
            ))
            .bind(scope.authority_id.as_uuid())
            .bind(scope.workspace_id.as_uuid())
            .bind(subject.as_uuid())
            .bind(after_sequence.unwrap_or(0) as i64)
            .bind(limit as i64)
            .fetch_all(&mut *tx)
            .await?;
            let entries = rows
                .iter()
                .map(LegacyPolicyAuditEntry::decode)
                .collect::<Result<_>>()?;
            tx.commit().await?;
            Ok(entries)
        })
        .await
    }
}
