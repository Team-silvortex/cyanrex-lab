use super::*;

pub(super) async fn identities(
    connection: &mut PgConnection,
    scope: WorkspaceRef,
    identities: &HashMap<PrincipalId, StoredLegacyIdentity>,
) -> CheckResult<u64> {
    let count = budget(connection, "collaboration_identity_audit", "authority_id=$1",
        "COALESCE(octet_length(before_state::text),0)+octet_length(after_state::text)+octet_length(username)
        +octet_length(kind)+COALESCE(octet_length(operation),0)+COALESCE(octet_length(request_digest),0)", scope.authority_id).await?;
    let rows = sqlx::query(&format!(
        "{} WHERE authority_id=$1 ORDER BY sequence LIMIT 10001",
        identity_audit::identity_audit_select(2)
    ))
    .bind(scope.authority_id.as_uuid())
    .bind(DateTime::<Utc>::MAX_UTC)
    .fetch_all(connection)
    .await?;
    let mut heads = HashMap::new();
    let mut commands = HashSet::new();
    for row in rows {
        let entry =
            LegacyIdentityAuditEntry::decode(&row).map_err(|_| Error::IdentityHistoryMismatch)?;
        let prior = heads.get(&entry.principal_id);
        // Sequence gaps are normal (rollbacks, other subjects/authorities). State continuity,
        // not contiguous sequence numbers or wall-clock ordering, is the invariant.
        if entry.workspace != scope
            || entry.before.as_ref() != prior
            || (entry.kind == LegacyIdentityAuditKind::Baseline && prior.is_some())
            || entry
                .actor
                .is_some_and(|actor| !identities.contains_key(&actor.principal_id))
            || entry.command_id.is_some_and(|id| !commands.insert(id))
        {
            return Err(Error::IdentityHistoryMismatch);
        }
        heads.insert(entry.principal_id, entry.after);
    }
    if &heads != identities {
        return Err(Error::IdentityHistoryMismatch);
    }
    Ok(count)
}

pub(super) async fn policies(
    connection: &mut PgConnection,
    scope: WorkspaceRef,
    identities: &HashMap<PrincipalId, StoredLegacyIdentity>,
    policies: &HashMap<PrincipalId, StoredLegacyAccessPolicy>,
) -> CheckResult<u64> {
    let count = budget(
        connection,
        "collaboration_policy_audit",
        "authority_id=$1",
        "COALESCE(octet_length(before_state::text),0)+octet_length(after_state::text)
        +octet_length(kind)+COALESCE(octet_length(request_digest),0)",
        scope.authority_id,
    )
    .await?;
    let rows = sqlx::query(&format!(
        "{} WHERE authority_id=$1 ORDER BY sequence LIMIT 10001",
        policy_audit::audit_select(2)
    ))
    .bind(scope.authority_id.as_uuid())
    .bind(DateTime::<Utc>::MAX_UTC)
    .fetch_all(connection)
    .await?;
    let mut heads = HashMap::new();
    let mut commands = HashSet::new();
    for row in rows {
        let entry =
            LegacyPolicyAuditEntry::decode(&row).map_err(|_| Error::PolicyHistoryMismatch)?;
        let prior = heads.get(&entry.principal_id);
        if entry.workspace != scope
            || entry.before.as_ref() != prior
            || (entry.kind == PolicyAuditKind::Baseline && prior.is_some())
            || entry
                .actor
                .is_some_and(|actor| !identities.contains_key(&actor.principal_id))
            || entry.command_id.is_some_and(|id| !commands.insert(id))
        {
            return Err(Error::PolicyHistoryMismatch);
        }
        heads.insert(entry.principal_id, entry.after);
    }
    if &heads != policies {
        return Err(Error::PolicyHistoryMismatch);
    }
    Ok(count)
}
