use super::login_transaction::{LoginContext, PreparedLoginRead};
use super::session_transaction::{PreparedSessionLogout, PreparedSessionRead};
use super::*;

impl DurableAuthSource {
    /// Explicit trusted Session stock limits; never public issuance, installation or cleanup.
    pub async fn login_with_session_limits(
        &self,
        username: &LegacyUsername,
        password: &str,
        otp: &str,
        limits: SessionStockLimits,
    ) -> Result<DurableLogin> {
        self.login_inner(username, password, otp, Some(limits))
            .await
    }

    pub async fn login(
        &self,
        username: &LegacyUsername,
        password: &str,
        otp: &str,
    ) -> Result<DurableLogin> {
        self.login_inner(username, password, otp, None).await
    }

    async fn login_inner(
        &self,
        username: &LegacyUsername,
        password: &str,
        otp: &str,
        limits: Option<SessionStockLimits>,
    ) -> Result<DurableLogin> {
        LoginContext::validate_input(password, otp)?;
        self.admit_login(username)?;
        let context = LoginContext::from_source(self);
        let result = bounded(async {
            let mut tx = self.transaction().await?;
            let prepared = PreparedLoginRead::prepare(&context, &mut tx, username).await?;
            tx.commit().await?;
            // The original first Source pin and account snapshot cross the password-work gap.
            let verified = prepared.verify_password(&context, password, otp).await?;
            let mut tx = self.transaction().await?;
            if limits.is_some() {
                // Fresh stock counts after the writer wait; preserve explicit READ ONLY.
                sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
                    .execute(&mut *tx)
                    .await?;
            }
            let prepared = verified.prepare(&context, &mut tx, otp, limits).await?;
            prepared.commit(&context, tx, otp).await
        })
        .await;
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
        super::session_transaction::record(self.authority_id, connection, digest).await
    }

    /// Fresh, read-only authentication snapshot; does not authorize a later policy/identity write.
    /// No DDL, cache, memory fallback, expiry deletion or Principal allocation is performed.
    pub async fn validate_session(&self, token: &str) -> Result<Option<DurableSession>> {
        if !valid_token(token) {
            return Ok(None);
        }
        bounded(async {
            let mut tx = self.transaction().await?;
            let prepared = PreparedSessionRead::prepare(self.authority_id, &mut tx, token).await?;
            let session = prepared.finish(&mut tx).await?;
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
            let prepared =
                PreparedSessionLogout::prepare(self.authority_id, &mut tx, token).await?;
            prepared.finish(&mut tx).await?;
            tx.commit().await?;
            Ok(())
        })
        .await
    }
}
