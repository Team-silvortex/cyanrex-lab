use super::*;

/// Original relation identity, with a private full-step snapshot for a begun dispatch only.
pub(crate) struct IntentRelations {
    version: i32,
    oids: Vec<i64>,
    pub(super) expected_steps: Option<Vec<DraftDispatchRecord>>,
}
impl IntentRelations {
    pub(crate) async fn verify(&self, journal: &DraftIntentJournal, tx: &mut Tx<'_>) -> Result<()> {
        let current = journal.check_schema(tx).await?;
        if current.version != self.version || current.oids != self.oids {
            return Err(DraftIntentJournalError::UnsupportedSchema);
        }
        Ok(())
    }

    pub(super) fn require_dispatch(&self) -> Result<()> {
        if self.version != 2 {
            return Err(DraftIntentJournalError::UnsupportedSchema);
        }
        Ok(())
    }
}

impl DraftIntentJournal {
    /// Explicit schema-1 fresh namespace only. Never adopts, repairs or migrates existing data.
    pub async fn install_empty_namespace(&self) -> Result<()> {
        self.install(1).await
    }

    /// Explicit schema-2 fresh namespace only. Schema 1 must not be upgraded in place.
    pub async fn install_empty_dispatch_namespace(&self) -> Result<()> {
        self.install(2).await
    }

