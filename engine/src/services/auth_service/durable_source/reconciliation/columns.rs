use super::*;

/// Check every selected field before fetching rows: fixed-width IDs/times must not have been
/// replaced by unbounded text, and nullable values must not bypass metadata-size arithmetic.
pub(super) async fn verify(connection: &mut PgConnection) -> CheckResult<()> {
    let invalid: bool = sqlx::query("WITH layouts(relation, fields) AS (VALUES
        ('users', ARRAY['username','account_id','otp_last_counter']),
        ('sessions', ARRAY['username','account_id','expires_at','token']),
        ('collaboration_auth_source_schema', ARRAY['singleton','version','authority_id']),
        ('collaboration_identity_schema', ARRAY['singleton','version']),
        ('collaboration_access_schema', ARRAY['singleton','version']),
        ('collaboration_authorities', ARRAY['authority_id']),
        ('collaboration_workspaces', ARRAY['authority_id','workspace_id','status']),
        ('collaboration_legacy_workspaces', ARRAY['authority_id','workspace_id']),
        ('collaboration_principals', ARRAY['authority_id','principal_id','kind','display_name','status']),
        ('collaboration_legacy_identities', ARRAY['authority_id','username','account_id','principal_id','retired_at']),
        ('collaboration_memberships', ARRAY['authority_id','workspace_id','principal_id','role_refs','status','revision']),
        ('collaboration_deployment_grants', ARRAY['authority_id','principal_id','source_workspace_id','status','revision']),
        ('collaboration_identity_audit', ARRAY['sequence','authority_id','workspace_id','principal_id',
            'username','account_id','kind','operation','command_id','actor_principal_id','request_digest',
            'recorded_at','before_state','after_state']),
        ('collaboration_policy_audit', ARRAY['sequence','authority_id','workspace_id','principal_id',
            'revision','kind','command_id','actor_principal_id','request_digest','recorded_at','before_state','after_state'])
        ), types(field, type_name, nullable) AS (VALUES
        ('username','text',false), ('token','text',false), ('account_id','uuid',false), ('otp_last_counter','int8',false),
        ('authority_id','uuid',false), ('workspace_id','uuid',false), ('principal_id','uuid',false),
        ('source_workspace_id','uuid',false), ('actor_principal_id','uuid',true), ('command_id','uuid',true),
        ('singleton','bool',false), ('version','int4',false), ('sequence','int8',false), ('revision','int8',false),
        ('kind','text',false), ('operation','text',true), ('status','text',false), ('display_name','text',false),
        ('request_digest','text',true), ('role_refs','_text',false), ('before_state','jsonb',true), ('after_state','jsonb',false),
        ('expires_at','timestamptz',false), ('retired_at','timestamptz',true), ('recorded_at','timestamptz',false)
        ) SELECT EXISTS (SELECT 1 FROM layouts l CROSS JOIN LATERAL unnest(l.fields) f(name)
        LEFT JOIN types e ON e.field=f.name
        LEFT JOIN pg_catalog.pg_class t ON t.relnamespace=current_schema()::regnamespace AND t.relname=l.relation
        LEFT JOIN pg_catalog.pg_attribute a ON a.attrelid=t.oid AND a.attname=f.name AND NOT a.attisdropped
        LEFT JOIN pg_catalog.pg_type ty ON ty.typname=e.type_name AND ty.typnamespace='pg_catalog'::regnamespace
        WHERE a.attnum IS NULL OR e.field IS NULL OR a.atttypid IS DISTINCT FROM ty.oid
            OR (NOT e.nullable AND NOT a.attnotnull)) AS invalid")
        .fetch_one(connection).await?.try_get("invalid")?;
    if invalid {
        return Err(ReconciliationError::UnsupportedSchema);
    }
    Ok(())
}
