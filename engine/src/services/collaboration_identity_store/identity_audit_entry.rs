use super::*;
use sqlx_postgres::PgRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyIdentityAuditKind {
    Baseline,
    Bound,
    Retired,
    Unchanged,
}

/// Identity metadata only, never passwords, token digests or proof of a revoked legacy session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyIdentityAuditEntry {
    pub sequence: u64,
    pub workspace: WorkspaceRef,
    pub principal_id: PrincipalId,
    pub kind: LegacyIdentityAuditKind,
    pub command_id: Option<IdentityCommandId>,
    pub actor: Option<PrincipalRef>,
    pub action: Option<LegacyIdentityAction>,
    pub request_digest: Option<Sha256Digest>,
    pub before: Option<StoredLegacyIdentity>,
    pub after: StoredLegacyIdentity,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyIdentityReceipt {
    pub entry: LegacyIdentityAuditEntry,
    pub replayed: bool,
}

impl LegacyIdentityAuditEntry {
    pub(super) fn decode(row: &PgRow) -> Result<Self> {
        let sequence = u64::try_from(row.try_get::<i64, _>("sequence")?)
            .map_err(|_| IdentityStoreError::InvalidRecord)?;
        RevisionNumber::try_from(sequence)?;
        let workspace = WorkspaceRef {
            authority_id: AuthorityId::try_from(row.try_get::<uuid::Uuid, _>("authority_id")?)?,
            workspace_id: WorkspaceId::try_from(row.try_get::<uuid::Uuid, _>("workspace_id")?)?,
        };
        let principal_id = PrincipalId::try_from(row.try_get::<uuid::Uuid, _>("principal_id")?)?;
        let username = row
            .try_get::<String, _>("username")?
            .parse::<LegacyUsername>()?;
        let account_id = LegacyAccountId::try_from(row.try_get::<uuid::Uuid, _>("account_id")?)?;
        let kind = match row.try_get::<&str, _>("kind")? {
            "baseline" => LegacyIdentityAuditKind::Baseline,
            "bound" => LegacyIdentityAuditKind::Bound,
            "retired" => LegacyIdentityAuditKind::Retired,
            "unchanged" => LegacyIdentityAuditKind::Unchanged,
            _ => return Err(IdentityStoreError::InvalidRecord),
        };
        let action = match row.try_get::<Option<&str>, _>("operation")? {
            None => None,
            Some("bind") => Some(LegacyIdentityAction::Bind {
                username: username.clone(),
                account_id,
            }),
            Some("retire") => Some(LegacyIdentityAction::Retire {
                username: username.clone(),
                account_id,
                expected_principal: principal_id,
            }),
            _ => return Err(IdentityStoreError::InvalidRecord),
        };
        let command_id = row
            .try_get::<Option<uuid::Uuid>, _>("command_id")?
            .map(IdentityCommandId::try_from)
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
        let before: Option<StoredLegacyIdentity> = row
            .try_get::<Option<String>, _>("before_json")?
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(|_| IdentityStoreError::InvalidRecord)?;
        let after: StoredLegacyIdentity =
            serde_json::from_str(row.try_get::<&str, _>("after_json")?)
                .map_err(|_| IdentityStoreError::InvalidRecord)?;
        for snapshot in before.iter().chain(std::iter::once(&after)) {
            if snapshot.binding.authority_id != workspace.authority_id
                || snapshot.binding.principal_id != principal_id
                || snapshot.binding.username != username
                || snapshot.account_id != account_id
                || snapshot.principal.reference
                    != (PrincipalRef {
                        authority_id: workspace.authority_id,
                        principal_id,
                    })
                || snapshot.principal.kind != PrincipalKind::Human
                || (snapshot.retired_at.is_some()
                    && snapshot.principal.status != PrincipalStatus::Disabled)
            {
                return Err(IdentityStoreError::InvalidRecord);
            }
        }
        if kind == LegacyIdentityAuditKind::Baseline {
            if command_id.is_some()
                || actor.is_some()
                || action.is_some()
                || request_digest.is_some()
                || before.is_some()
            {
                return Err(IdentityStoreError::InvalidRecord);
            }
        } else {
            let command = LegacyIdentityCommand {
                command_id: command_id.ok_or(IdentityStoreError::InvalidRecord)?,
                actor: actor.ok_or(IdentityStoreError::InvalidRecord)?,
                workspace,
                action: action.clone().ok_or(IdentityStoreError::InvalidRecord)?,
            };
            if Some(command.digest()?) != request_digest {
                return Err(IdentityStoreError::InvalidRecord);
            }
            let valid = match (kind, &command.action) {
                (LegacyIdentityAuditKind::Bound, LegacyIdentityAction::Bind { .. }) => {
                    before.is_none()
                        && after.retired_at.is_none()
                        && after.principal.status == PrincipalStatus::Active
                        && after.principal.display_name == username.as_str()
                }
                (LegacyIdentityAuditKind::Retired, LegacyIdentityAction::Retire { .. }) => {
                    before.as_ref().is_some_and(|before| {
                        let mut expected = before.clone();
                        expected.retired_at = after.retired_at;
                        expected.principal.status = PrincipalStatus::Disabled;
                        before.retired_at.is_none()
                            && after.retired_at.is_some()
                            && expected == after
                    })
                }
                (LegacyIdentityAuditKind::Unchanged, LegacyIdentityAction::Bind { .. }) => {
                    before.as_ref() == Some(&after)
                        && after.retired_at.is_none()
                        && after.principal.status == PrincipalStatus::Active
                }
                (LegacyIdentityAuditKind::Unchanged, LegacyIdentityAction::Retire { .. }) => {
                    before.as_ref() == Some(&after) && after.retired_at.is_some()
                }
                _ => false,
            };
            if !valid {
                return Err(IdentityStoreError::InvalidRecord);
            }
        }
        Ok(Self {
            sequence,
            workspace,
            principal_id,
            kind,
            command_id,
            actor,
            action,
            request_digest,
            before,
            after,
            recorded_at: row.try_get("recorded_at")?,
        })
    }
}
