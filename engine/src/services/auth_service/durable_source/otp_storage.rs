//! Compare-and-update only inside a caller-owned, pinned source writer transaction.
use super::{accounts::AccountRecord, otp_consumption::PendingConsumption, *};

impl DurableAuthSource {
    /// Caller checks the pending watermark, credentials and business result again after writes.
    /// No commit, fallback, reset, last-code cache or independent consumption transaction.
    pub(super) async fn consume_login_otp(
        tx: &mut Transaction<'_, Postgres>,
        account: &AccountRecord,
        pending: &PendingConsumption,
    ) -> Result<()> {
        confirmed_one(
            sqlx::query(
                "UPDATE users SET otp_last_counter = $1
            WHERE username = $2 AND account_id = $3 AND otp_last_counter = $4
                AND password_salt = $5 AND password_hash = $6 AND totp_secret = $7",
            )
            .bind(pending.next().stored())
            .bind(account.account.username.as_str())
            .bind(account.account.account_id.as_uuid())
            .bind(pending.previous().stored())
            .bind(&account.user.password_salt)
            .bind(&account.user.password_hash)
            .bind(&account.user.totp_secret)
            .execute(&mut **tx)
            .await?
            .rows_affected(),
        )
    }
}