    async fn install(&self, version: i32) -> Result<()> {
        bounded(async {
            let mut tx = prepared_resource_sql::begin(&self.pool, false).await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'").execute(&mut *tx).await?;
            if !prepared_resource_sql::namespace(&mut tx).await? {
                return Err(DraftIntentJournalError::UnsupportedSchema);
            }
            sqlx::query("SELECT pg_advisory_xact_lock(hashtext(current_database()), hashtext('cyanrex.draft-intents.' || current_schema()))")
                .execute(&mut *tx).await?;
            let occupied: bool = sqlx::query("SELECT EXISTS(SELECT 1 FROM pg_class WHERE relnamespace = current_schema()::regnamespace)
                OR EXISTS(SELECT 1 FROM pg_proc WHERE pronamespace = current_schema()::regnamespace)
                OR EXISTS(SELECT 1 FROM pg_type WHERE typnamespace = current_schema()::regnamespace) AS occupied")
                .fetch_one(&mut *tx).await?.try_get("occupied")?;
            if occupied { return Err(DraftIntentJournalError::NamespaceNotEmpty); }
            let base = include_str!("../../../migrations/0016_collaboration_draft_intents.sql");
            let steps = if version == 2 {
                include_str!("../../../migrations/0017_collaboration_draft_dispatch.sql")
            } else { "" };
            for statement in [base, steps].into_iter().flat_map(|ddl| ddl.split(';'))
                .map(str::trim).filter(|s| !s.is_empty()) {
                sqlx::query(statement).execute(&mut *tx).await?;
            }
            confirmed_one(sqlx::query("INSERT INTO collaboration_draft_intent_schema
                (singleton, version, authority_id, workspace_id, installation_id) VALUES(TRUE, $1, $2, $3, $4)")
                .bind(version).bind(self.scope.authority_id.as_uuid()).bind(self.scope.workspace_id.as_uuid())
                .bind(self.installation_id).execute(&mut *tx).await?.rows_affected())?;
            let installed = self.check_schema(&mut tx).await?;
            if installed.version != version { return Err(DraftIntentJournalError::UnsupportedSchema); }
            tx.commit().await?;
            Ok(())
        }).await
    }

    pub(crate) async fn check_schema(&self, tx: &mut Tx<'_>) -> Result<IntentRelations> {
        if !prepared_resource_sql::namespace(tx).await? {
            return Err(DraftIntentJournalError::UnsupportedSchema);
        }
        sqlx::query("LOCK TABLE collaboration_draft_intent_schema, collaboration_draft_intents IN ACCESS SHARE MODE")
            .execute(&mut **tx).await?;
        // Check the original two tables before reading metadata or touching a schema-2 relation.
        let base = relation_shapes(tx, false).await?;
        let markers = sqlx::query(
            "SELECT singleton, version, authority_id, workspace_id, installation_id
            FROM collaboration_draft_intent_schema LIMIT 2 FOR UPDATE",
        )
        .fetch_all(&mut **tx)
        .await?;
        if markers.len() != 1 {
            return Err(DraftIntentJournalError::UnsupportedSchema);
        }
        let row = &markers[0];
        let version: i32 = row.try_get("version")?;
        if !row.try_get::<bool, _>("singleton")? || !matches!(version, 1 | 2) {
            return Err(DraftIntentJournalError::UnsupportedSchema);
        }
        if row.try_get::<Uuid, _>("authority_id")? != self.scope.authority_id.as_uuid()
            || row.try_get::<Uuid, _>("workspace_id")? != self.scope.workspace_id.as_uuid()
        {
            return Err(DraftIntentJournalError::ScopeMismatch);
        }
        if row.try_get::<Uuid, _>("installation_id")? != self.installation_id {
            return Err(DraftIntentJournalError::IncarnationMismatch);
        }
        let oids = if version == 2 {
            sqlx::query("LOCK TABLE collaboration_draft_dispatch_steps IN ACCESS SHARE MODE")
                .execute(&mut **tx)
                .await?;
            let all = relation_shapes(tx, true).await?;
            if all.get(..2) != Some(base.as_slice()) {
                return Err(DraftIntentJournalError::UnsupportedSchema);
            }
            all
        } else {
            base
        };
        Ok(IntentRelations {
            version,
            oids,
            expected_steps: None,
        })
    }
}

async fn relation_shapes(tx: &mut Tx<'_>, dispatch: bool) -> Result<Vec<i64>> {
    let rows = sqlx::query("WITH expected(relation, key, position) AS (VALUES
        ('collaboration_draft_intent_schema', ARRAY['singleton'], 0),
        ('collaboration_draft_intents', ARRAY['task_id'], 1),
        ('collaboration_draft_dispatch_steps', ARRAY['task_id', 'ordinal'], 2)),
        columns(relation, name, type_name) AS (VALUES
        ('collaboration_draft_intent_schema', 'singleton', 'bool'),
        ('collaboration_draft_intent_schema', 'version', 'int4'),
        ('collaboration_draft_intent_schema', 'authority_id', 'uuid'),
        ('collaboration_draft_intent_schema', 'workspace_id', 'uuid'),
        ('collaboration_draft_intent_schema', 'installation_id', 'uuid'),
        ('collaboration_draft_intents', 'task_id', 'uuid'),
        ('collaboration_draft_intents', 'owner_id', 'uuid'),
        ('collaboration_draft_intents', 'account_id', 'uuid'),
        ('collaboration_draft_intents', 'source_namespace', 'text'),
        ('collaboration_draft_intents', 'task_namespace', 'text'),
        ('collaboration_draft_intents', 'artifact_namespace', 'text'),
        ('collaboration_draft_intents', 'checkpoint', 'bytea'),
        ('collaboration_draft_dispatch_steps', 'task_id', 'uuid'),
        ('collaboration_draft_dispatch_steps', 'ordinal', 'int4'),
        ('collaboration_draft_dispatch_steps', 'nonce', 'uuid'),
        ('collaboration_draft_dispatch_steps', 'committed', 'bool'))
        SELECT c.oid::bigint AS oid, c.relkind::text AS kind, c.relpersistence::text AS persistence,
        c.relrowsecurity AS rls, c.relispartition AS partition,
        to_regclass(c.relname)::oid = c.oid AS resolved,
        EXISTS(SELECT 1 FROM pg_catalog.pg_inherits i WHERE i.inhrelid = c.oid OR i.inhparent = c.oid) AS inheritance,
        (NOT c.relforcerowsecurity AND
            EXISTS(SELECT 1 FROM pg_catalog.pg_constraint p
                JOIN pg_catalog.pg_index idx ON idx.indexrelid=p.conindid
                WHERE p.conrelid=c.oid AND p.contype='p' AND p.convalidated
                AND NOT p.condeferrable AND idx.indisvalid AND idx.indisready
                AND e.key=ARRAY(SELECT a.attname::text FROM unnest(p.conkey)
                    WITH ORDINALITY k(n,position) JOIN pg_catalog.pg_attribute a
                    ON a.attrelid=c.oid AND a.attnum=k.n ORDER BY k.position))
            AND (SELECT count(*) FROM pg_catalog.pg_attribute a WHERE a.attrelid=c.oid
                AND a.attnum>0 AND NOT a.attisdropped) =
                (SELECT count(*) FROM columns x WHERE x.relation=e.relation)
            AND NOT EXISTS(SELECT 1 FROM columns x LEFT JOIN pg_catalog.pg_attribute a
                ON a.attrelid=c.oid AND a.attname=x.name AND a.attnum>0 AND NOT a.attisdropped
                WHERE x.relation=e.relation AND (a.attnum IS NULL OR NOT a.attnotnull
                    OR a.atttypid<>x.type_name::regtype OR a.atttypmod<>-1
                    OR a.attgenerated<>'' OR a.attidentity<>''))
            AND (e.position<>2 OR (
                EXISTS(SELECT 1 FROM pg_catalog.pg_constraint f
                    WHERE f.conrelid=c.oid AND f.contype='f' AND f.convalidated
                    AND NOT f.condeferrable AND f.confdeltype='a' AND f.confupdtype='a'
                    AND f.confmatchtype='s'
                    AND f.confrelid=to_regclass('collaboration_draft_intents')
                    AND ARRAY(SELECT a.attname::text FROM unnest(f.conkey)
                        WITH ORDINALITY k(n,position) JOIN pg_catalog.pg_attribute a
                        ON a.attrelid=c.oid AND a.attnum=k.n ORDER BY k.position)=ARRAY['task_id']
                    AND ARRAY(SELECT a.attname::text FROM unnest(f.confkey)
                        WITH ORDINALITY k(n,position) JOIN pg_catalog.pg_attribute a
                        ON a.attrelid=f.confrelid AND a.attnum=k.n ORDER BY k.position)=ARRAY['task_id'])
                AND EXISTS(SELECT 1 FROM pg_catalog.pg_constraint u
                    JOIN pg_catalog.pg_index idx ON idx.indexrelid=u.conindid
                    WHERE u.conrelid=c.oid AND u.contype='u' AND u.convalidated
                    AND NOT u.condeferrable AND idx.indisvalid AND idx.indisready
                    AND ARRAY(SELECT a.attname::text FROM unnest(u.conkey)
                        WITH ORDINALITY k(n,position) JOIN pg_catalog.pg_attribute a
                        ON a.attrelid=c.oid AND a.attnum=k.n ORDER BY k.position)=ARRAY['nonce'])
            ))) AS shape
        FROM expected e JOIN pg_catalog.pg_class c ON c.relname=e.relation
        AND c.relnamespace=current_schema()::regnamespace
        WHERE $1 OR e.position<2 ORDER BY e.position")
        .bind(dispatch).fetch_all(&mut **tx).await?;
    if !prepared_resource_sql::ordinary_tables(&rows, if dispatch { 3 } else { 2 })? {
        return Err(DraftIntentJournalError::UnsupportedSchema);
    }
    let mut oids = Vec::with_capacity(rows.len());
    for row in rows {
        if !row.try_get::<bool, _>("shape")? {
            return Err(DraftIntentJournalError::UnsupportedSchema);
        }
        oids.push(row.try_get("oid")?);
    }
    Ok(oids)
}
