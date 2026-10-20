//! Shared private login stages. Callers own connections and insert retained guards before commit.
use super::{
    accounts::{self, AccountRecord},
    otp_consumption::PendingConsumption,
    session_source::SessionSourcePin,
    session_stock::SessionStock,
    *,
};

pub(crate) struct LoginContext {
    authority_id: AuthorityId,
    attempts: Arc<Mutex<HashMap<String, (u8, Instant)>>>,
    #[cfg(test)]
    otp_clock: Option<Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>>,
}

impl LoginContext {
    pub(crate) fn new(authority_id: AuthorityId) -> Self {
        Self {
            authority_id,
            attempts: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(test)]
            otp_clock: None,
        }
    }

    pub(super) fn from_source(source: &DurableAuthSource) -> Self {
        Self {
            authority_id: source.authority_id,
            attempts: source.attempts.clone(),
            #[cfg(test)]
            otp_clock: source.otp_clock.clone(),
        }
    }

    pub(crate) fn validate_input(password: &str, otp: &str) -> Result<()> {
        if password.len() > 4096 || otp.len() > 64 {
            return Err(DurableAuthError::InvalidInput);
        }
        Ok(())
    }

    pub(crate) fn admit(&self, username: &LegacyUsername) -> Result<()> {
        let mut attempts = self
            .attempts
            .lock()
            .map_err(|_| DurableAuthError::StorageUnavailable)?;
        attempts.retain(|_, (_, at)| at.elapsed() < Duration::from_secs(300));
        if !attempts.contains_key(username.as_str()) && attempts.len() >= 1024 {
            return Err(DurableAuthError::RateLimited);
        }
        let (count, _) = attempts
            .entry(username.to_string())
            .or_insert((0, Instant::now()));
        if *count >= 5 {
            return Err(DurableAuthError::RateLimited);
        }
        *count += 1;
        Ok(())
    }

    pub(crate) fn clear(&self, username: &LegacyUsername) {
        if let Ok(mut attempts) = self.attempts.lock() {
            attempts.remove(username.as_str());
        }
    }

    fn now(&self) -> DateTime<Utc> {
        #[cfg(test)]
        if let Some(clock) = &self.otp_clock {
            return clock();
        }
        Utc::now()
    }
}

/// Neither first-read preparation nor its credential snapshot authorizes issuance or COMMIT.
pub(crate) struct PreparedLoginRead {
    authority_id: AuthorityId,
    username: LegacyUsername,
    pin: SessionSourcePin,
    verified: Option<AccountRecord>,
}

impl PreparedLoginRead {
    pub(crate) async fn prepare(
        context: &LoginContext,
        tx: &mut Transaction<'_, Postgres>,
        username: &LegacyUsername,
    ) -> Result<Self> {
        let pin = SessionSourcePin::capture_for(context.authority_id, tx, false).await?;
        let verified = accounts::account_record(context.authority_id, tx, username).await?;
        pin.verify_for(context.authority_id, tx, false).await?;
        Ok(Self {
            authority_id: context.authority_id,
            username: username.clone(),
            pin,
            verified,
        })
    }

    /// Call only after the read transaction and its connection have been released.
    pub(crate) async fn verify_password(
        self,
        context: &LoginContext,
        password: &str,
        otp: &str,
    ) -> Result<VerifiedLogin> {
        let (salt, hash) = self
            .verified
            .as_ref()
            .map(|record| {
                (
                    record.user.password_salt.as_str(),
                    record.user.password_hash.as_str(),
                )
            })
            .unwrap_or(("", password_profile::MISSING_ACCOUNT_HASH));
        let password_matches = verify_password(password, salt, hash).await?;
        // A matching public dummy hash never promotes the original absent snapshot.
        let verified = self.verified.ok_or(DurableAuthError::InvalidCredentials)?;
        if !password_matches || !otp::verify_at(&verified.user.totp_secret, otp, context.now()) {
            return Err(DurableAuthError::InvalidCredentials);
        }
        Ok(VerifiedLogin {
            authority_id: self.authority_id,
            username: self.username,
            pin: self.pin,
            verified,
        })
    }
}

pub(crate) struct VerifiedLogin {
    authority_id: AuthorityId,
    username: LegacyUsername,
    pin: SessionSourcePin,
    verified: AccountRecord,
}

