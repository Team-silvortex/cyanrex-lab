//! Runtime relation identity/shape, not installation or a full historical audit.
use super::*;

pub(super) struct RelationPin {
    oids: Vec<i64>,
    registry: bool,
}
impl RelationPin {
    /// Source-only Session operations require only the three auth tables. Existing private work
    /// also validates the registry tables in the same order and with the same shape restrictions.
    pub async fn capture(tx: &mut Transaction<'_, Postgres>, registry: bool) -> Result<Self> {
        let lock = if registry {
            "LOCK TABLE users, sessions, collaboration_auth_source_schema,
            collaboration_identity_schema, collaboration_access_schema, collaboration_authorities,
            collaboration_workspaces, collaboration_legacy_workspaces, collaboration_principals,
            collaboration_legacy_identities, collaboration_memberships, collaboration_deployment_grants,
            collaboration_identity_audit, collaboration_policy_audit IN ACCESS SHARE MODE"
        } else {
            "LOCK TABLE users, sessions, collaboration_auth_source_schema IN ACCESS SHARE MODE"
        };
        sqlx::query(lock).execute(&mut **tx).await?;
        let rows = sqlx::query("WITH expected(relation, key, registry) AS (VALUES
            ('users', ARRAY['username'], false), ('sessions', ARRAY['token'], false),
            ('collaboration_auth_source_schema', ARRAY['singleton'], false),
            ('collaboration_identity_schema', ARRAY['singleton'], true),
            ('collaboration_access_schema', ARRAY['singleton'], true),
            ('collaboration_authorities', ARRAY['authority_id'], true),
            ('collaboration_workspaces', ARRAY['authority_id','workspace_id'], true),
            ('collaboration_legacy_workspaces', ARRAY['authority_id'], true),
            ('collaboration_principals', ARRAY['authority_id','principal_id'], true),
            ('collaboration_legacy_identities', ARRAY['authority_id','username','account_id'], true),
            ('collaboration_memberships', ARRAY['authority_id','workspace_id','principal_id'], true),
            ('collaboration_deployment_grants', ARRAY['authority_id','principal_id'], true),
            ('collaboration_identity_audit', ARRAY['sequence'], true),
            ('collaboration_policy_audit', ARRAY['sequence'], true))
            SELECT t.oid::bigint AS oid, (t.oid IS NOT NULL
                AND to_regclass(e.relation)::oid = t.oid AND t.relkind='r'
                AND t.relpersistence='p' AND NOT t.relrowsecurity AND NOT t.relforcerowsecurity
                AND NOT t.relhassubclass AND NOT t.relispartition
                AND NOT EXISTS (SELECT 1 FROM pg_catalog.pg_inherits i WHERE i.inhrelid=t.oid)
                AND EXISTS (SELECT 1 FROM pg_catalog.pg_constraint c WHERE c.conrelid=t.oid
                    AND c.contype='p' AND c.convalidated AND NOT c.condeferrable
                    AND e.key=ARRAY(SELECT a.attname::text FROM unnest(c.conkey)
                        WITH ORDINALITY k(n,position) JOIN pg_catalog.pg_attribute a
                        ON a.attrelid=t.oid AND a.attnum=k.n
                        WHERE a.attnotnull AND NOT a.attisdropped ORDER BY k.position))) AS valid
            FROM expected e LEFT JOIN pg_catalog.pg_class t
            ON t.relnamespace=current_schema()::regnamespace AND t.relname=e.relation
            WHERE NOT e.registry OR $1 ORDER BY e.relation")
            .bind(registry).fetch_all(&mut **tx).await?;
        let mut oids = Vec::with_capacity(rows.len());
        for row in rows {
            if !row.try_get::<bool, _>("valid")? {
                return Err(DurableAuthError::UnsupportedSchema);
            }
            oids.push(row.try_get("oid")?);
        }
        Ok(Self { oids, registry })
    }

    pub async fn verify(&self, tx: &mut Transaction<'_, Postgres>) -> Result<()> {
        if Self::capture(tx, self.registry).await?.oids != self.oids {
            return Err(DurableAuthError::SourceMismatch);
        }
        Ok(())
    }
}
