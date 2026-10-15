//! Explicit trusted per-call Session stock policy, not a persisted global quota or cleanup rule.

use super::{sqlx, DurableAuthError, LegacyAccountId, Postgres, Result, Row, Transaction};

/// Counts every persistent Session row, including expired rows; no implicit maintenance.
/// The legacy login entry point remains independent of this caller-supplied policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionStockLimits {
    per_account: u16,
    total: u16,
}

impl SessionStockLimits {
    pub fn new(per_account: u16, total: u16) -> Result<Self> {
        if !(1..=64).contains(&per_account) || !(1..=4096).contains(&total) || per_account > total {
            return Err(DurableAuthError::InvalidInput);
        }
        Ok(Self { per_account, total })
    }

    pub fn per_account(&self) -> u16 {
        self.per_account
    }

    pub fn total(&self) -> u16 {
        self.total
    }

    /// The caller has already verified credentials/TOTP and holds the original source writer
    /// fence. Count all rows, not just active or well-formed Sessions, before consuming the OTP.
    pub(super) async fn check_before(
        self,
        tx: &mut Transaction<'_, Postgres>,
        account_id: LegacyAccountId,
    ) -> Result<SessionStock> {
        let total = total_stock(tx, self.total).await?;
        if total >= i64::from(self.total) {
            return Err(DurableAuthError::RateLimited);
        }
        let per_account = account_stock(tx, account_id, self.per_account).await?;
        if per_account >= i64::from(self.per_account) {
            return Err(DurableAuthError::RateLimited);
        }
        Ok(SessionStock {
            limits: self,
            account_id,
            total,
            per_account,
        })
    }
}

/// Private counts, not a reservation, full-ledger fingerprint or authority to issue later.
pub(super) struct SessionStock {
    limits: SessionStockLimits,
    account_id: LegacyAccountId,
    total: i64,
    per_account: i64,
}

impl SessionStock {
    /// Check the expected one-row increment after all writes/deferred work and original pin,
    /// credential, watermark and selected-Session verification. Keep no SQL after final OTP proof.
    pub(super) async fn verify_after(self, tx: &mut Transaction<'_, Postgres>) -> Result<()> {
        let total = total_stock(tx, self.limits.total).await?;
        let per_account = account_stock(tx, self.account_id, self.limits.per_account).await?;
        if total > i64::from(self.limits.total)
            || per_account > i64::from(self.limits.per_account)
            || total != self.total + 1
            || per_account != self.per_account + 1
        {
            return Err(DurableAuthError::StorageUnavailable);
        }
        Ok(())
    }
}

async fn total_stock(tx: &mut Transaction<'_, Postgres>, limit: u16) -> Result<i64> {
    let stock = sqlx::query(
        "SELECT COUNT(*) AS stock FROM
        (SELECT 1 FROM sessions LIMIT $1) AS bounded_sessions",
    )
    .bind(i64::from(limit) + 1)
    .fetch_one(&mut **tx)
    .await?
    .try_get("stock")?;
    checked_count(stock, limit)
}

async fn account_stock(
    tx: &mut Transaction<'_, Postgres>,
    account_id: LegacyAccountId,
    limit: u16,
) -> Result<i64> {
    let stock = sqlx::query(
        "SELECT COUNT(*) AS stock FROM
        (SELECT 1 FROM sessions WHERE account_id = $1 LIMIT $2) AS bounded_sessions",
    )
    .bind(account_id.as_uuid())
    .bind(i64::from(limit) + 1)
    .fetch_one(&mut **tx)
    .await?
    .try_get("stock")?;
    checked_count(stock, limit)
}

fn checked_count(stock: i64, limit: u16) -> Result<i64> {
    if !(0..=i64::from(limit) + 1).contains(&stock) {
        return Err(DurableAuthError::StorageUnavailable);
    }
    Ok(stock)
}