impl VerifiedLogin {
    /// Caller selects RC for bounded stock, retains READ ONLY, and owns any outer storage guards.
    pub(crate) async fn prepare(
        self,
        context: &LoginContext,
        tx: &mut Transaction<'_, Postgres>,
        otp: &str,
        limits: Option<SessionStockLimits>,
    ) -> Result<PreparedLoginWrite> {
        self.pin.verify_for(self.authority_id, tx, true).await?;
        let current = accounts::account_record(self.authority_id, tx, &self.username)
            .await?
            .ok_or(DurableAuthError::InvalidCredentials)?;
        if current.account != self.verified.account
            || !same_credentials(&current.user, &self.verified.user)
        {
            return Err(DurableAuthError::InvalidCredentials);
        }
        let pending_otp = otp_consumption::prepare(
            &current.user.totp_secret,
            otp,
            context.now(),
            current.otp_watermark,
        )?;
        let stock = match limits {
            Some(limits) => Some(limits.check_before(tx, current.account.account_id).await?),
            None => None,
        };
        DurableAuthSource::consume_login_otp(tx, &current, &pending_otp).await?;
        self.pin.verify_for(self.authority_id, tx, true).await?;
        let consumed = accounts::account_record(self.authority_id, tx, &self.username)
            .await?
            .ok_or(DurableAuthError::StorageUnavailable)?;
        if consumed.account != self.verified.account
            || !same_credentials(&consumed.user, &self.verified.user)
            || consumed.otp_watermark != pending_otp.next()
        {
            return Err(DurableAuthError::StorageUnavailable);
        }
        let token = Uuid::new_v4().to_string();
        let digest = hash_session_token(&token);
        let expires_at = DurableAuthSource::now(tx).await? + chrono::Duration::hours(12);
        confirmed_one(sqlx::query("INSERT INTO sessions (token, username, account_id, expires_at) VALUES ($1, $2, $3, $4)")
            .bind(&digest).bind(self.username.as_str()).bind(self.verified.account.account_id.as_uuid()).bind(expires_at)
            .execute(&mut **tx).await?.rows_affected())?;
        if limits.is_some() {
            sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
                .execute(&mut **tx)
                .await?;
        }
        Ok(PreparedLoginWrite {
            login: self,
            pending_otp,
            stock,
            token,
            digest,
            expires_at,
        })
    }
}

/// Owns an uncommitted secret. Only the consumed, successfully committed result may escape.
pub(crate) struct PreparedLoginWrite {
    login: VerifiedLogin,
    pending_otp: PendingConsumption,
    stock: Option<SessionStock>,
    token: String,
    digest: String,
    expires_at: DateTime<Utc>,
}

impl PreparedLoginWrite {
    /// All retained-target guards precede this method. No callback or SQL follows final OTP proof.
    pub(crate) async fn commit(
        self,
        context: &LoginContext,
        mut tx: Transaction<'_, Postgres>,
        otp: &str,
    ) -> Result<DurableLogin> {
        let login = self.login;
        login
            .pin
            .verify_for(login.authority_id, &mut tx, true)
            .await?;
        let current = accounts::account_record(login.authority_id, &mut tx, &login.username)
            .await?
            .ok_or(DurableAuthError::InvalidCredentials)?;
        if current.account != login.verified.account
            || !same_credentials(&current.user, &login.verified.user)
        {
            return Err(DurableAuthError::InvalidCredentials);
        }
        if current.otp_watermark != self.pending_otp.next() {
            return Err(DurableAuthError::StorageUnavailable);
        }
        let expected = DurableSession {
            account: login.verified.account,
            expires_at: self.expires_at,
        };
        if super::session_transaction::record(login.authority_id, &mut tx, &self.digest)
            .await?
            .as_ref()
            != Some(&expected)
        {
            return Err(DurableAuthError::StorageUnavailable);
        }
        if let Some(stock) = self.stock {
            stock.verify_after(&mut tx).await?;
        }
        self.pending_otp
            .recheck(&current.user.totp_secret, otp, context.now())?;
        tx.commit().await?;
        Ok(DurableLogin {
            token: self.token,
            session: expected,
        })
    }
}

#[cfg(test)]
#[path = "login_transaction_tests.rs"]
mod tests;
