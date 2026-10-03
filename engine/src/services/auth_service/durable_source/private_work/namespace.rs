use super::*;

pub(super) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 63
        && name.as_bytes()[0].is_ascii_lowercase()
        && name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
        && !matches!(name, "public" | "information_schema")
        && !name.starts_with("pg_")
}
#[derive(PartialEq, Eq)]
pub(super) struct NamespacePin {
    pub name: String,
    oid: i64,
}
impl NamespacePin {
    pub async fn capture(tx: &mut Transaction<'_, Postgres>) -> WorkResult<Self> {
        let row = sqlx::query(
            "SELECT current_schema()::text AS name,
            current_schema()::regnamespace::oid::bigint AS oid,
            cardinality(current_schemas(false)) AS count, pg_my_temp_schema() = 0 AS no_temp",
        )
        .fetch_one(&mut **tx)
        .await?;
        let name: Option<String> = row.try_get("name")?;
        let name = name.ok_or(PrivateWorkError::InvalidNamespace)?;
        if !valid_name(&name)
            || row.try_get::<i32, _>("count")? != 1
            || !row.try_get::<bool, _>("no_temp")?
        {
            return Err(PrivateWorkError::InvalidNamespace);
        }
        Ok(Self {
            name,
            oid: row.try_get("oid")?,
        })
    }
    pub async fn select(tx: &mut Transaction<'_, Postgres>, name: &str) -> WorkResult<()> {
        if !valid_name(name) {
            return Err(PrivateWorkError::InvalidNamespace);
        }
        // Identifiers are canonical and quoted, while the GUC value is still a SQL parameter.
        // LOCAL automatically restores the original pool configuration on commit/rollback.
        sqlx::query("SELECT pg_catalog.set_config('search_path', $1, true)")
            .bind(format!("\"{name}\""))
            .execute(&mut **tx)
            .await?;
        Ok(())
    }
    pub async fn verify(&self, tx: &mut Transaction<'_, Postgres>) -> WorkResult<()> {
        if Self::capture(tx).await? != *self {
            return Err(PrivateWorkError::InvalidNamespace);
        }
        Ok(())
    }
}

/// Runtime relation pinning, NOT the separate reconciliation snapshot or full audit history.
/// Refuse hidden/foreign/replaceable auth relations before any unqualified authority lookup.
pub(super) async fn verify_source_relations(tx: &mut Transaction<'_, Postgres>) -> WorkResult<()> {
    sqlx::query(
        "LOCK TABLE users, sessions, collaboration_auth_source_schema,
        collaboration_identity_schema, collaboration_access_schema, collaboration_authorities,
        collaboration_workspaces, collaboration_legacy_workspaces, collaboration_principals,
        collaboration_legacy_identities, collaboration_memberships, collaboration_deployment_grants,
        collaboration_identity_audit, collaboration_policy_audit IN ACCESS SHARE MODE",
    )
    .execute(&mut **tx)
    .await?;
    let invalid: i64 = sqlx::query("WITH expected(relation, key) AS (VALUES
        ('users', ARRAY['username']), ('sessions', ARRAY['token']),
        ('collaboration_auth_source_schema', ARRAY['singleton']),
        ('collaboration_identity_schema', ARRAY['singleton']), ('collaboration_access_schema', ARRAY['singleton']),
        ('collaboration_authorities', ARRAY['authority_id']),
        ('collaboration_workspaces', ARRAY['authority_id','workspace_id']),
        ('collaboration_legacy_workspaces', ARRAY['authority_id']),
        ('collaboration_principals', ARRAY['authority_id','principal_id']),
        ('collaboration_legacy_identities', ARRAY['authority_id','username','account_id']),
        ('collaboration_memberships', ARRAY['authority_id','workspace_id','principal_id']),
        ('collaboration_deployment_grants', ARRAY['authority_id','principal_id']),
        ('collaboration_identity_audit', ARRAY['sequence']), ('collaboration_policy_audit', ARRAY['sequence']))
        SELECT count(*) AS invalid FROM expected e LEFT JOIN pg_catalog.pg_class t
        ON t.relnamespace=current_schema()::regnamespace AND t.relname=e.relation
        WHERE t.oid IS NULL OR to_regclass(e.relation)::oid <> t.oid OR t.relkind<>'r'
        OR t.relpersistence<>'p' OR t.relrowsecurity OR t.relforcerowsecurity OR t.relhassubclass OR t.relispartition
        OR EXISTS (SELECT 1 FROM pg_catalog.pg_inherits i WHERE i.inhrelid=t.oid) OR NOT EXISTS (
            SELECT 1 FROM pg_catalog.pg_constraint c WHERE c.conrelid=t.oid AND c.contype='p'
            AND c.convalidated AND NOT c.condeferrable AND e.key=ARRAY(
                SELECT a.attname::text FROM unnest(c.conkey) WITH ORDINALITY k(n,position)
                JOIN pg_catalog.pg_attribute a ON a.attrelid=t.oid AND a.attnum=k.n
                WHERE a.attnotnull AND NOT a.attisdropped ORDER BY k.position))")
        .fetch_one(&mut **tx).await?.try_get("invalid")?;
    if invalid != 0 {
        return Err(DurableAuthError::UnsupportedSchema.into());
    }
    Ok(())
}
