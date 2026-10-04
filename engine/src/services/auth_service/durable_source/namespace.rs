//! Internal namespace identity shared by private work and source-only Session operations.
use super::*;

pub(super) enum NamespaceError {
    InvalidNamespace,
    StorageUnavailable,
}
impl From<sqlx::Error> for NamespaceError {
    fn from(_: sqlx::Error) -> Self {
        Self::StorageUnavailable
    }
}
impl From<NamespaceError> for DurableAuthError {
    fn from(error: NamespaceError) -> Self {
        match error {
            NamespaceError::InvalidNamespace => Self::SourceMismatch,
            NamespaceError::StorageUnavailable => Self::StorageUnavailable,
        }
    }
}
type NamespaceResult<T> = std::result::Result<T, NamespaceError>;

pub(super) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 63
        && name.as_bytes()[0].is_ascii_lowercase()
        && name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
        && !matches!(name, "public" | "information_schema")
        && !name.starts_with("pg_")
}

#[derive(PartialEq, Eq)]
pub(super) struct NamespacePin {
    pub name: String,
    oid: i64,
}
impl NamespacePin {
    pub fn has_same_oid(&self, other: &Self) -> bool {
        self.oid == other.oid
    }

    pub async fn capture(tx: &mut Transaction<'_, Postgres>) -> NamespaceResult<Self> {
        let row = sqlx::query(
            "SELECT current_schema()::text AS name,
            current_schema()::regnamespace::oid::bigint AS oid,
            cardinality(current_schemas(false)) AS count, pg_my_temp_schema() = 0 AS no_temp",
        )
        .fetch_one(&mut **tx)
        .await?;
        let name: Option<String> = row.try_get("name")?;
        let name = name.ok_or(NamespaceError::InvalidNamespace)?;
        if !valid_name(&name)
            || row.try_get::<i32, _>("count")? != 1
            || !row.try_get::<bool, _>("no_temp")?
        {
            return Err(NamespaceError::InvalidNamespace);
        }
        Ok(Self {
            name,
            oid: row.try_get("oid")?,
        })
    }
    pub async fn select(tx: &mut Transaction<'_, Postgres>, name: &str) -> NamespaceResult<()> {
        if !valid_name(name) {
            return Err(NamespaceError::InvalidNamespace);
        }
        // Private work deliberately selects targets. Source-only Session guards NEVER call this:
        // resetting search_path would erase evidence of trigger-induced source drift.
        sqlx::query("SELECT pg_catalog.set_config('search_path', $1, true)")
            .bind(format!("\"{name}\""))
            .execute(&mut **tx)
            .await?;
        Ok(())
    }
    pub async fn verify(&self, tx: &mut Transaction<'_, Postgres>) -> NamespaceResult<()> {
        if Self::capture(tx).await? != *self {
            return Err(NamespaceError::InvalidNamespace);
        }
        Ok(())
    }
}
