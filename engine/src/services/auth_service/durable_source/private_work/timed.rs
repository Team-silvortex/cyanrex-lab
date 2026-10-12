//! Original private-work pins across a lock-free external-work gap and a final time window.
//! No file types, permissions, byte IO, independent transaction or reusable authorization token.
use super::*;

impl PrivateWorkContext {
    /// Caller starts a fresh source transaction. Recheck the first source/Registry identities and
    /// original exact Session before entering the original selected target; never recapture them.
    pub async fn resume(
        &self,
        source: &DurableAuthSource,
        tx: &mut Transaction<'_, Postgres>,
    ) -> WorkResult<()> {
        self.source.verify(tx).await?;
        self.relations.verify(tx).await?;
        source.lock_source(tx, false).await?;
        DurableAuthSource::verify_schema(tx).await?;
        Registry::lock_session_command_scope(tx, self.scope).await?;
        let owner = Registry::session_private_work_actor(
            tx,
            self.scope,
            &self.session.account.username,
            self.session.account.account_id,
        )
        .await?;
        if owner != self.owner {
            return Err(SessionCommandError::InvalidSession.into());
        }
        source
            .recheck_command_session(tx, &self.digest, &self.session)
            .await?;
        NamespacePin::select(tx, &self.targets[self.selected].name).await?;
        self.targets[self.selected].verify(tx).await?;
        Ok(())
    }

    /// All original target/source/Registry and exact Session checks precede one final fresh clock
    /// sample covering BOTH the extra window and original Session expiry. No later SQL before COMMIT.
    pub async fn finish_in_window(
        self,
        source: &DurableAuthSource,
        tx: &mut Transaction<'_, Postgres>,
        not_before: u64,
        expires_at: u64,
    ) -> WorkResult<bool> {
        self.finish_retained_in_window(source, tx, not_before, expires_at)
            .await
    }

    /// Same final sample, retaining the original identities for reentry after private file work.
    /// This initial observation is not permission to disclose later bytes without final reentry.
    pub(in super::super) async fn finish_retained_in_window(
        &self,
        source: &DurableAuthSource,
        tx: &mut Transaction<'_, Postgres>,
        not_before: u64,
        expires_at: u64,
    ) -> WorkResult<bool> {
        self.finish_retained(source, tx).await?;
        let row = sqlx::query(
            "WITH observed AS MATERIALIZED (SELECT pg_catalog.clock_timestamp() AS now)
            SELECT now < $1 AS session_valid,
                extract(epoch FROM now) >= $2::bigint AND extract(epoch FROM now) < $3::bigint
                AS window_valid FROM observed",
        )
        .bind(self.session.expires_at)
        .bind(i64::try_from(not_before).map_err(|_| DurableAuthError::InvalidInput)?)
        .bind(i64::try_from(expires_at).map_err(|_| DurableAuthError::InvalidInput)?)
        .fetch_one(&mut **tx)
        .await?;
        if !row.try_get::<bool, _>("session_valid")? {
            return Err(SessionCommandError::InvalidSession.into());
        }
        Ok(row.try_get("window_valid")?)
    }
}
