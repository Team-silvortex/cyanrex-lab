use super::*;
use namespace::NamespacePin;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct SchemaPin {
    namespace: NamespacePin,
    relations: [i64; 2],
    limits: FileUploadLimits,
}

impl SchemaPin {
    #[cfg(unix)]
    pub(super) fn source_identity(&self) -> (i64, [i64; 2]) {
        (self.namespace.oid, self.relations)
    }

    pub(super) async fn verify(
        &self,
        store: &FileUploadStore,
        tx: &mut Tx<'_>,
        writer: bool,
    ) -> Result<()> {
        // Never erase changed search_path or silently capture a replacement before this check.
        self.namespace.verify(store, tx).await?;
        if store.check_schema(tx, writer).await? != *self {
            return Err(FileUploadStoreError::UnsupportedSchema);
        }
        Ok(())
    }
}

impl FileUploadStore {
    pub(super) async fn begin(&self, readonly: bool) -> Result<Tx<'_>> {
        let mut tx = prepared_resource_sql::begin(&self.pool, readonly).await?;
        if !readonly {
            // A stale REPEATABLE READ snapshot created before a lock wait must not bypass quota.
            // Preserve a caller-configured read-only policy rather than forcing READ WRITE.
            sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
                .execute(&mut *tx)
                .await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'")
                .execute(&mut *tx)
                .await?;
        }
        Ok(tx)
    }

    /// Explicit fresh namespace only. No repair, migration, repeat adoption or runtime installer.
    pub async fn install_empty_namespace(&self) -> Result<()> {
        bounded(async {
            let mut tx = self.begin(false).await?;
            let namespace = NamespacePin::capture(self, &mut tx).await?;
            sqlx::query("SELECT pg_advisory_xact_lock(hashtext(current_database()),
                hashtext('cyanrex.file-uploads.' || current_schema()))")
                .execute(&mut *tx).await?;
            namespace.verify(self, &mut tx).await?;
            let occupied: bool = sqlx::query("SELECT EXISTS(SELECT 1 FROM pg_class WHERE relnamespace = current_schema()::regnamespace)
                OR EXISTS(SELECT 1 FROM pg_proc WHERE pronamespace = current_schema()::regnamespace)
                OR EXISTS(SELECT 1 FROM pg_type WHERE typnamespace = current_schema()::regnamespace) AS occupied")
                .fetch_one(&mut *tx).await?.try_get("occupied")?;
            if occupied { return Err(FileUploadStoreError::NamespaceNotEmpty); }
            for statement in include_str!("../../../migrations/0018_collaboration_file_uploads.sql")
                .split(';').map(str::trim).filter(|statement| !statement.is_empty()) {
                sqlx::query(statement).execute(&mut *tx).await?;
            }
            namespace.verify(self, &mut tx).await?;
            let relations = relation_shapes(&mut tx).await?;
            confirmed_one(sqlx::query("INSERT INTO collaboration_file_upload_schema
                (singleton, version, authority_id, workspace_id, installation_id, namespace_oid,
                schema_oid, uploads_oid, total_bytes, owner_bytes, total_uploads, owner_uploads)
                VALUES(TRUE, 1, $1, $2, $3, $4, $5, $6, $7, $8, $9, $10)")
                .bind(self.scope.authority_id.as_uuid()).bind(self.scope.workspace_id.as_uuid())
                .bind(self.installation_id).bind(namespace.oid).bind(relations[0]).bind(relations[1])
                .bind(self.limits.total_bytes as i64).bind(self.limits.owner_bytes as i64)
                .bind(self.limits.total_uploads as i32).bind(self.limits.owner_uploads as i32)
                .execute(&mut *tx).await?.rows_affected())?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'").execute(&mut *tx).await?;
            sqlx::query("SET CONSTRAINTS ALL IMMEDIATE").execute(&mut *tx).await?;
            namespace.verify(self, &mut tx).await?;
            let installed = self.check_schema(&mut tx, true).await?;
            if installed.namespace != namespace || installed.relations != relations {
                return Err(FileUploadStoreError::UnsupportedSchema);
            }
            installed.verify(self, &mut tx, true).await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'").execute(&mut *tx).await?;
            tx.commit().await?;
            Ok(())
        }).await
    }

    pub(super) async fn check_schema(&self, tx: &mut Tx<'_>, writer: bool) -> Result<SchemaPin> {
        let namespace = NamespacePin::capture(self, tx).await?;
        // Writers fence even ordinary SQL DML, not only cooperative stores using the marker row.
        // Every reserve takes both tables in this order, avoiding later lock-upgrade deadlocks.
        let lock = if writer {
            "LOCK TABLE collaboration_file_upload_schema, collaboration_file_uploads IN SHARE ROW EXCLUSIVE MODE"
        } else {
            "LOCK TABLE collaboration_file_upload_schema, collaboration_file_uploads IN ACCESS SHARE MODE"
        };
        sqlx::query(lock).execute(&mut **tx).await?;
        namespace.verify(self, tx).await?;
        let relations = relation_shapes(tx).await?;
        let query = if writer {
            "SELECT singleton, version, authority_id, workspace_id, installation_id, namespace_oid,
            schema_oid, uploads_oid, total_bytes, owner_bytes, total_uploads, owner_uploads
            FROM collaboration_file_upload_schema LIMIT 2 FOR UPDATE"
        } else {
            "SELECT singleton, version, authority_id, workspace_id, installation_id, namespace_oid,
            schema_oid, uploads_oid, total_bytes, owner_bytes, total_uploads, owner_uploads
            FROM collaboration_file_upload_schema LIMIT 2"
        };
        let rows = sqlx::query(query).fetch_all(&mut **tx).await?;
        if rows.len() != 1 {
            return Err(FileUploadStoreError::UnsupportedSchema);
        }
        let row = &rows[0];
        if !row.try_get::<bool, _>("singleton")? || row.try_get::<i32, _>("version")? != 1 {
            return Err(FileUploadStoreError::UnsupportedSchema);
        }
        if row.try_get::<Uuid, _>("authority_id")? != self.scope.authority_id.as_uuid()
            || row.try_get::<Uuid, _>("workspace_id")? != self.scope.workspace_id.as_uuid()
        {
            return Err(FileUploadStoreError::ScopeMismatch);
        }
        if row.try_get::<Uuid, _>("installation_id")? != self.installation_id {
            return Err(FileUploadStoreError::IncarnationMismatch);
        }
        if row.try_get::<i64, _>("namespace_oid")? != namespace.oid
            || row.try_get::<i64, _>("schema_oid")? != relations[0]
            || row.try_get::<i64, _>("uploads_oid")? != relations[1]
        {
            return Err(FileUploadStoreError::UnsupportedSchema);
        }
        let invalid = || FileUploadStoreError::InvalidRecord;
        let limits = FileUploadLimits::new(
            u64::try_from(row.try_get::<i64, _>("total_bytes")?).map_err(|_| invalid())?,
            u64::try_from(row.try_get::<i64, _>("owner_bytes")?).map_err(|_| invalid())?,
            u32::try_from(row.try_get::<i32, _>("total_uploads")?).map_err(|_| invalid())?,
            u32::try_from(row.try_get::<i32, _>("owner_uploads")?).map_err(|_| invalid())?,
        )
        .map_err(|_| invalid())?;
        if limits != self.limits {
            return Err(FileUploadStoreError::UnsupportedSchema);
        }
        namespace.verify(self, tx).await?;
        Ok(SchemaPin {
            namespace,
            relations,
            limits,
        })
    }
}

async fn relation_shapes(tx: &mut Tx<'_>) -> Result<[i64; 2]> {
    let rows = sqlx::query("WITH expected(relation, key, position) AS (VALUES
        ('collaboration_file_upload_schema', ARRAY['singleton'], 0),
        ('collaboration_file_uploads', ARRAY['upload_id'], 1)),
        columns(relation, name, type_name) AS (VALUES
        ('collaboration_file_upload_schema', 'singleton', 'bool'),
        ('collaboration_file_upload_schema', 'version', 'int4'),
        ('collaboration_file_upload_schema', 'authority_id', 'uuid'),
        ('collaboration_file_upload_schema', 'workspace_id', 'uuid'),
        ('collaboration_file_upload_schema', 'installation_id', 'uuid'),
        ('collaboration_file_upload_schema', 'namespace_oid', 'int8'),
        ('collaboration_file_upload_schema', 'schema_oid', 'int8'),
        ('collaboration_file_upload_schema', 'uploads_oid', 'int8'),
        ('collaboration_file_upload_schema', 'total_bytes', 'int8'),
        ('collaboration_file_upload_schema', 'owner_bytes', 'int8'),
        ('collaboration_file_upload_schema', 'total_uploads', 'int4'),
        ('collaboration_file_upload_schema', 'owner_uploads', 'int4'),
        ('collaboration_file_uploads', 'upload_id', 'uuid'),
        ('collaboration_file_uploads', 'owner_id', 'uuid'),
        ('collaboration_file_uploads', 'account_id', 'uuid'),
        ('collaboration_file_uploads', 'manifest', 'bytea'))
        SELECT c.oid::bigint AS oid, c.relkind::text AS kind, c.relpersistence::text AS persistence,
        c.relrowsecurity AS rls, c.relispartition AS partition,
        to_regclass(c.relname)::oid = c.oid AS resolved,
        EXISTS(SELECT 1 FROM pg_catalog.pg_inherits i WHERE i.inhrelid = c.oid OR i.inhparent = c.oid) AS inheritance,
        (NOT c.relforcerowsecurity AND
            EXISTS(SELECT 1 FROM pg_catalog.pg_constraint p
                JOIN pg_catalog.pg_index idx ON idx.indexrelid = p.conindid
                WHERE p.conrelid = c.oid AND p.contype = 'p' AND p.convalidated
                AND NOT p.condeferrable AND idx.indisvalid AND idx.indisready
                AND e.key = ARRAY(SELECT a.attname::text FROM unnest(p.conkey)
                    WITH ORDINALITY k(n, position) JOIN pg_catalog.pg_attribute a
                    ON a.attrelid = c.oid AND a.attnum = k.n ORDER BY k.position))
            AND (SELECT count(*) FROM pg_catalog.pg_attribute a WHERE a.attrelid = c.oid
                AND a.attnum > 0 AND NOT a.attisdropped) =
                (SELECT count(*) FROM columns x WHERE x.relation = e.relation)
            AND NOT EXISTS(SELECT 1 FROM columns x LEFT JOIN pg_catalog.pg_attribute a
                ON a.attrelid = c.oid AND a.attname = x.name AND a.attnum > 0 AND NOT a.attisdropped
                WHERE x.relation = e.relation AND (a.attnum IS NULL OR NOT a.attnotnull
                    OR a.atttypid <> x.type_name::regtype OR a.atttypmod <> -1
                    OR a.attgenerated <> '' OR a.attidentity <> ''))) AS shape
        FROM expected e JOIN pg_catalog.pg_class c ON c.relname = e.relation
        AND c.relnamespace = current_schema()::regnamespace ORDER BY e.position")
        .fetch_all(&mut **tx).await?;
    if !prepared_resource_sql::ordinary_tables(&rows, 2)? {
        return Err(FileUploadStoreError::UnsupportedSchema);
    }
    for row in &rows {
        if !row.try_get::<bool, _>("shape")? {
            return Err(FileUploadStoreError::UnsupportedSchema);
        }
    }
    Ok([rows[0].try_get("oid")?, rows[1].try_get("oid")?])
}
