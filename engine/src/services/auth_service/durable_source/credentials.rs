//! Self-service credential rotation for the explicit source, not a live AuthService adapter.
use super::*;

impl DurableAuthSource {
    /// Reauthenticate the exact Session account, change its password and revoke ALL its Sessions.
    /// No caller-selected username, replacement token, manager reset or registry mutation.
    /// Success requires commit acknowledgement. An error/timeout is not proof of rollback;
    /// uncertain outcomes require fresh authentication, never a blind retry with a new identity.
    pub async fn change_session_password(
        &self,
        token: &str,
        current_password: &str,
        new_password: &str,
        otp: &str,
    ) -> Result<()> {
        if current_password.len() > 4096
            || !(8..=4096).contains(&new_password.len())
            || otp.len() > 64
        {
            return Err(DurableAuthError::InvalidInput);
        }
        if !valid_token(token) {
            return Err(DurableAuthError::InvalidCredentials);
        }
        let username = bounded(async {
            let digest = hash_session_token(token);
            let mut tx = self.transaction().await?;
            self.lock_source(&mut tx, false).await?;
            let session = self.session_record(&mut tx, &digest).await?.ok_or(DurableAuthError::InvalidCredentials)?;
            let verified = self.account_record(&mut tx, &session.account.username).await?.ok_or(DurableAuthError::InvalidCredentials)?;
            if verified.account != session.account {
                return Err(DurableAuthError::InvalidCredentials);
            }
            tx.commit().await?;
            // Share the bounded, clone-shared login budget. No DB lock/connection spans Argon2.
            self.admit_login(&session.account.username)?;
            if !verify_password_async(current_password, &verified.user.password_salt, &verified.user.password_hash).await
                || !verify_totp(&verified.user.totp_secret, otp) {
                return Err(DurableAuthError::InvalidCredentials);
            }
            let salt = generate_password_salt();
            let hash = derive_password_hash_async(new_password, &salt).await.ok_or(DurableAuthError::StorageUnavailable)?;
            let expected = UserRecord {
                password_salt: salt, password_hash: hash, ..verified.user.clone()
            };

            let mut tx = self.transaction().await?;
            // Acquire the writer fence before child rows; never upgrade a source SHARE lock.
            // Login rechecks credentials behind this same fence before publishing a token.
            self.lock_source(&mut tx, true).await?;
            if self.session_record(&mut tx, &digest).await?.as_ref() != Some(&session) {
                return Err(DurableAuthError::InvalidCredentials);
            }
            let current = self.account_record(&mut tx, &session.account.username).await?.ok_or(DurableAuthError::InvalidCredentials)?;
            if current.account != verified.account || !same_credentials(&current.user, &verified.user)
                || !verify_totp(&current.user.totp_secret, otp) {
                return Err(DurableAuthError::InvalidCredentials);
            }
            let sessions: i64 = sqlx::query("SELECT count(*) AS count FROM sessions WHERE username = $1 AND account_id = $2")
                .bind(session.account.username.as_str()).bind(session.account.account_id.as_uuid())
                .fetch_one(&mut *tx).await?.try_get("count")?;
            confirmed_one(sqlx::query("UPDATE users SET password_salt = $1, password_hash = $2, updated_at = clock_timestamp()
                WHERE username = $3 AND account_id = $4 AND password_salt = $5 AND password_hash = $6 AND totp_secret = $7")
                .bind(&expected.password_salt).bind(&expected.password_hash)
                .bind(session.account.username.as_str()).bind(session.account.account_id.as_uuid())
                .bind(&verified.user.password_salt).bind(&verified.user.password_hash).bind(&verified.user.totp_secret)
                .execute(&mut *tx).await?.rows_affected())?;
            // Detect a write trigger revoking/altering the authorizing Session before our revoke.
            if self.session_record(&mut tx, &digest).await?.as_ref() != Some(&session) {
                return Err(DurableAuthError::InvalidCredentials);
            }
            let removed = sqlx::query("DELETE FROM sessions WHERE username = $1 AND account_id = $2")
                .bind(session.account.username.as_str()).bind(session.account.account_id.as_uuid())
                .execute(&mut *tx).await?.rows_affected();
            if removed != sessions as u64 {
                return Err(DurableAuthError::StorageUnavailable);
            }
            let remains: bool = sqlx::query("SELECT EXISTS (SELECT 1 FROM sessions WHERE username = $1 OR account_id = $2) AS remains")
                .bind(session.account.username.as_str()).bind(session.account.account_id.as_uuid())
                .fetch_one(&mut *tx).await?.try_get("remains")?;
            let actual = self.account_record(&mut tx, &session.account.username).await?.ok_or(DurableAuthError::StorageUnavailable)?;
            if remains || actual.account != session.account || !same_credentials(&actual.user, &expected) {
                return Err(DurableAuthError::StorageUnavailable);
            }
            // The authorizing row is intentionally gone; retain its exact expiry and use fresh
            // DB time after UPDATE/DELETE trigger waits, not transaction-start time or a cache.
            if session.expires_at <= Self::now(&mut tx).await? {
                return Err(DurableAuthError::InvalidCredentials);
            }
            tx.commit().await?;
            Ok(session.account.username)
        }).await?;
        self.clear_login_attempts(&username);
        Ok(())
    }
}
