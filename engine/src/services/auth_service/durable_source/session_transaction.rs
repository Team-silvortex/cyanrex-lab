//! Internal two-phase Session work. Neither preparation nor finish confirms COMMIT.
//! Callers own the transaction and insert any retained-storage guards BEFORE finish.
use super::{session_source::SessionSourcePin, *};

pub(crate) struct PreparedSessionRead {
    authority_id: AuthorityId,
    pin: SessionSourcePin,
    digest: String,
}

impl PreparedSessionRead {
    pub(crate) async fn prepare(
        authority_id: AuthorityId,
        tx: &mut Transaction<'_, Postgres>,
        token: &str,
    ) -> Result<Self> {
        if !valid_token(token) {
            return Err(DurableAuthError::InvalidInput);
        }
        let pin = SessionSourcePin::capture_for(authority_id, tx, false).await?;
        let digest = hash_session_token(token);
        // Lock the account and exact Session before final guards; discard this early snapshot.
        record(authority_id, tx, &digest).await?;
        Ok(Self {
            authority_id,
            pin,
            digest,
        })
    }

    pub(crate) async fn finish(
        self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Option<DurableSession>> {
        self.pin.verify_for(self.authority_id, tx, false).await?;
        // All guard waits precede the final fresh expiry read, including platform catalog work.
        record(self.authority_id, tx, &self.digest).await
    }
}

pub(crate) struct PreparedSessionLogout {
    authority_id: AuthorityId,
    pin: SessionSourcePin,
    digest: String,
}

impl PreparedSessionLogout {
    pub(crate) async fn prepare(
        authority_id: AuthorityId,
        tx: &mut Transaction<'_, Postgres>,
        token: &str,
    ) -> Result<Self> {
        if !valid_token(token) {
            return Err(DurableAuthError::InvalidInput);
        }
        let pin = SessionSourcePin::capture_for(authority_id, tx, true).await?;
        let digest = hash_session_token(token);
        let deleted = sqlx::query("DELETE FROM sessions WHERE token = $1")
            .bind(&digest)
            .execute(&mut **tx)
            .await?;
        if deleted.rows_affected() > 1 {
            return Err(DurableAuthError::StorageUnavailable);
        }
        Ok(Self {
            authority_id,
            pin,
            digest,
        })
    }

    pub(crate) async fn finish(self, tx: &mut Transaction<'_, Postgres>) -> Result<()> {
        // Prove unqualified absence is still read from the original source after DELETE triggers.
        self.pin.verify_for(self.authority_id, tx, true).await?;
        let remaining = sqlx::query("SELECT token FROM sessions WHERE token = $1 FOR SHARE")
            .bind(&self.digest)
            .fetch_optional(&mut **tx)
            .await?;
        if remaining.is_some() {
            return Err(DurableAuthError::StorageUnavailable);
        }
        Ok(())
    }
}

pub(super) async fn record(
    authority_id: AuthorityId,
    connection: &mut PgConnection,
    digest: &str,
) -> Result<Option<DurableSession>> {
    // Guard SQLx's binary chrono decoder from PostgreSQL infinity and its wider finite range.
    let row = sqlx::query("SELECT s.username, s.account_id,
        CASE WHEN isfinite(s.expires_at) AND s.expires_at <= $2 THEN s.expires_at END AS expires_at FROM sessions s
        JOIN users u ON u.username = s.username AND u.account_id = s.account_id WHERE s.token = $1 FOR SHARE OF s, u")
        .bind(digest).bind(DateTime::<Utc>::MAX_UTC).fetch_optional(&mut *connection).await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let username = row.try_get::<String, _>("username")?.parse()?;
    let account_id = LegacyAccountId::try_from(row.try_get::<Uuid, _>("account_id")?)?;
    let expires_at = row
        .try_get::<Option<DateTime<Utc>>, _>("expires_at")
        .map_err(|_| DurableAuthError::InvalidRecord)?
        .ok_or(DurableAuthError::InvalidRecord)?;
    if expires_at <= DurableAuthSource::now(connection).await? {
        return Ok(None);
    }
    Ok(Some(DurableSession {
        account: DurableAccountRef {
            authority_id,
            username,
            account_id,
        },
        expires_at,
    }))
}
