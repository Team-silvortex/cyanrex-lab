//! Runtime relation identity/shape, not installation or a full historical audit.
use super::*;
use sqlx_core::executor::Executor;

pub(super) struct RelationPin {
    oids: Vec<i64>,
    registry: bool,
    private_index: bool,
}
impl RelationPin {
    /// Source-only Session operations require only the three auth tables. Existing private work
    /// also validates the registry tables in the same order and with the same shape restrictions.
    pub async fn capture(tx: &mut Transaction<'_, Postgres>, registry: bool) -> Result<Self> {
        Self::capture_mode(tx, registry, false).await
    }

    /// Index-only roles need relation locks without credential-wide SELECT. This retains the
    /// same fourteen-table identity/shape checks and proves each actual transaction-owned lock.
    pub async fn capture_private_index(tx: &mut Transaction<'_, Postgres>) -> Result<Self> {
        Self::capture_mode(tx, true, true).await
    }

    async fn capture_mode(
        tx: &mut Transaction<'_, Postgres>,
        registry: bool,
        private_index: bool,
    ) -> Result<Self> {
        if private_index {
            // Parse/rewrite only, never plan or execute this SELECT. One reserved name bounds
            // interrupted work; collisions fail without removing or adopting an existing plan.
            // DEALLOCATE leaves all relation locks held by this same business transaction.
            Executor::execute(
                &mut **tx,
                "PREPARE cyanrex_private_index_relations_v1 AS
                SELECT 1 FROM ONLY users, ONLY sessions,
                    ONLY collaboration_auth_source_schema, ONLY collaboration_identity_schema,
                    ONLY collaboration_access_schema, ONLY collaboration_authorities,
                    ONLY collaboration_workspaces, ONLY collaboration_legacy_workspaces,
                    ONLY collaboration_principals, ONLY collaboration_legacy_identities,
                    ONLY collaboration_memberships, ONLY collaboration_deployment_grants,
                    ONLY collaboration_identity_audit, ONLY collaboration_policy_audit;
                DEALLOCATE cyanrex_private_index_relations_v1",
            )
            .await?;
        } else if registry {
            sqlx::query("LOCK TABLE users, sessions, collaboration_auth_source_schema,
            collaboration_identity_schema, collaboration_access_schema, collaboration_authorities,
            collaboration_workspaces, collaboration_legacy_workspaces, collaboration_principals,
            collaboration_legacy_identities, collaboration_memberships, collaboration_deployment_grants,
            collaboration_identity_audit, collaboration_policy_audit IN ACCESS SHARE MODE")
                .execute(&mut **tx).await?;
        } else {
            // LOCK TABLE requires table-wide SELECT, unnecessarily exposing credentials to a
            // Session-only role. PREPARE parses/rewrites but never plans or executes this query;
            // DEALLOCATE releases the prepared statement while transaction relation locks remain. Use one
            // Simple Query batch, not a cached/extended prepared query or an executed LIMIT 0.
            // The fixed reserved name bounds an interrupted batch to one slot per connection.
            // A collision fails closed: never execute, adopt, remove or retry an existing plan.
            Executor::execute(
                &mut **tx,
                "PREPARE cyanrex_session_source_relations_v1 AS
                SELECT 1 FROM ONLY users, ONLY sessions,
                    ONLY collaboration_auth_source_schema LIMIT 0;
                DEALLOCATE cyanrex_session_source_relations_v1",
            )
            .await?;
        }
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
        if !registry {
            if oids.len() != 3 {
                return Err(DurableAuthError::UnsupportedSchema);
            }
            let held: bool = sqlx::query(
                "SELECT count(DISTINCT l.relation)=3 AS held
                FROM pg_catalog.pg_locks l
                WHERE l.pid=pg_catalog.pg_backend_pid() AND l.locktype='relation'
                    AND l.database=(SELECT d.oid FROM pg_catalog.pg_database d
                        WHERE d.datname=pg_catalog.current_database())
                    AND l.relation::bigint = ANY($1)
                    AND l.mode='AccessShareLock' AND l.granted",
            )
            .bind(&oids)
            .fetch_one(&mut **tx)
            .await?
            .try_get("held")?;
            if !held {
                return Err(DurableAuthError::UnsupportedSchema);
            }
        } else if private_index {
            if oids.len() != 14 {
                return Err(DurableAuthError::UnsupportedSchema);
            }
            let held: bool = sqlx::query(
                "SELECT count(DISTINCT l.relation)=14 AS held
                FROM pg_catalog.pg_locks l
                WHERE l.pid=pg_catalog.pg_backend_pid() AND l.locktype='relation'
                    AND l.database=(SELECT d.oid FROM pg_catalog.pg_database d
                        WHERE d.datname=pg_catalog.current_database())
                    AND l.relation::bigint = ANY($1)
                    AND l.mode='AccessShareLock' AND l.granted",
            )
            .bind(&oids)
            .fetch_one(&mut **tx)
            .await?
            .try_get("held")?;
            if !held {
                return Err(DurableAuthError::UnsupportedSchema);
            }
        }
        Ok(Self {
            oids,
            registry,
            private_index,
        })
    }

    pub async fn verify(&self, tx: &mut Transaction<'_, Postgres>) -> Result<()> {
        let current = if self.private_index {
            Self::capture_private_index(tx).await?
        } else {
            Self::capture(tx, self.registry).await?
        };
        if current.oids != self.oids {
            return Err(DurableAuthError::SourceMismatch);
        }
        Ok(())
    }
}
