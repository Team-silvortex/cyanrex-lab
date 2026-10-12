use super::*;
use namespace::NamespacePin;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct SchemaPin {
    namespace: NamespacePin,
    relations: [i64; 3],
    scope: WorkspaceRef,
    installation_id: Uuid,
    max_grants: u32,
}

impl SchemaPin {
    pub(super) async fn verify(
        &self,
        store: &FilePublisherGrantStore,
        tx: &mut Tx<'_>,
        writer: bool,
    ) -> Result<()> {
        self.namespace.verify(store, tx).await?;
        if store.check_schema(tx, writer).await? != *self {
            return Err(FilePublisherGrantStoreError::UnsupportedSchema);
        }
        Ok(())
    }
}

impl FilePublisherGrantStore {
    pub(super) async fn begin(&self, readonly: bool) -> Result<Tx<'_>> {
        let mut tx = prepared_resource_sql::begin(&self.pool, readonly).await?;
        if !readonly {
            // Fresh reads after quota/marker waits; never override configured READ ONLY policy.
            sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
                .execute(&mut *tx)
                .await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'")
                .execute(&mut *tx)
                .await?;
        }
        Ok(tx)
    }

    /// Explicit fresh namespace only: no partial-schema repair, adoption or automatic installation.
    pub async fn install_empty_namespace(&self) -> Result<()> {
        bounded(async {
            let mut tx = self.begin(false).await?;
            let namespace = NamespacePin::capture(self, &mut tx).await?;
            sqlx::query("SELECT pg_advisory_xact_lock(hashtext(current_database()),
                hashtext('cyanrex.file-publisher-grants.' || current_schema()))")
                .execute(&mut *tx).await?;
            namespace.verify(self, &mut tx).await?;
            let occupied: bool = sqlx::query("SELECT EXISTS(SELECT 1 FROM pg_class WHERE relnamespace=current_schema()::regnamespace)
                OR EXISTS(SELECT 1 FROM pg_proc WHERE pronamespace=current_schema()::regnamespace)
                OR EXISTS(SELECT 1 FROM pg_type WHERE typnamespace=current_schema()::regnamespace) AS occupied")
                .fetch_one(&mut *tx).await?.try_get("occupied")?;
            if occupied { return Err(FilePublisherGrantStoreError::NamespaceNotEmpty); }
            for statement in include_str!("../../../migrations/0021_collaboration_file_publisher_grants.sql")
                .split(';').map(str::trim).filter(|statement| !statement.is_empty()) {
                sqlx::query(statement).execute(&mut *tx).await?;
            }
            namespace.verify(self, &mut tx).await?;
            let relations = shape::relations(&mut tx).await?;
            confirmed_one(sqlx::query("INSERT INTO collaboration_file_publisher_grant_schema
                (singleton,version,authority_id,workspace_id,installation_id,namespace_oid,schema_oid,grants_oid,changes_oid,max_grants)
                VALUES(TRUE,1,$1,$2,$3,$4,$5,$6,$7,$8)")
                .bind(self.scope.authority_id.as_uuid()).bind(self.scope.workspace_id.as_uuid())
                .bind(self.installation_id).bind(namespace.oid).bind(relations[0]).bind(relations[1])
                .bind(relations[2]).bind(self.max_grants as i32).execute(&mut *tx).await?.rows_affected())?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'").execute(&mut *tx).await?;
            sqlx::query("SET CONSTRAINTS ALL IMMEDIATE").execute(&mut *tx).await?;
            namespace.verify(self, &mut tx).await?;
            let installed = self.check_schema(&mut tx, true).await?;
            if installed.namespace != namespace || installed.relations != relations {
                return Err(FilePublisherGrantStoreError::UnsupportedSchema);
            }
            if self.ledger(&mut tx).await?.len() != 0 {
                return Err(FilePublisherGrantStoreError::InvalidRecord);
            }
            installed.verify(self, &mut tx, true).await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'").execute(&mut *tx).await?;
            tx.commit().await?;
            Ok(())
        }).await
    }

    pub(super) async fn check_schema(&self, tx: &mut Tx<'_>, writer: bool) -> Result<SchemaPin> {
        let namespace = NamespacePin::capture(self, tx).await?;
        let lock = if writer {
            "LOCK TABLE collaboration_file_publisher_grant_schema, collaboration_file_publisher_grants,
                collaboration_file_publisher_grant_changes IN SHARE ROW EXCLUSIVE MODE"
        } else {
            "LOCK TABLE collaboration_file_publisher_grant_schema, collaboration_file_publisher_grants,
                collaboration_file_publisher_grant_changes IN ACCESS SHARE MODE"
        };
        sqlx::query(lock).execute(&mut **tx).await?;
        namespace.verify(self, tx).await?;
        let relations = shape::relations(tx).await?;
        let query = if writer {
            "SELECT singleton,version,authority_id,workspace_id,installation_id,namespace_oid,
            schema_oid,grants_oid,changes_oid,max_grants FROM collaboration_file_publisher_grant_schema LIMIT 2 FOR UPDATE"
        } else {
            "SELECT singleton,version,authority_id,workspace_id,installation_id,namespace_oid,
            schema_oid,grants_oid,changes_oid,max_grants FROM collaboration_file_publisher_grant_schema LIMIT 2"
        };
        let rows = sqlx::query(query).fetch_all(&mut **tx).await?;
        if rows.len() != 1 {
            return Err(FilePublisherGrantStoreError::UnsupportedSchema);
        }
        let row = &rows[0];
        if !row.try_get::<bool, _>("singleton")? || row.try_get::<i32, _>("version")? != 1 {
            return Err(FilePublisherGrantStoreError::UnsupportedSchema);
        }
        if row.try_get::<Uuid, _>("authority_id")? != self.scope.authority_id.as_uuid()
            || row.try_get::<Uuid, _>("workspace_id")? != self.scope.workspace_id.as_uuid()
        {
            return Err(FilePublisherGrantStoreError::ScopeMismatch);
        }
        if row.try_get::<Uuid, _>("installation_id")? != self.installation_id {
            return Err(FilePublisherGrantStoreError::IncarnationMismatch);
        }
        if row.try_get::<i64, _>("namespace_oid")? != namespace.oid
            || row.try_get::<i64, _>("schema_oid")? != relations[0]
            || row.try_get::<i64, _>("grants_oid")? != relations[1]
            || row.try_get::<i64, _>("changes_oid")? != relations[2]
        {
            return Err(FilePublisherGrantStoreError::UnsupportedSchema);
        }
        let max_grants = u32::try_from(row.try_get::<i32, _>("max_grants")?)
            .map_err(|_| FilePublisherGrantStoreError::InvalidRecord)?;
        if !(1..=MAX_FILE_PUBLISHER_GRANTS).contains(&max_grants) {
            return Err(FilePublisherGrantStoreError::InvalidRecord);
        }
        if max_grants != self.max_grants {
            return Err(FilePublisherGrantStoreError::UnsupportedSchema);
        }
        namespace.verify(self, tx).await?;
        Ok(SchemaPin {
            namespace,
            relations,
            scope: self.scope,
            installation_id: self.installation_id,
            max_grants,
        })
    }
}
