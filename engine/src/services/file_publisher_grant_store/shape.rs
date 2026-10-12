use super::*;

pub(super) async fn relations(tx: &mut Tx<'_>) -> Result<[i64; 3]> {
    let rows = sqlx::query("WITH expected(relation, key, position) AS (VALUES
        ('collaboration_file_publisher_grant_schema', ARRAY['singleton'], 0),
        ('collaboration_file_publisher_grants', ARRAY['grant_id'], 1),
        ('collaboration_file_publisher_grant_changes', ARRAY['grant_id','revision'], 2)),
        columns(relation, name, type_name) AS (VALUES
        ('collaboration_file_publisher_grant_schema', 'singleton', 'bool'),
        ('collaboration_file_publisher_grant_schema', 'version', 'int4'),
        ('collaboration_file_publisher_grant_schema', 'authority_id', 'uuid'),
        ('collaboration_file_publisher_grant_schema', 'workspace_id', 'uuid'),
        ('collaboration_file_publisher_grant_schema', 'installation_id', 'uuid'),
        ('collaboration_file_publisher_grant_schema', 'namespace_oid', 'int8'),
        ('collaboration_file_publisher_grant_schema', 'schema_oid', 'int8'),
        ('collaboration_file_publisher_grant_schema', 'grants_oid', 'int8'),
        ('collaboration_file_publisher_grant_schema', 'changes_oid', 'int8'),
        ('collaboration_file_publisher_grant_schema', 'max_grants', 'int4'),
        ('collaboration_file_publisher_grants', 'grant_id', 'uuid'),
        ('collaboration_file_publisher_grants', 'subject_id', 'uuid'),
        ('collaboration_file_publisher_grants', 'subject_account_id', 'uuid'),
        ('collaboration_file_publisher_grants', 'issuer_id', 'uuid'),
        ('collaboration_file_publisher_grants', 'issuer_account_id', 'uuid'),
        ('collaboration_file_publisher_grants', 'revision', 'int8'),
        ('collaboration_file_publisher_grants', 'not_before', 'int8'),
        ('collaboration_file_publisher_grants', 'expires_at', 'int8'),
        ('collaboration_file_publisher_grants', 'grant_payload', 'bytea'),
        ('collaboration_file_publisher_grant_changes', 'grant_id', 'uuid'),
        ('collaboration_file_publisher_grant_changes', 'revision', 'int8'),
        ('collaboration_file_publisher_grant_changes', 'actor_id', 'uuid'),
        ('collaboration_file_publisher_grant_changes', 'actor_account_id', 'uuid'),
        ('collaboration_file_publisher_grant_changes', 'not_before', 'int8'),
        ('collaboration_file_publisher_grant_changes', 'expires_at', 'int8'),
        ('collaboration_file_publisher_grant_changes', 'recorded_at', 'int8'),
        ('collaboration_file_publisher_grant_changes', 'grant_payload', 'bytea'))
        SELECT c.oid::bigint AS oid, c.relkind::text AS kind, c.relpersistence::text AS persistence,
        c.relrowsecurity AS rls, c.relispartition AS partition,
        to_regclass(c.relname)::oid = c.oid AS resolved,
        EXISTS(SELECT 1 FROM pg_catalog.pg_inherits i WHERE i.inhrelid=c.oid OR i.inhparent=c.oid) AS inheritance,
        (NOT c.relforcerowsecurity AND
            EXISTS(SELECT 1 FROM pg_catalog.pg_constraint p
                JOIN pg_catalog.pg_index idx ON idx.indexrelid=p.conindid
                WHERE p.conrelid=c.oid AND p.contype='p' AND p.convalidated
                AND NOT p.condeferrable AND idx.indisvalid AND idx.indisready
                AND e.key=ARRAY(SELECT a.attname::text FROM unnest(p.conkey)
                    WITH ORDINALITY k(n,position) JOIN pg_catalog.pg_attribute a
                    ON a.attrelid=c.oid AND a.attnum=k.n ORDER BY k.position))
            AND (SELECT count(*) FROM pg_catalog.pg_attribute a WHERE a.attrelid=c.oid
                AND a.attnum>0 AND NOT a.attisdropped)=(SELECT count(*) FROM columns x WHERE x.relation=e.relation)
            AND NOT EXISTS(SELECT 1 FROM columns x LEFT JOIN pg_catalog.pg_attribute a
                ON a.attrelid=c.oid AND a.attname=x.name AND a.attnum>0 AND NOT a.attisdropped
                WHERE x.relation=e.relation AND (a.attnum IS NULL OR NOT a.attnotnull
                    OR a.atttypid<>x.type_name::regtype OR a.atttypmod<>-1
                    OR a.attgenerated<>'' OR a.attidentity<>''))) AS shape
        FROM expected e JOIN pg_catalog.pg_class c ON c.relname=e.relation
        AND c.relnamespace=current_schema()::regnamespace ORDER BY e.position")
        .fetch_all(&mut **tx).await?;
    if !prepared_resource_sql::ordinary_tables(&rows, 3)? {
        return Err(FilePublisherGrantStoreError::UnsupportedSchema);
    }
    for row in &rows {
        if !row.try_get::<bool, _>("shape")? {
            return Err(FilePublisherGrantStoreError::UnsupportedSchema);
        }
    }
    Ok([
        rows[0].try_get("oid")?,
        rows[1].try_get("oid")?,
        rows[2].try_get("oid")?,
    ])
}
