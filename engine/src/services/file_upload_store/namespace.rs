use super::*;

pub(super) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 63
        && (name.as_bytes()[0].is_ascii_lowercase() || name.as_bytes()[0] == b'_')
        && name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
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
    pub(super) async fn capture(store: &FileUploadStore, tx: &mut Tx<'_>) -> Result<Self> {
        if !prepared_resource_sql::namespace(tx).await? {
            return Err(FileUploadStoreError::UnsupportedSchema);
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
        if name.as_deref() != Some(store.namespace.as_str())
            || !row.try_get::<bool, _>("no_temp")?
        {
            return Err(FileUploadStoreError::UnsupportedSchema);
        }
        Ok(Self {
            oid: row.try_get("oid")?,
            name: name.ok_or(FileUploadStoreError::UnsupportedSchema)?,
            search_path: row.try_get("search_path")?,
        })
    }

    pub(super) async fn verify(&self, store: &FileUploadStore, tx: &mut Tx<'_>) -> Result<()> {
        if Self::capture(store, tx).await? != *self {
            return Err(FileUploadStoreError::UnsupportedSchema);
        }
        Ok(())
    }

    // Borrowed Session work selects the same restricted identifier with canonical quotes.
    // Compare original physical identity, never reset search_path or permit extra raw entries.
    #[cfg(unix)]
    pub(super) fn matches_selected(&self, current: &Self) -> bool {
        self.name == current.name
            && self.oid == current.oid
            && (current.search_path == current.name
                || current.search_path == format!("\"{}\"", current.name))
    }

    #[cfg(unix)]
    pub(super) async fn verify_selected(
        &self,
        store: &FileUploadStore,
        tx: &mut Tx<'_>,
    ) -> Result<()> {
        if !self.matches_selected(&Self::capture(store, tx).await?) {
            return Err(FileUploadStoreError::UnsupportedSchema);
        }
        Ok(())
    }
}
