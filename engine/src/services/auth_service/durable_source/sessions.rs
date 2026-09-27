use super::*;

impl DurableAuthSource {
    pub async fn login(
        &self,
        username: &LegacyUsername,
        password: &str,
        otp: &str,
    ) -> Result<DurableLogin> {
        if password.len() > 4096 || otp.len() > 64 {
            return Err(DurableAuthError::InvalidInput);
        }
        self.admit_login(username)?;
        let result = bounded(async {
            let mut tx = self.transaction().await?;
            self.lock_source(&mut tx, false).await?;
            let verified = self.account_record(&mut tx, username).await?.ok_or(DurableAuthError::InvalidCredentials)?;
            tx.commit().await?;
            // Do not keep a database lock/connection across expensive password verification.
            if !verify_password_async(password, &verified.user.password_salt, &verified.user.password_hash).await
                || !verify_totp(&verified.user.totp_secret, otp) { return Err(DurableAuthError::InvalidCredentials); }
            let mut tx = self.transaction().await?;
            self.lock_source(&mut tx, true).await?;
            let current = self.account_record(&mut tx, username).await?.ok_or(DurableAuthError::InvalidCredentials)?;
            if current.account != verified.account || !same_credentials(&current.user, &verified.user) {
                return Err(DurableAuthError::InvalidCredentials);
            }
            let token = Uuid::new_v4().to_string();
            let digest = hash_session_token(&token);
            let expires_at = Self::now(&mut tx).await? + chrono::Duration::hours(12);
            confirmed_one(sqlx::query("INSERT INTO sessions (token, username, account_id, expires_at) VALUES ($1, $2, $3, $4)")
                .bind(&digest).bind(username.as_str()).bind(verified.account.account_id.as_uuid()).bind(expires_at)
                .execute(&mut *tx).await?.rows_affected())?;
            // INSERT triggers can change credentials/identity in our own transaction. Verify both
            // the exact source generation and complete session row again before COMMIT/publication.
            let current = self.account_record(&mut tx, username).await?.ok_or(DurableAuthError::InvalidCredentials)?;
            if current.account != verified.account || !same_credentials(&current.user, &verified.user) {
                return Err(DurableAuthError::InvalidCredentials);
            }
            let expected = DurableSession { account: verified.account, expires_at };
            if self.session_record(&mut tx, &digest).await?.as_ref() != Some(&expected) { return Err(DurableAuthError::StorageUnavailable); }
            tx.commit().await?;
            Ok(DurableLogin { token, session: expected })
        }).await;
        if result.is_ok() {
            self.clear_login_attempts(username);
        }
        result
    }

    pub(super) async fn session_record(
        &self,
        connection: &mut PgConnection,
        digest: &str,
    ) -> Result<Option<DurableSession>> {
        let row = sqlx::query("SELECT s.username, s.account_id, s.expires_at FROM sessions s
            JOIN users u ON u.username = s.username AND u.account_id = s.account_id WHERE s.token = $1 FOR SHARE OF s, u")
            .bind(digest).fetch_optional(&mut *connection).await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let username = row.try_get::<String, _>("username")?.parse()?;
        let account_id = LegacyAccountId::try_from(row.try_get::<Uuid, _>("account_id")?)?;
        let expires_at: DateTime<Utc> = row.try_get("expires_at")?;
        if expires_at <= Self::now(connection).await? {
            return Ok(None);
        }
        Ok(Some(DurableSession {
            account: DurableAccountRef {
                authority_id: self.authority_id,
                username,
                account_id,
            },
            expires_at,
        }))
    }

    /// Fresh, read-only authentication snapshot; does not authorize a later policy/identity write.
    /// No DDL, cache, memory fallback, expiry deletion or Principal allocation is performed.
    pub async fn validate_session(&self, token: &str) -> Result<Option<DurableSession>> {
        if !valid_token(token) {
            return Ok(None);
        }
        bounded(async {
            let mut tx = self.transaction().await?;
            self.lock_source(&mut tx, false).await?;
            let session = self
                .session_record(&mut tx, &hash_session_token(token))
                .await?;
            tx.commit().await?;
            Ok(session)
        })
        .await
    }

    /// Delete this exact session, not every device, an account or running kernel resources.
    pub async fn logout(&self, token: &str) -> Result<()> {
        if !valid_token(token) {
            return Err(DurableAuthError::InvalidInput);
        }
        bounded(async {
            let mut tx = self.transaction().await?;
            self.lock_source(&mut tx, true).await?;
            let digest = hash_session_token(token);
            let deleted = sqlx::query("DELETE FROM sessions WHERE token = $1")
                .bind(&digest)
                .execute(&mut *tx)
                .await?;
            if deleted.rows_affected() > 1 {
                return Err(DurableAuthError::StorageUnavailable);
            }
            let remaining = sqlx::query("SELECT token FROM sessions WHERE token = $1 FOR SHARE")
                .bind(&digest)
                .fetch_optional(&mut *tx)
                .await?;
            if remaining.is_some() {
                return Err(DurableAuthError::StorageUnavailable);
            }
            tx.commit().await?;
            Ok(())
        })
        .await
    }
}
