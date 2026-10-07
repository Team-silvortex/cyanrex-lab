//! The existing durable writer's exact Argon2 profile, checked before worker admission.
//! This is not a configurable PHC importer or a legacy password fallback.
use super::{DurableAuthError, Result};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, SaltString},
    Algorithm, Argon2, Params, Version,
};

const MEMORY_COST: u32 = 19_456;
const TIME_COST: u32 = 2;
const LANES: u32 = 1;
const OUTPUT_BYTES: usize = 32;

// Public synthetic record, precomputed with this exact profile and a fixed public salt.
// It supplies work for a missing-account denial, never an account or accepted credential.
pub(super) const MISSING_ACCOUNT_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$Y3lhbnJleC1wdWJsaWMtbWlzc2luZy1hY2NvdW50$UkitGPnpb64CcWjacN8Pr5ls5wcDV07brjI+FEr++hY";

fn parse(encoded: &str) -> Result<PasswordHash<'_>> {
    if encoded.len() > 1024 {
        return Err(DurableAuthError::InvalidRecord);
    }
    let parsed = PasswordHash::new(encoded).map_err(|_| DurableAuthError::InvalidRecord)?;
    if parsed.algorithm.as_str() != "argon2id" || parsed.version != Some(19) {
        return Err(DurableAuthError::InvalidRecord);
    }
    let mut seen = 0;
    for (name, value) in parsed.params.iter() {
        let (bit, expected) = match name.as_str() {
            "m" => (1, MEMORY_COST),
            "t" => (2, TIME_COST),
            "p" => (4, LANES),
            _ => return Err(DurableAuthError::InvalidRecord),
        };
        // Checked decimal parsing and exact constants precede any Argon2 Params construction.
        // In particular, never let duplicate fields or unchecked p reach Params::try_from.
        if seen & bit != 0 || value.decimal().ok() != Some(expected) {
            return Err(DurableAuthError::InvalidRecord);
        }
        seen |= bit;
    }
    if seen != 7
        || parsed
            .hash
            .as_ref()
            .is_none_or(|hash| hash.len() != OUTPUT_BYTES)
    {
        return Err(DurableAuthError::InvalidRecord);
    }
    let salt = parsed.salt.ok_or(DurableAuthError::InvalidRecord)?;
    let mut salt_buffer = [0u8; 48];
    let decoded = salt
        .decode_b64(&mut salt_buffer)
        .map_err(|_| DurableAuthError::InvalidRecord)?;
    if !(8..=48).contains(&decoded.len()) {
        return Err(DurableAuthError::InvalidRecord);
    }
    Ok(parsed)
}

pub(super) fn validate(encoded: &str) -> Result<()> {
    parse(encoded).map(|_| ())
}

fn hasher() -> Result<Argon2<'static>> {
    let params = Params::new(MEMORY_COST, TIME_COST, LANES, Some(OUTPUT_BYTES))
        .map_err(|_| DurableAuthError::StorageUnavailable)?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

pub(super) fn derive(password: &str, salt: &str) -> Result<String> {
    let salt = SaltString::encode_b64(salt.as_bytes())
        .map_err(|_| DurableAuthError::StorageUnavailable)?;
    hasher()?
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| DurableAuthError::StorageUnavailable)
}

pub(super) fn verify(password: &str, encoded: &str) -> Result<bool> {
    let parsed = parse(encoded)?;
    let salt = parsed.salt.ok_or(DurableAuthError::InvalidRecord)?;
    let expected = parsed.hash.ok_or(DurableAuthError::InvalidRecord)?;
    // Use only our explicit parameters, not PasswordVerifier's stored-parameter conversion.
    // Output's equality uses the library's constant-time digest comparison.
    let actual = hasher()?
        .hash_password(password.as_bytes(), salt)
        .map_err(|_| DurableAuthError::StorageUnavailable)?
        .hash
        .ok_or(DurableAuthError::StorageUnavailable)?;
    Ok(expected == actual)
}

#[cfg(test)]
#[path = "password_profile_tests.rs"]
mod tests;
