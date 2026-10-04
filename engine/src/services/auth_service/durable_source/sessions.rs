use super::session_source::SessionSourcePin;
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
            let pin = SessionSourcePin::capture(self, &mut tx, false).await?;
            let verified = self.account_record(&mut tx, username).await?.ok_or(DurableAuthError::InvalidCredentials)?;
            pin.verify(self, &mut tx, false).await?;
            tx.commit().await?;
            // Do not keep a database lock/connection across expensive password verification.
            if !verify_password(password, &verified.user.password_salt, &verified.user.password_hash).await?
                || !self.verify_current_totp(&verified.user.totp_secret, otp) { return Err(DurableAuthError::InvalidCredentials); }
            let mut tx = self.transaction().await?;
            // The password worker gap must not silently admit an equal-valued replacement source.
            pin.verify(self, &mut tx, true).await?;
            let current = self.account_record(&mut tx, username).await?.ok_or(DurableAuthError::InvalidCredentials)?;
            if current.account != verified.account || !same_credentials(&current.user, &verified.user) {
                return Err(DurableAuthError::InvalidCredentials);
            }
            let pending_otp = self.prepare_otp_consumption(&current, otp)?;
            Self::consume_login_otp(&mut tx, &current, &pending_otp).await?;
            // Consumption triggers may suppress/alter the watermark, credentials or source path.
            pin.verify(self, &mut tx, true).await?;
            let consumed = self.account_record(&mut tx, username).await?.ok_or(DurableAuthError::StorageUnavailable)?;
            if consumed.account != verified.account || !same_credentials(&consumed.user, &verified.user)
                || consumed.otp_watermark != pending_otp.next() {
                return Err(DurableAuthError::StorageUnavailable);
            }
            let token = Uuid::new_v4().to_string();
            let digest = hash_session_token(&token);
            let expires_at = Self::now(&mut tx).await? + chrono::Duration::hours(12);
            confirmed_one(sqlx::query("INSERT INTO sessions (token, username, account_id, expires_at) VALUES ($1, $2, $3, $4)")
                .bind(&digest).bind(username.as_str()).bind(verified.account.account_id.as_uuid()).bind(expires_at)
                .execute(&mut *tx).await?.rows_affected())?;
            // INSERT triggers can change credentials/identity in our own transaction. Verify both
            // the exact source generation and complete session row again before COMMIT/publication.
            pin.verify(self, &mut tx, true).await?;
            let current = self.account_record(&mut tx, username).await?.ok_or(DurableAuthError::InvalidCredentials)?;
            if current.account != verified.account || !same_credentials(&current.user, &verified.user) {
                return Err(DurableAuthError::InvalidCredentials);
            }
            if current.otp_watermark != pending_otp.next() { return Err(DurableAuthError::StorageUnavailable); }
            let expected = DurableSession { account: verified.account, expires_at };
            if self.session_record(&mut tx, &digest).await?.as_ref() != Some(&expected) { return Err(DurableAuthError::StorageUnavailable); }
            // The writer/INSERT and final readback may wait across a TOTP boundary. No SQL
            // remains between this fresh OTP check and the attempt to confirm commit.
            self.recheck_otp_consumption(&pending_otp, &current.user.totp_secret, otp)?;
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
            let pin = SessionSourcePin::capture(self, &mut tx, false).await?;
            // Lock the observed Session/account, then recheck the source before the final fresh
            // expiry read. Guard waits must never make an earlier validity result reusable.
            self.session_record(&mut tx, &hash_session_token(token))
                .await?;
            pin.verify(self, &mut tx, false).await?;
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
            let pin = SessionSourcePin::capture(self, &mut tx, true).await?;
            let digest = hash_session_token(token);
            let deleted = sqlx::query("DELETE FROM sessions WHERE token = $1")
                .bind(&digest)
                .execute(&mut *tx)
                .await?;
            if deleted.rows_affected() > 1 {
                return Err(DurableAuthError::StorageUnavailable);
            }
            // A DELETE trigger may suppress deletion and redirect unqualified reads to an empty
            // shadow. Prove we still read the original source before checking absence.
            pin.verify(self, &mut tx, true).await?;
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
