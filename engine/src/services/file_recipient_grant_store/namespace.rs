use super::*;

pub(super) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 63
        && (name.as_bytes()[0].is_ascii_lowercase() || name.as_bytes()[0] == b'_')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && !matches!(name, "public" | "information_schema")
        && !name.starts_with("pg_")
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct NamespacePin {
    pub(super) oid: i64,
    name: String,
    search_path: String,
}

impl NamespacePin {
    pub(super) async fn capture(store: &FileRecipientGrantStore, tx: &mut Tx<'_>) -> Result<Self> {
        if !prepared_resource_sql::namespace(tx).await? {
            return Err(FileRecipientGrantStoreError::UnsupportedSchema);
        }
        let row = sqlx::query(
            "SELECT current_schema()::text AS name,
            current_schema()::regnamespace::oid::bigint AS oid,
            pg_catalog.current_setting('search_path') AS search_path,
            pg_my_temp_schema() = 0 AS no_temp",
        )
        .fetch_one(&mut **tx)
        .await?;
        let name: Option<String> = row.try_get("name")?;
        let search_path: String = row.try_get("search_path")?;
        if name.as_deref() != Some(store.namespace.as_str())
            || !row.try_get::<bool, _>("no_temp")?
            // Effective schemas omit nonexistent entries. Admit only one exact raw target,
            // so hidden extra entries cannot become future namespace authority.
            || (search_path != store.namespace
                && search_path != format!("\"{}\"", store.namespace))
        {
            return Err(FileRecipientGrantStoreError::UnsupportedSchema);
        }
        Ok(Self {
            oid: row.try_get("oid")?,
            name: name.ok_or(FileRecipientGrantStoreError::UnsupportedSchema)?,
            search_path,
        })
    }

    pub(super) async fn verify(
        &self,
        store: &FileRecipientGrantStore,
        tx: &mut Tx<'_>,
    ) -> Result<()> {
        if Self::capture(store, tx).await? != *self {
            return Err(FileRecipientGrantStoreError::UnsupportedSchema);
        }
        Ok(())
    }
}
