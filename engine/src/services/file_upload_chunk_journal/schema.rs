use super::*;
use namespace::NamespacePin;

/// Original private namespace, configuration and relation identity; not a restored dispatch grant.
pub(crate) struct JournalSchemaPin {
    namespace: NamespacePin,
    relations: [i64; 2],
    scope: WorkspaceRef,
    installation_id: Uuid,
    max_entries: u32,
}

impl JournalSchemaPin {
    // Binds only this configured namespace/installation/relation identity, not a database cluster.
    pub(super) fn publication_identity(&self, namespace: &str) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(b"cyanrex.file-upload.chunk-schema-publication.v1\0");
        hash.update((namespace.len() as u32).to_be_bytes());
        hash.update(namespace.as_bytes());
        hash.update(self.namespace.oid.to_be_bytes());
        for relation in self.relations {
            hash.update(relation.to_be_bytes());
        }
        hash.update(self.scope.authority_id.as_uuid().as_bytes());
        hash.update(self.scope.workspace_id.as_uuid().as_bytes());
        hash.update(self.installation_id.as_bytes());
        hash.update(self.max_entries.to_be_bytes());
        hash.finalize().into()
    }

    pub(super) fn retain(&self) -> Self {
        Self {
            namespace: self.namespace.clone(),
            relations: self.relations,
            scope: self.scope,
            installation_id: self.installation_id,
            max_entries: self.max_entries,
        }
    }

    pub(super) async fn verify(
        &self,
        journal: &FileUploadChunkJournal,
        tx: &mut Tx<'_>,
        writer: bool,
    ) -> Result<()> {
        // Do not reset a changed raw search_path or adopt replacement relations as a fresh pin.
        self.namespace.verify(journal, tx).await?;
        let current = journal.check_schema(tx, writer).await?;
        if current.namespace != self.namespace
            || current.relations != self.relations
            || current.scope != self.scope
            || current.installation_id != self.installation_id
            || current.max_entries != self.max_entries
        {
            return Err(FileUploadChunkJournalError::UnsupportedSchema);
        }
        Ok(())
    }
}

