//! A source-only pin for Session APIs. No registry, installer, namespace selection or fallback.
use super::{namespace::NamespacePin, source_relations::RelationPin, *};

pub(super) struct SessionSourcePin {
    namespace: NamespacePin,
    relations: RelationPin,
    search_path: String,
}
impl SessionSourcePin {
    pub async fn capture(
        source: &DurableAuthSource,
        tx: &mut Transaction<'_, Postgres>,
        writer: bool,
    ) -> Result<Self> {
        let namespace = NamespacePin::capture(tx).await?;
        let search_path = Self::search_path(tx).await?;
        let relations = RelationPin::capture(tx, false).await?;
        let pin = Self {
            namespace,
            relations,
            search_path,
        };
        pin.verify(source, tx, writer).await?;
        Ok(pin)
    }

    async fn search_path(tx: &mut Transaction<'_, Postgres>) -> Result<String> {
        Ok(
            sqlx::query("SELECT pg_catalog.current_setting('search_path') AS path")
                .fetch_one(&mut **tx)
                .await?
                .try_get("path")?,
        )
    }

    async fn verify_namespace(&self, tx: &mut Transaction<'_, Postgres>) -> Result<()> {
        self.namespace.verify(tx).await?;
        if Self::search_path(tx).await? != self.search_path {
            return Err(DurableAuthError::SourceMismatch);
        }
        Ok(())
    }

    /// Verify the first-observed pin, including across login's two transactions. Do not capture
    /// a replacement source, reset search_path, or make an unqualified post-write absence read
    /// before this check. Call the final fresh Session check AFTER all guard/metadata waits.
    pub async fn verify(
        &self,
        source: &DurableAuthSource,
        tx: &mut Transaction<'_, Postgres>,
        writer: bool,
    ) -> Result<()> {
        self.verify_namespace(tx).await?;
        self.relations.verify(tx).await?;
        source.lock_source(tx, writer).await?;
        DurableAuthSource::verify_schema(tx).await?;
        self.verify_namespace(tx).await
    }
}
