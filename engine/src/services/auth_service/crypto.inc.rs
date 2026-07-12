fn derive_password_hash(password: &str, salt: &str) -> String {
    let encoded_salt = SaltString::encode_b64(salt.as_bytes()).expect("UUID salt should be valid");
    Argon2::default()
        .hash_password(password.as_bytes(), &encoded_salt)
        .expect("Argon2 password hashing should succeed")
        .to_string()
}

fn derive_legacy_password_hash(password: &str, salt: &str) -> String {
    let mut material = format!("{salt}:{password}");
    for _ in 0..LEGACY_PASSWORD_HASH_ROUNDS {
        let mut hasher = Sha256::new();
        hasher.update(material.as_bytes());
        material = format!("{:x}", hasher.finalize());
    }
    material
}

fn verify_password(password: &str, salt: &str, expected_hash: &str) -> bool {
    if expected_hash.starts_with("$argon2") {
        return PasswordHash::new(expected_hash)
            .ok()
            .is_some_and(|parsed| {
                Argon2::default()
                    .verify_password(password.as_bytes(), &parsed)
                    .is_ok()
            });
    }
    derive_legacy_password_hash(password, salt) == expected_hash
}

fn generate_password_salt() -> String {
    Uuid::new_v4().to_string()
}

fn hash_session_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn generate_totp_secret() -> String {
    let mut hasher = Sha256::new();
    hasher.update(Uuid::new_v4().as_bytes());
    let digest = hasher.finalize();
    BASE32.encode(&digest[..16])
}

fn verify_totp(secret: &str, otp: &str) -> bool {
    let normalized = otp.trim();
    if normalized.len() != TOTP_DIGITS as usize || !normalized.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    let secret_bytes = match decode_base32_secret(secret) {
        Some(bytes) => bytes,
        None => return false,
    };

    let current_counter = Utc::now().timestamp().div_euclid(TOTP_STEP_SECONDS);

    for drift in -1..=1 {
        let counter = current_counter + drift;
        if hotp_code(&secret_bytes, counter as u64) == normalized {
            return true;
        }
    }

    false
}

fn compute_current_totp_code(secret: &str) -> String {
    let secret_bytes = decode_base32_secret(secret).unwrap_or_default();
    if secret_bytes.is_empty() {
        return "000000".to_string();
    }

    let counter = Utc::now().timestamp().div_euclid(TOTP_STEP_SECONDS) as u64;
    hotp_code(&secret_bytes, counter)
}

fn decode_base32_secret(secret: &str) -> Option<Vec<u8>> {
    let normalized = secret
        .trim()
        .to_ascii_uppercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();

    BASE32.decode(normalized.as_bytes()).ok()
}

fn hotp_code(secret: &[u8], counter: u64) -> String {
    let mut mac = HmacSha1::new_from_slice(secret).expect("invalid HMAC key length");
    mac.update(&counter.to_be_bytes());

    let hash = mac.finalize().into_bytes();
    let offset = (hash[19] & 0x0f) as usize;
    let binary = ((hash[offset] as u32 & 0x7f) << 24)
        | ((hash[offset + 1] as u32) << 16)
        | ((hash[offset + 2] as u32) << 8)
        | (hash[offset + 3] as u32);

    let code = binary % 10_u32.pow(TOTP_DIGITS);
    format!("{:06}", code)
}

fn build_otpauth_uri(issuer: &str, account_name: &str, secret: &str) -> String {
    format!(
        "otpauth://totp/{issuer}:{account_name}?secret={secret}&issuer={issuer}&algorithm=SHA1&digits={TOTP_DIGITS}&period={TOTP_STEP_SECONDS}"
    )
}
