use super::*;

pub(super) async fn verify(
    connection: &mut PgConnection,
    scope: WorkspaceRef,
) -> CheckResult<DateTime<Utc>> {
    let row = sqlx::query("SELECT current_schema() IS NOT NULL
        AND left(current_schema(), 3) <> 'pg_' AND current_schema() <> 'information_schema'
        AND current_schemas(false)=ARRAY[current_schema()]::name[] AND pg_my_temp_schema()=0 AS valid,
        clock_timestamp() AS observed_at").fetch_one(&mut *connection).await?;
    if !row.try_get::<bool, _>("valid")? {
        return Err(ReconciliationError::InvalidNamespace);
    }
    let observed_at = row.try_get("observed_at")?;
    // A view, inherited relation or RLS-filtered table could silently omit rows. Refuse such
    // layouts even for a superuser, and require the expected PK coordinates before observation.
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
        WHERE t.oid IS NULL OR t.relkind<>'r' OR t.relpersistence<>'p' OR t.relrowsecurity
        OR t.relforcerowsecurity OR t.relhassubclass OR t.relispartition
        OR EXISTS (SELECT 1 FROM pg_catalog.pg_inherits i WHERE i.inhrelid=t.oid) OR NOT EXISTS (
            SELECT 1 FROM pg_catalog.pg_constraint c WHERE c.conrelid=t.oid AND c.contype='p'
            AND c.convalidated AND NOT c.condeferrable AND e.key=ARRAY(
                SELECT a.attname::text FROM unnest(c.conkey) WITH ORDINALITY k(n,position)
                JOIN pg_catalog.pg_attribute a ON a.attrelid=t.oid AND a.attnum=k.n
                WHERE a.attnotnull AND NOT a.attisdropped ORDER BY k.position))")
        .fetch_one(&mut *connection).await?.try_get("invalid")?;
    if invalid != 0 {
        return Err(ReconciliationError::UnsupportedSchema);
    }
    columns::verify(connection).await?;
    for (table, expected) in [
        ("collaboration_auth_source_schema", 1),
        ("collaboration_identity_schema", 2),
        ("collaboration_access_schema", 2),
    ] {
        let rows = sqlx::query(&format!("SELECT singleton, version FROM {table} LIMIT 2"))
            .fetch_all(&mut *connection)
            .await?;
        if rows.len() != 1
            || !rows[0].try_get::<bool, _>("singleton")?
            || rows[0].try_get::<i32, _>("version")? != expected
        {
            return Err(ReconciliationError::UnsupportedSchema);
        }
    }
    let authority: Uuid =
        sqlx::query("SELECT authority_id FROM collaboration_auth_source_schema WHERE singleton")
            .fetch_one(&mut *connection)
            .await?
            .try_get("authority_id")?;
    if authority != scope.authority_id.as_uuid() {
        return Err(ReconciliationError::ScopeMismatch);
    }
    DurableAuthSource::verify_schema(connection)
        .await
        .map_err(|e| match e {
            DurableAuthError::StorageUnavailable => ReconciliationError::StorageUnavailable,
            _ => ReconciliationError::UnsupportedSchema,
        })?;
    Ok(observed_at)
}
