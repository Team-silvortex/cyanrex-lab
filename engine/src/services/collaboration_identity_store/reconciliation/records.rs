use super::*;

pub(super) async fn identities(
    connection: &mut PgConnection,
    scope: WorkspaceRef,
) -> CheckResult<HashMap<PrincipalId, StoredLegacyIdentity>> {
    budget(
        connection,
        "collaboration_principals",
        "authority_id=$1",
        "octet_length(display_name)+octet_length(kind)+octet_length(status)",
        scope.authority_id,
    )
    .await?;
    let count = budget(
        connection,
        "collaboration_legacy_identities",
        "authority_id=$1",
        "octet_length(username)",
        scope.authority_id,
    )
    .await?;
    let rows = sqlx::query("SELECT i.authority_id, i.username, i.account_id, i.principal_id,
        i.retired_at IS NULL OR (isfinite(i.retired_at) AND i.retired_at <= $2) AS valid_retired_at,
        CASE WHEN isfinite(i.retired_at) AND i.retired_at <= $2 THEN i.retired_at END AS retired_at,
        p.kind, p.display_name, p.status FROM collaboration_legacy_identities i
        JOIN collaboration_principals p ON p.authority_id=i.authority_id AND p.principal_id=i.principal_id
        WHERE i.authority_id=$1 LIMIT 10001").bind(scope.authority_id.as_uuid())
        .bind(DateTime::<Utc>::MAX_UTC).fetch_all(connection).await?;
    if rows.len() as u64 != count {
        return Err(Error::InvalidIdentity);
    }
    let mut identities = HashMap::new();
    let mut accounts = HashSet::new();
    let mut active_names = HashSet::new();
    for row in rows {
        let identity = StoredLegacyIdentity::decode(&row).map_err(|_| Error::InvalidIdentity)?;
        if !accounts.insert(identity.account_id)
            || (identity.retired_at.is_none()
                && !active_names.insert(identity.binding.username.clone()))
            || identities
                .insert(identity.binding.principal_id, identity)
                .is_some()
        {
            return Err(Error::InvalidIdentity);
        }
    }
    Ok(identities)
}

pub(super) async fn policies(
    connection: &mut PgConnection,
    scope: WorkspaceRef,
    identities: &HashMap<PrincipalId, StoredLegacyIdentity>,
) -> CheckResult<HashMap<PrincipalId, StoredLegacyAccessPolicy>> {
    budget(
        connection,
        "collaboration_memberships",
        "authority_id=$1",
        "octet_length(role_refs::text)+octet_length(status)",
        scope.authority_id,
    )
    .await?;
    budget(
        connection,
        "collaboration_deployment_grants",
        "authority_id=$1",
        "octet_length(status)",
        scope.authority_id,
    )
    .await?;
    let rows = sqlx::query("SELECT principal_id, source_workspace_id, revision, status FROM collaboration_deployment_grants
        WHERE authority_id=$1 LIMIT 10001").bind(scope.authority_id.as_uuid()).fetch_all(&mut *connection).await?;
    let mut grants = HashMap::new();
    for row in rows {
        let principal = PrincipalId::try_from(row.try_get::<uuid::Uuid, _>("principal_id")?)
            .map_err(|_| Error::InvalidPolicy)?;
        if grants.insert(principal, row).is_some() {
            return Err(Error::InvalidPolicy);
        }
    }
    let rows = sqlx::query("SELECT principal_id, workspace_id, role_refs, status, revision FROM collaboration_memberships
        WHERE authority_id=$1 LIMIT 10001").bind(scope.authority_id.as_uuid()).fetch_all(connection).await?;
    let mut policies = HashMap::new();
    for row in rows {
        let principal = PrincipalId::try_from(row.try_get::<uuid::Uuid, _>("principal_id")?)
            .map_err(|_| Error::InvalidPolicy)?;
        if row.try_get::<uuid::Uuid, _>("workspace_id")? != scope.workspace_id.as_uuid()
            || !identities.contains_key(&principal)
        {
            return Err(Error::InvalidPolicy);
        }
        let grant = grants.remove(&principal).ok_or(Error::InvalidPolicy)?;
        let policy = StoredLegacyAccessPolicy::decode(scope, principal, &row, &grant)
            .map_err(|_| Error::InvalidPolicy)?;
        if policies.insert(principal, policy).is_some() {
            return Err(Error::InvalidPolicy);
        }
    }
    if !grants.is_empty() {
        return Err(Error::InvalidPolicy);
    }
    Ok(policies)
}
