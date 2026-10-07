//! Explicit bounded maintenance of expired Sessions, never a read-path side effect.
use super::session_source::SessionSourcePin;
use super::*;

const BATCH_SIZE: i64 = 128;
// Bound text/time before transfer/decoding without hiding corrupt candidates behind a WHERE filter.
// No passwords, OTP state or raw usable tokens are loaded by this maintenance operation.
// SQLx's binary chrono decoder can panic on infinity or PostgreSQL's larger finite upper range.
// Finite PostgreSQL's lower range fits chrono; $3 binds chrono's maximum on BOTH query paths.
const PROJECTION: &str = "CASE WHEN octet_length(s.token) = 64 THEN s.token END AS token,
    CASE WHEN octet_length(s.username) <= 64 THEN s.username END AS username,
    s.account_id,
    CASE WHEN isfinite(s.expires_at) AND s.expires_at <= $3 THEN s.expires_at END AS expires_at,
    CASE WHEN isfinite(s.created_at) AND s.created_at <= $3 THEN s.created_at END AS created_at,
    EXISTS (SELECT 1 FROM users u WHERE u.username = s.username AND u.account_id = s.account_id) AS account_present";

// Private full-row snapshot, intentionally not Debug/Serialize or a public cleanup receipt.
#[derive(PartialEq, Eq)]
struct ExpiredSession {
    token: String,
    username: LegacyUsername,
    account_id: LegacyAccountId,
    expires_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

fn decode(row: sqlx_postgres::PgRow, cutoff: DateTime<Utc>) -> Result<ExpiredSession> {
    let token = row
        .try_get::<Option<String>, _>("token")?
        .ok_or(DurableAuthError::InvalidRecord)?;
    let username = row
        .try_get::<Option<String>, _>("username")?
        .ok_or(DurableAuthError::InvalidRecord)?
        .parse()?;
    let account_id = LegacyAccountId::try_from(row.try_get::<Uuid, _>("account_id")?)?;
    let expires_at = row
        .try_get::<Option<DateTime<Utc>>, _>("expires_at")
        .map_err(|_| DurableAuthError::InvalidRecord)?
        .ok_or(DurableAuthError::InvalidRecord)?;
    let created_at = row
        .try_get::<Option<DateTime<Utc>>, _>("created_at")
        .map_err(|_| DurableAuthError::InvalidRecord)?
        .ok_or(DurableAuthError::InvalidRecord)?;
    if token.len() != 64
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || expires_at > cutoff
        || !row.try_get::<bool, _>("account_present")?
    {
        return Err(DurableAuthError::InvalidRecord);
    }
    Ok(ExpiredSession {
        token,
        username,
        account_id,
        expires_at,
        created_at,
    })
}

impl DurableAuthSource {
    /// Trusted maintenance only; no live AuthService, HTTP or automatic scheduled cleanup.
    /// Delete at most 128 Sessions expired at one fresh database cutoff, under the source fence.
    /// Only a confirmed commit returns the removed count; zero is not a global emptiness proof.
    /// Cancellation, timeout or a lost reply does not prove rollback or authorize blind replay.
    pub async fn prune_expired_sessions(&self) -> Result<u32> {
        bounded(async {
            let mut tx = self.transaction().await?;
            let pin = SessionSourcePin::capture(self, &mut tx, true).await?;
            // Take time AFTER source-lock waits. Selection and deletion share this cutoff,
            // rather than extending the eligible set while waiting or processing the batch.
            let cutoff = Self::now(&mut tx).await?;
            let rows = sqlx::query(&format!(
                "SELECT {PROJECTION} FROM sessions AS s
                WHERE s.expires_at <= $1 ORDER BY s.expires_at, s.token LIMIT $2 FOR UPDATE OF s"
            ))
            .bind(cutoff)
            .bind(BATCH_SIZE)
            .bind(DateTime::<Utc>::MAX_UTC)
            .fetch_all(&mut *tx)
            .await?;
            // Validate the entire selected batch before ANY deletion. Corruption is not trash.
            let mut expected = rows
                .into_iter()
                .map(|row| decode(row, cutoff))
                .collect::<Result<Vec<_>>>()?;
            expected.sort_unstable_by(|left, right| left.token.cmp(&right.token));
            let tokens: Vec<_> = expected.iter().map(|row| row.token.clone()).collect();
            if !tokens.is_empty() {
                let removed = sqlx::query(&format!(
                    "DELETE FROM sessions AS s
                    WHERE s.token = ANY($1) AND s.expires_at <= $2 RETURNING {PROJECTION}"
                ))
                .bind(&tokens)
                .bind(cutoff)
                .bind(DateTime::<Utc>::MAX_UTC)
                .fetch_all(&mut *tx)
                .await?;
                pin.verify(self, &mut tx, true).await?;
                let mut actual = removed
                    .into_iter()
                    .map(|row| decode(row, cutoff))
                    .collect::<Result<Vec<_>>>()?;
                actual.sort_unstable_by(|left, right| left.token.cmp(&right.token));
                if actual != expected {
                    return Err(DurableAuthError::StorageUnavailable);
                }
                // Check every selected digest, including reinsertion with a future expiry.
                // The original source pin must pass before this unqualified absence read.
                let remains: bool = sqlx::query(
                    "SELECT EXISTS (SELECT 1 FROM sessions WHERE token = ANY($1)) AS remains",
                )
                .bind(&tokens)
                .fetch_one(&mut *tx)
                .await?
                .try_get("remains")?;
                if remains {
                    return Err(DurableAuthError::StorageUnavailable);
                }
            }
            // An empty batch still requires a valid original source and confirmed read commit.
            pin.verify(self, &mut tx, true).await?;
            tx.commit().await?;
            Ok(expected.len() as u32)
        })
        .await
    }
}

#[cfg(test)]
#[path = "session_cleanup_sql_tests.rs"]
mod sql_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn closed_source_cleanup_cannot_confirm_an_empty_batch() {
        let pool = sqlx::PgPoolOptions::new()
            .connect_lazy("postgres://synthetic@127.0.0.1/unused")
            .unwrap();
        pool.close().await;
        let source = DurableAuthSource::new(pool, AuthorityId::try_from(Uuid::new_v4()).unwrap());
        assert_eq!(
            source.prune_expired_sessions().await,
            Err(DurableAuthError::StorageUnavailable)
        );
    }
}
