//! Durable-only TOTP timing and consumption proof preparation; live AuthService remains separate.
use super::super::{decode_base32_secret, hotp_code, TOTP_DIGITS, TOTP_STEP_SECONDS};
use super::{accounts::AccountRecord, otp_consumption, DateTime, DurableAuthSource, Result, Utc};

pub(super) fn verify_at(secret: &str, otp: &str, at: DateTime<Utc>) -> bool {
    if at.timestamp() < 0 {
        return false;
    }
    let normalized = otp.trim();
    if normalized.len() != TOTP_DIGITS as usize
        || !normalized.chars().all(|digit| digit.is_ascii_digit())
    {
        return false;
    }
    let Some(secret_bytes) = decode_base32_secret(secret).filter(|bytes| !bytes.is_empty()) else {
        return false;
    };
    let current = at.timestamp().div_euclid(TOTP_STEP_SECONDS);
    for drift in -1..=1 {
        let counter = current + drift;
        // There is no negative HOTP counter at the Unix epoch.
        if counter >= 0 && hotp_code(&secret_bytes, counter as u64) == normalized {
            return true;
        }
    }
    false
}

impl DurableAuthSource {
    fn otp_now(&self) -> DateTime<Utc> {
        #[cfg(test)]
        if let Some(clock) = &self.otp_clock {
            return clock();
        }
        Utc::now()
    }

    pub(super) fn verify_current_totp(&self, secret: &str, otp: &str) -> bool {
        verify_at(secret, otp, self.otp_now())
    }

    pub(super) fn prepare_otp_consumption(
        &self,
        account: &AccountRecord,
        otp: &str,
    ) -> Result<otp_consumption::PendingConsumption> {
        Ok(otp_consumption::prepare(
            &account.user.totp_secret,
            otp,
            self.otp_now(),
            account.otp_watermark,
        )?)
    }

    pub(super) fn recheck_otp_consumption(
        &self,
        pending: &otp_consumption::PendingConsumption,
        secret: &str,
        otp: &str,
    ) -> Result<()> {
        Ok(pending.recheck(secret, otp, self.otp_now())?)
    }
}

#[cfg(test)]
#[path = "otp_consumption_sql_tests.rs"]
mod consumption_sql_tests;
#[cfg(test)]
#[path = "otp_sql_tests.rs"]
mod sql_tests;
#[cfg(test)]
#[path = "otp_tests.rs"]
mod tests;
