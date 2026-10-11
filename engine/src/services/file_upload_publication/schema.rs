use super::*;
use namespace::NamespacePin;

/// Original configured publication installation; never silently replaced with a current pin.
#[derive(PartialEq, Eq)]
pub(crate) struct PublicationSchemaPin {
    namespace: NamespacePin,
    relations: [i64; 2],
    scope: WorkspaceRef,
    installation_id: Uuid,
    max_publications: u32,
}

impl PublicationSchemaPin {
    pub(super) fn retain(&self) -> Self {
        Self {
            namespace: self.namespace.clone(),
            relations: self.relations,
            scope: self.scope,
            installation_id: self.installation_id,
            max_publications: self.max_publications,
        }
    }

    pub(super) async fn verify(
        &self,
        store: &FileUploadPublicationStore,
        tx: &mut Tx<'_>,
        writer: bool,
    ) -> Result<()> {
        self.namespace.verify(store, tx).await?;
        if store.check_schema(tx, writer).await? != *self {
            return Err(FileUploadPublicationStoreError::UnsupportedSchema);
        }
        Ok(())
    }
}

impl FileUploadPublicationStore {
    pub(super) async fn begin(&self, readonly: bool) -> Result<Tx<'_>> {
        let mut tx = prepared_resource_sql::begin(&self.pool, readonly).await?;
        if !readonly {
            // Override stale default snapshots before any SELECT, without weakening read-only policy.
            sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
                .execute(&mut *tx)
                .await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'")
                .execute(&mut *tx)
                .await?;
        }
        Ok(tx)
    }

    /// Explicit empty namespace only. No adoption, startup installation, migration or cleanup.
    pub async fn install_empty_namespace(&self) -> Result<()> {
        bounded(async {
            let mut tx = self.begin(false).await?;
            let namespace = NamespacePin::capture(self, &mut tx).await?;
            sqlx::query("SELECT pg_advisory_xact_lock(hashtext(current_database()),
                hashtext('cyanrex.file-publications.' || current_schema()))").execute(&mut *tx).await?;
            namespace.verify(self, &mut tx).await?;
            let occupied: bool = sqlx::query("SELECT EXISTS(SELECT 1 FROM pg_class WHERE relnamespace=current_schema()::regnamespace)
                OR EXISTS(SELECT 1 FROM pg_proc WHERE pronamespace=current_schema()::regnamespace)
                OR EXISTS(SELECT 1 FROM pg_type WHERE typnamespace=current_schema()::regnamespace) AS occupied")
                .fetch_one(&mut *tx).await?.try_get("occupied")?;
            if occupied { return Err(FileUploadPublicationStoreError::NamespaceNotEmpty); }
            for statement in include_str!("../../../migrations/0020_collaboration_file_publications.sql")
                .split(';').map(str::trim).filter(|statement| !statement.is_empty()) {
                sqlx::query(statement).execute(&mut *tx).await?;
            }
            namespace.verify(self, &mut tx).await?;
            let relations = relation_shapes(&mut tx).await?;
            confirmed_one(sqlx::query("INSERT INTO collaboration_file_publication_schema
                (singleton,version,authority_id,workspace_id,installation_id,namespace_oid,schema_oid,publications_oid,max_publications)
                VALUES(TRUE,1,$1,$2,$3,$4,$5,$6,$7)")
                .bind(self.scope.authority_id.as_uuid()).bind(self.scope.workspace_id.as_uuid()).bind(self.installation_id)
                .bind(namespace.oid).bind(relations[0]).bind(relations[1]).bind(self.max_publications as i32)
                .execute(&mut *tx).await?.rows_affected())?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'").execute(&mut *tx).await?;
            sqlx::query("SET CONSTRAINTS ALL IMMEDIATE").execute(&mut *tx).await?;
            namespace.verify(self, &mut tx).await?;
            let installed = self.check_schema(&mut tx, true).await?;
            if installed.namespace != namespace || installed.relations != relations {
                return Err(FileUploadPublicationStoreError::UnsupportedSchema);
            }
            if self.ledger(&mut tx).await?.len() != 0 { return Err(FileUploadPublicationStoreError::InvalidRecord); }
            installed.verify(self, &mut tx, true).await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'").execute(&mut *tx).await?;
            tx.commit().await?;
            Ok(())
        }).await
    }

    pub(crate) async fn capture_pin(&self) -> Result<PublicationSchemaPin> {
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
    ) -> Result<PublicationSchemaPin> {
        let namespace = NamespacePin::capture(self, tx).await?;
        let lock = if writer {
            "LOCK TABLE collaboration_file_publication_schema, collaboration_file_publications IN SHARE ROW EXCLUSIVE MODE"
        } else {
            "LOCK TABLE collaboration_file_publication_schema, collaboration_file_publications IN ACCESS SHARE MODE"
        };
        sqlx::query(lock).execute(&mut **tx).await?;
        namespace.verify(self, tx).await?;
        let relations = relation_shapes(tx).await?;
        let query = if writer {
            "SELECT singleton,version,authority_id,workspace_id,installation_id,namespace_oid,schema_oid,publications_oid,max_publications
            FROM collaboration_file_publication_schema LIMIT 2 FOR UPDATE"
        } else {
            "SELECT singleton,version,authority_id,workspace_id,installation_id,namespace_oid,schema_oid,publications_oid,max_publications
            FROM collaboration_file_publication_schema LIMIT 2"
        };
        let rows = sqlx::query(query).fetch_all(&mut **tx).await?;
        if rows.len() != 1 {
            return Err(FileUploadPublicationStoreError::UnsupportedSchema);
        }
        let row = &rows[0];
        if !row.try_get::<bool, _>("singleton")? || row.try_get::<i32, _>("version")? != 1 {
            return Err(FileUploadPublicationStoreError::UnsupportedSchema);
        }
        if row.try_get::<Uuid, _>("authority_id")? != self.scope.authority_id.as_uuid()
            || row.try_get::<Uuid, _>("workspace_id")? != self.scope.workspace_id.as_uuid()
        {
            return Err(FileUploadPublicationStoreError::ScopeMismatch);
        }
        if row.try_get::<Uuid, _>("installation_id")? != self.installation_id {
            return Err(FileUploadPublicationStoreError::IncarnationMismatch);
        }
        if row.try_get::<i64, _>("namespace_oid")? != namespace.oid
            || row.try_get::<i64, _>("schema_oid")? != relations[0]
            || row.try_get::<i64, _>("publications_oid")? != relations[1]
        {
            return Err(FileUploadPublicationStoreError::UnsupportedSchema);
        }
        let max_publications = u32::try_from(row.try_get::<i32, _>("max_publications")?)
            .map_err(|_| FileUploadPublicationStoreError::InvalidRecord)?;
        if !(1..=MAX_FILE_UPLOAD_PUBLICATIONS).contains(&max_publications) {
            return Err(FileUploadPublicationStoreError::InvalidRecord);
        }
        if max_publications != self.max_publications {
            return Err(FileUploadPublicationStoreError::UnsupportedSchema);
        }
        namespace.verify(self, tx).await?;
        Ok(PublicationSchemaPin {
            namespace,
            relations,
            scope: self.scope,
            installation_id: self.installation_id,
            max_publications,
        })
    }
}

async fn relation_shapes(tx: &mut Tx<'_>) -> Result<[i64; 2]> {
    let rows = sqlx::query("WITH expected(relation,key,position) AS (VALUES
        ('collaboration_file_publication_schema',ARRAY['singleton'],0),
        ('collaboration_file_publications',ARRAY['upload_id'],1)),
        columns(relation,name,type_name) AS (VALUES
        ('collaboration_file_publication_schema','singleton','bool'),
        ('collaboration_file_publication_schema','version','int4'),
        ('collaboration_file_publication_schema','authority_id','uuid'),
        ('collaboration_file_publication_schema','workspace_id','uuid'),
        ('collaboration_file_publication_schema','installation_id','uuid'),
        ('collaboration_file_publication_schema','namespace_oid','int8'),
        ('collaboration_file_publication_schema','schema_oid','int8'),
        ('collaboration_file_publication_schema','publications_oid','int8'),
        ('collaboration_file_publication_schema','max_publications','int4'),
        ('collaboration_file_publications','upload_id','uuid'),
        ('collaboration_file_publications','file_id','uuid'),
        ('collaboration_file_publications','version_id','uuid'),
        ('collaboration_file_publications','owner_id','uuid'),
        ('collaboration_file_publications','account_id','uuid'),
        ('collaboration_file_publications','manifest','bytea'),
        ('collaboration_file_publications','source_key','bytea'),
        ('collaboration_file_publications','root_key','bytea'),
        ('collaboration_file_publications','journal_key','bytea'),
        ('collaboration_file_publications','nonce','uuid'))
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
            AND (e.position<>1 OR (EXISTS(SELECT 1 FROM pg_catalog.pg_constraint u
                JOIN pg_catalog.pg_index idx ON idx.indexrelid=u.conindid WHERE u.conrelid=c.oid AND u.contype='u'
                AND u.convalidated AND NOT u.condeferrable AND idx.indisvalid AND idx.indisready
                AND ARRAY(SELECT a.attname::text FROM unnest(u.conkey) WITH ORDINALITY k(n,position)
                    JOIN pg_catalog.pg_attribute a ON a.attrelid=c.oid AND a.attnum=k.n ORDER BY k.position)=ARRAY['nonce'])
                AND EXISTS(SELECT 1 FROM pg_catalog.pg_constraint u JOIN pg_catalog.pg_index idx ON idx.indexrelid=u.conindid
                    WHERE u.conrelid=c.oid AND u.contype='u' AND u.convalidated AND NOT u.condeferrable
                    AND idx.indisvalid AND idx.indisready
                    AND ARRAY(SELECT a.attname::text FROM unnest(u.conkey) WITH ORDINALITY k(n,position)
                        JOIN pg_catalog.pg_attribute a ON a.attrelid=c.oid AND a.attnum=k.n ORDER BY k.position)=ARRAY['file_id','version_id'])))) AS shape
        FROM expected e JOIN pg_catalog.pg_class c ON c.relname=e.relation
        AND c.relnamespace=current_schema()::regnamespace ORDER BY e.position").fetch_all(&mut **tx).await?;
    if !prepared_resource_sql::ordinary_tables(&rows, 2)? {
        return Err(FileUploadPublicationStoreError::UnsupportedSchema);
    }
    for row in &rows {
        if !row.try_get::<bool, _>("shape")? {
            return Err(FileUploadPublicationStoreError::UnsupportedSchema);
        }
    }
    Ok([rows[0].try_get("oid")?, rows[1].try_get("oid")?])
}