impl FileUploadChunkJournal {
    pub(super) async fn begin(&self, readonly: bool) -> Result<Tx<'_>> {
        let mut tx = prepared_resource_sql::begin(&self.pool, readonly).await?;
        if !readonly {
            // Preserve configured read-only policy, but never use a stale pre-lock writer snapshot.
            sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
                .execute(&mut *tx)
                .await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'")
                .execute(&mut *tx)
                .await?;
        }
        Ok(tx)
    }

    /// Explicit installation in an empty private namespace only. Never repairs or adopts data.
    pub async fn install_empty_namespace(&self) -> Result<()> {
        bounded(async {
            let mut tx = self.begin(false).await?;
            let namespace = NamespacePin::capture(self, &mut tx).await?;
            sqlx::query("SELECT pg_advisory_xact_lock(hashtext(current_database()),
                hashtext('cyanrex.file-upload-chunks.' || current_schema()))").execute(&mut *tx).await?;
            namespace.verify(self, &mut tx).await?;
            let occupied: bool = sqlx::query("SELECT EXISTS(SELECT 1 FROM pg_class WHERE relnamespace=current_schema()::regnamespace)
                OR EXISTS(SELECT 1 FROM pg_proc WHERE pronamespace=current_schema()::regnamespace)
                OR EXISTS(SELECT 1 FROM pg_type WHERE typnamespace=current_schema()::regnamespace) AS occupied")
                .fetch_one(&mut *tx).await?.try_get("occupied")?;
            if occupied { return Err(FileUploadChunkJournalError::NamespaceNotEmpty); }
            for statement in include_str!("../../../migrations/0019_collaboration_file_upload_chunks.sql")
                .split(';').map(str::trim).filter(|statement| !statement.is_empty()) {
                sqlx::query(statement).execute(&mut *tx).await?;
            }
            namespace.verify(self, &mut tx).await?;
            let relations = relation_shapes(&mut tx).await?;
            confirmed_one(sqlx::query("INSERT INTO collaboration_file_upload_chunk_schema
                (singleton,version,authority_id,workspace_id,installation_id,namespace_oid,schema_oid,chunks_oid,max_entries)
                VALUES(TRUE,1,$1,$2,$3,$4,$5,$6,$7)")
                .bind(self.scope.authority_id.as_uuid()).bind(self.scope.workspace_id.as_uuid()).bind(self.installation_id)
                .bind(namespace.oid).bind(relations[0]).bind(relations[1]).bind(self.max_entries as i32)
                .execute(&mut *tx).await?.rows_affected())?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'").execute(&mut *tx).await?;
            sqlx::query("SET CONSTRAINTS ALL IMMEDIATE").execute(&mut *tx).await?;
            namespace.verify(self, &mut tx).await?;
            let installed = self.check_schema(&mut tx, true).await?;
            if installed.namespace != namespace || installed.relations != relations {
                return Err(FileUploadChunkJournalError::UnsupportedSchema);
            }
            if self.ledger(&mut tx).await?.len() != 0 {
                return Err(FileUploadChunkJournalError::InvalidRecord);
            }
            installed.verify(self, &mut tx, true).await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'").execute(&mut *tx).await?;
            tx.commit().await?;
            Ok(())
        }).await
    }

    pub(crate) async fn capture_pin(&self) -> Result<JournalSchemaPin> {
        bounded(async {
            let mut tx = self.begin(true).await?;
            let pin = self.check_schema(&mut tx, false).await?;
            pin.verify(self, &mut tx, false).await?;
            tx.commit().await?;
            Ok(pin)
        })
        .await
    }

    pub(super) async fn check_schema(
        &self,
        tx: &mut Tx<'_>,
        writer: bool,
    ) -> Result<JournalSchemaPin> {
        let namespace = NamespacePin::capture(self, tx).await?;
        let lock = if writer {
            "LOCK TABLE collaboration_file_upload_chunk_schema, collaboration_file_upload_chunks IN SHARE ROW EXCLUSIVE MODE"
        } else {
            "LOCK TABLE collaboration_file_upload_chunk_schema, collaboration_file_upload_chunks IN ACCESS SHARE MODE"
        };
        sqlx::query(lock).execute(&mut **tx).await?;
        namespace.verify(self, tx).await?;
        let relations = relation_shapes(tx).await?;
        let query = if writer {
            "SELECT singleton,version,authority_id,workspace_id,installation_id,namespace_oid,schema_oid,chunks_oid,max_entries
            FROM collaboration_file_upload_chunk_schema LIMIT 2 FOR UPDATE"
        } else {
            "SELECT singleton,version,authority_id,workspace_id,installation_id,namespace_oid,schema_oid,chunks_oid,max_entries
            FROM collaboration_file_upload_chunk_schema LIMIT 2"
        };
        let rows = sqlx::query(query).fetch_all(&mut **tx).await?;
        if rows.len() != 1 {
            return Err(FileUploadChunkJournalError::UnsupportedSchema);
        }
        let row = &rows[0];
        if !row.try_get::<bool, _>("singleton")? || row.try_get::<i32, _>("version")? != 1 {
            return Err(FileUploadChunkJournalError::UnsupportedSchema);
        }
        if row.try_get::<Uuid, _>("authority_id")? != self.scope.authority_id.as_uuid()
            || row.try_get::<Uuid, _>("workspace_id")? != self.scope.workspace_id.as_uuid()
        {
            return Err(FileUploadChunkJournalError::ScopeMismatch);
        }
        if row.try_get::<Uuid, _>("installation_id")? != self.installation_id {
            return Err(FileUploadChunkJournalError::IncarnationMismatch);
        }
        if row.try_get::<i64, _>("namespace_oid")? != namespace.oid
            || row.try_get::<i64, _>("schema_oid")? != relations[0]
            || row.try_get::<i64, _>("chunks_oid")? != relations[1]
        {
            return Err(FileUploadChunkJournalError::UnsupportedSchema);
        }
        let max_entries = u32::try_from(row.try_get::<i32, _>("max_entries")?)
            .map_err(|_| FileUploadChunkJournalError::InvalidRecord)?;
        if !(1..=MAX_FILE_UPLOAD_CHUNK_ENTRIES).contains(&max_entries) {
            return Err(FileUploadChunkJournalError::InvalidRecord);
        }
        if max_entries != self.max_entries {
            return Err(FileUploadChunkJournalError::UnsupportedSchema);
        }
        namespace.verify(self, tx).await?;
        Ok(JournalSchemaPin {
            namespace,
            relations,
            scope: self.scope,
            installation_id: self.installation_id,
            max_entries,
        })
    }
}

async fn relation_shapes(tx: &mut Tx<'_>) -> Result<[i64; 2]> {
    let rows = sqlx::query("WITH expected(relation,key,position) AS (VALUES
        ('collaboration_file_upload_chunk_schema',ARRAY['singleton'],0),
        ('collaboration_file_upload_chunks',ARRAY['upload_id','chunk_index'],1)),
        columns(relation,name,type_name) AS (VALUES
        ('collaboration_file_upload_chunk_schema','singleton','bool'),
        ('collaboration_file_upload_chunk_schema','version','int4'),
        ('collaboration_file_upload_chunk_schema','authority_id','uuid'),
        ('collaboration_file_upload_chunk_schema','workspace_id','uuid'),
        ('collaboration_file_upload_chunk_schema','installation_id','uuid'),
        ('collaboration_file_upload_chunk_schema','namespace_oid','int8'),
        ('collaboration_file_upload_chunk_schema','schema_oid','int8'),
        ('collaboration_file_upload_chunk_schema','chunks_oid','int8'),
        ('collaboration_file_upload_chunk_schema','max_entries','int4'),
        ('collaboration_file_upload_chunks','upload_id','uuid'),
        ('collaboration_file_upload_chunks','chunk_index','int4'),
        ('collaboration_file_upload_chunks','owner_id','uuid'),
        ('collaboration_file_upload_chunks','account_id','uuid'),
        ('collaboration_file_upload_chunks','source_key','bytea'),
        ('collaboration_file_upload_chunks','root_key','bytea'),
        ('collaboration_file_upload_chunks','total_length','int8'),
        ('collaboration_file_upload_chunks','chunk_length','int8'),
        ('collaboration_file_upload_chunks','sha256','text'),
        ('collaboration_file_upload_chunks','nonce','uuid'),
        ('collaboration_file_upload_chunks','confirmed','bool'))
        SELECT c.oid::bigint AS oid,c.relkind::text AS kind,c.relpersistence::text AS persistence,
        c.relrowsecurity AS rls,c.relispartition AS partition,to_regclass(c.relname)::oid=c.oid AS resolved,
        EXISTS(SELECT 1 FROM pg_catalog.pg_inherits i WHERE i.inhrelid=c.oid OR i.inhparent=c.oid) AS inheritance,
        (NOT c.relforcerowsecurity AND
            EXISTS(SELECT 1 FROM pg_catalog.pg_constraint p JOIN pg_catalog.pg_index idx ON idx.indexrelid=p.conindid
                WHERE p.conrelid=c.oid AND p.contype='p' AND p.convalidated AND NOT p.condeferrable
                AND idx.indisvalid AND idx.indisready
                AND e.key=ARRAY(SELECT a.attname::text FROM unnest(p.conkey) WITH ORDINALITY k(n,position)
                    JOIN pg_catalog.pg_attribute a ON a.attrelid=c.oid AND a.attnum=k.n ORDER BY k.position))
            AND (SELECT count(*) FROM pg_catalog.pg_attribute a WHERE a.attrelid=c.oid AND a.attnum>0 AND NOT a.attisdropped)
                =(SELECT count(*) FROM columns x WHERE x.relation=e.relation)
            AND NOT EXISTS(SELECT 1 FROM columns x LEFT JOIN pg_catalog.pg_attribute a
                ON a.attrelid=c.oid AND a.attname=x.name AND a.attnum>0 AND NOT a.attisdropped
                WHERE x.relation=e.relation AND (a.attnum IS NULL OR NOT a.attnotnull OR a.atttypid<>x.type_name::regtype
                    OR a.atttypmod<>-1 OR a.attgenerated<>'' OR a.attidentity<>''))
            AND (e.position<>1 OR EXISTS(SELECT 1 FROM pg_catalog.pg_constraint u
                JOIN pg_catalog.pg_index idx ON idx.indexrelid=u.conindid WHERE u.conrelid=c.oid AND u.contype='u'
                AND u.convalidated AND NOT u.condeferrable AND idx.indisvalid AND idx.indisready
                AND ARRAY(SELECT a.attname::text FROM unnest(u.conkey) WITH ORDINALITY k(n,position)
                    JOIN pg_catalog.pg_attribute a ON a.attrelid=c.oid AND a.attnum=k.n ORDER BY k.position)=ARRAY['nonce']))) AS shape
        FROM expected e JOIN pg_catalog.pg_class c ON c.relname=e.relation
        AND c.relnamespace=current_schema()::regnamespace ORDER BY e.position")
        .fetch_all(&mut **tx).await?;
    if !prepared_resource_sql::ordinary_tables(&rows, 2)? {
        return Err(FileUploadChunkJournalError::UnsupportedSchema);
    }
    for row in &rows {
        if !row.try_get::<bool, _>("shape")? {
            return Err(FileUploadChunkJournalError::UnsupportedSchema);
        }
    }
    Ok([rows[0].try_get("oid")?, rows[1].try_get("oid")?])
}
