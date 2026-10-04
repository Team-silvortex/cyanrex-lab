//! Pure counter policy used by the prepared source's atomic OTP-consumption transaction.
//! Supplied state is not trusted storage, account identity, an authorization proof or a commit receipt.
use super::super::{decode_base32_secret, hotp_code, TOTP_DIGITS, TOTP_STEP_SECONDS};
use super::{DateTime, Utc};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConsumptionError {
    InvalidState,
    InvalidCredentials,
}

/// Highest consumed counter, including all earlier counters. -1 means none has been consumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Watermark(i64);

impl Watermark {
    pub(super) fn from_stored(value: i64) -> Result<Self, ConsumptionError> {
        if value < -1 {
            return Err(ConsumptionError::InvalidState);
        }
        Ok(Self(value))
    }

    pub(super) fn stored(self) -> i64 {
        self.0
    }
}

/// A proposed state transition, deliberately not Debug/Serialize/Clone and not reusable authority.
pub(super) struct PendingConsumption {
    previous: Watermark,
    next: Watermark,
    binding: [u8; 32],
}

impl PendingConsumption {
    pub(super) fn previous(&self) -> Watermark {
        self.previous
    }

    pub(super) fn next(&self) -> Watermark {
        self.next
    }

    pub(super) fn recheck(
        &self,
        secret: &str,
        otp: &str,
        at: DateTime<Utc>,
    ) -> Result<(), ConsumptionError> {
        // The caller must separately compare/read back durable state under its transaction.
        // Rechecking with our proposed next watermark would reject our own pending consumption.
        let current = prepare(secret, otp, at, self.previous)?;
        if current.next != self.next || current.binding != self.binding {
            return Err(ConsumptionError::InvalidCredentials);
        }
        Ok(())
    }
}

pub(super) fn prepare(
    secret: &str,
    otp: &str,
    at: DateTime<Utc>,
    watermark: Watermark,
) -> Result<PendingConsumption, ConsumptionError> {
    // Match the prepared source's existing raw input/record bounds before allocation/normalization.
    if secret.len() > 128 || otp.len() > 64 || at.timestamp() < 0 {
        return Err(ConsumptionError::InvalidCredentials);
    }
    let normalized = otp.trim();
    if normalized.len() != TOTP_DIGITS as usize
        || !normalized.bytes().all(|digit| digit.is_ascii_digit())
    {
        return Err(ConsumptionError::InvalidCredentials);
    }
    let secret = decode_base32_secret(secret)
        .filter(|bytes| !bytes.is_empty())
        .ok_or(ConsumptionError::InvalidCredentials)?;
    let current = at.timestamp().div_euclid(TOTP_STEP_SECONDS);
    let mut highest = None;
    for drift in -1..=1 {
        // DateTime's timestamp range fits safely; checked addition also documents no wrap policy.
        let counter = current
            .checked_add(drift)
            .ok_or(ConsumptionError::InvalidCredentials)?;
        if counter < 0 || hotp_code(&secret, counter as u64) != normalized {
            continue;
        }
        // Never filter old counters before matching: a newer counter may generate the same digits.
        if counter <= watermark.0 {
            return Err(ConsumptionError::InvalidCredentials);
        }
        highest = Some(counter);
    }
    let next = Watermark(highest.ok_or(ConsumptionError::InvalidCredentials)?);
    let mut binding = Sha256::new();
    binding.update(b"cyanrex.prepared-otp-consumption.v1\0");
    binding.update((secret.len() as u64).to_be_bytes());
    binding.update(&secret);
    binding.update(normalized.as_bytes());
    Ok(PendingConsumption {
        previous: watermark,
        next,
        binding: binding.finalize().into(),
    })
}

#[cfg(test)]
#[path = "otp_consumption_tests.rs"]
mod tests;
