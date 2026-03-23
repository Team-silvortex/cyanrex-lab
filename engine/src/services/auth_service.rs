use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
};

use chrono::{DateTime, Duration, Utc};
use data_encoding::BASE32;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use tokio::sync::OnceCell;
use uuid::Uuid;

type HmacSha1 = Hmac<Sha1>;

const DEFAULT_ADMIN_USERNAME: &str = "admin";
const DEFAULT_ADMIN_PASSWORD: &str = "cyanrex-admin";
const DEFAULT_ADMIN_TOTP_SECRET: &str = "JBSWY3DPEHPK3PXP";
const SESSION_HOURS: i64 = 12;
const TOTP_DIGITS: u32 = 6;
const TOTP_STEP_SECONDS: i64 = 30;
const PASSWORD_HASH_ROUNDS: usize = 120_000;

#[derive(Clone)]
pub struct AuthService {
    users: Arc<RwLock<HashMap<String, UserRecord>>>,
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    db_pool: Option<PgPool>,
    schema_ready: Arc<OnceCell<()>>,
    db_disabled: Arc<AtomicBool>,
    default_admin: UserRecord,
}

#[derive(Clone)]
struct UserRecord {
    username: String,
    password_salt: String,
    password_hash: String,
    totp_secret: String,
}

#[derive(Clone)]
pub struct SessionRecord {
    pub token: String,
    pub username: String,
    pub expires_at: DateTime<Utc>,
}

pub struct LoginOk {
    pub token: String,
    pub username: String,
    pub expires_at: DateTime<Utc>,
}

pub struct TotpBootstrap {
    pub issuer: String,
    pub account_name: String,
    pub secret: String,
    pub otpauth_uri: String,
}

pub struct RegisterOk {
    pub issuer: String,
    pub account_name: String,
    pub secret: String,
    pub otpauth_uri: String,
}

#[derive(Debug)]
pub enum AuthError {
    InvalidCredentials,
    InvalidOtp,
    UserAlreadyExists,
    InvalidInput,
    WeakPassword,
    Forbidden,
}

impl AuthService {
    pub fn new_with_default_admin() -> Self {
        let username = std::env::var("CYANREX_ADMIN_USERNAME")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_ADMIN_USERNAME.to_string());

        let password = std::env::var("CYANREX_ADMIN_PASSWORD")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_ADMIN_PASSWORD.to_string());

        let totp_secret = std::env::var("CYANREX_ADMIN_TOTP_SECRET")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_ADMIN_TOTP_SECRET.to_string());

        let password_salt = generate_password_salt();
        let default_admin = UserRecord {
            username: username.clone(),
            password_salt: password_salt.clone(),
            password_hash: derive_password_hash(&password, &password_salt),
            totp_secret,
        };

        let mut users = HashMap::new();
        users.insert(username, default_admin.clone());

        let db_pool = std::env::var("DATABASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .and_then(|url| {
                PgPoolOptions::new()
                    .max_connections(5)
                    .connect_lazy(&url)
                    .ok()
            });

        Self {
            users: Arc::new(RwLock::new(users)),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            db_pool,
            schema_ready: Arc::new(OnceCell::new()),
            db_disabled: Arc::new(AtomicBool::new(false)),
            default_admin,
        }
    }

    pub async fn login(
        &self,
        username: &str,
        password: &str,
        otp: &str,
    ) -> Result<LoginOk, AuthError> {
        let user = self
            .get_user(username)
            .await
            .ok_or(AuthError::InvalidCredentials)?;

        if !verify_password(password, &user.password_salt, &user.password_hash) {
            return Err(AuthError::InvalidCredentials);
        }

        if !verify_totp(&user.totp_secret, otp) {
            return Err(AuthError::InvalidOtp);
        }

        let token = Uuid::new_v4().to_string();
        let expires_at = Utc::now() + Duration::hours(SESSION_HOURS);

        let session = SessionRecord {
            token: token.clone(),
            username: user.username.clone(),
            expires_at,
        };

        {
            let mut sessions = self.sessions.write().expect("auth sessions lock poisoned");
            sessions.insert(token.clone(), session);
        }

        if let Some(pool) = self.active_pool() {
            if let Err(error) = self.ensure_schema_and_seed().await {
                tracing::warn!("auth db unavailable, fallback to memory: {error}");
            } else if let Err(error) = sqlx::query(
                "INSERT INTO sessions (token, username, expires_at) VALUES ($1, $2, $3)",
            )
            .bind(&token)
            .bind(&user.username)
            .bind(expires_at)
            .execute(pool)
            .await
            {
                self.disable_db(&format!("insert session failed: {error}"));
            }
        }

        Ok(LoginOk {
            token,
            username: user.username,
            expires_at,
        })
    }

    pub async fn validate_session(&self, token: &str) -> Option<SessionRecord> {
        if let Some(pool) = self.active_pool() {
            if self.ensure_schema_and_seed().await.is_ok() {
                match sqlx::query(
                    "SELECT token, username, expires_at FROM sessions WHERE token = $1",
                )
                .bind(token)
                .fetch_optional(pool)
                .await
                {
                    Ok(Some(row)) => {
                        let expires_at: DateTime<Utc> = row.get("expires_at");
                        if expires_at <= Utc::now() {
                            let _ = sqlx::query("DELETE FROM sessions WHERE token = $1")
                                .bind(token)
                                .execute(pool)
                                .await;
                            return None;
                        }

                        return Some(SessionRecord {
                            token: row.get("token"),
                            username: row.get("username"),
                            expires_at,
                        });
                    }
                    Ok(None) => return None,
                    Err(error) => self.disable_db(&format!("validate session failed: {error}")),
                }
            }
        }

        let mut sessions = self.sessions.write().expect("auth sessions lock poisoned");
        if let Some(session) = sessions.get(token).cloned() {
            if session.expires_at > Utc::now() {
                return Some(session);
            }
            sessions.remove(token);
        }

        None
    }

    pub async fn logout(&self, token: &str) {
        {
            let mut sessions = self.sessions.write().expect("auth sessions lock poisoned");
            sessions.remove(token);
        }

        if let Some(pool) = self.active_pool() {
            if self.ensure_schema_and_seed().await.is_ok() {
                if let Err(error) = sqlx::query("DELETE FROM sessions WHERE token = $1")
                    .bind(token)
                    .execute(pool)
                    .await
                {
                    self.disable_db(&format!("logout delete session failed: {error}"));
                }
            }
        }
    }

    pub fn generate_current_totp_for_user(&self, username: &str) -> Option<String> {
        let users = self.users.read().expect("auth users lock poisoned");
        let user = users.get(username)?;
        Some(compute_current_totp_code(&user.totp_secret))
    }

    pub async fn bootstrap_totp(
        &self,
        username: &str,
        password: &str,
    ) -> Result<TotpBootstrap, AuthError> {
        let user = self
            .get_user(username)
            .await
            .ok_or(AuthError::InvalidCredentials)?;

        if !verify_password(password, &user.password_salt, &user.password_hash) {
            return Err(AuthError::InvalidCredentials);
        }

        let issuer = std::env::var("CYANREX_TOTP_ISSUER")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "cyanrex-lab".to_string());
        let account_name = user.username.clone();
        let otpauth_uri = build_otpauth_uri(&issuer, &account_name, &user.totp_secret);

        Ok(TotpBootstrap {
            issuer,
            account_name,
            secret: user.totp_secret,
            otpauth_uri,
        })
    }

    pub async fn register(&self, username: &str, password: &str) -> Result<RegisterOk, AuthError> {
        let normalized_username = username.trim();
        if normalized_username.len() < 3 {
            return Err(AuthError::InvalidInput);
        }
        if password.len() < 8 {
            return Err(AuthError::WeakPassword);
        }

        let totp_secret = generate_totp_secret();
        let password_salt = generate_password_salt();
        let password_hash = derive_password_hash(password, &password_salt);
        let issuer = std::env::var("CYANREX_TOTP_ISSUER")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "cyanrex-lab".to_string());
        let account_name = normalized_username.to_string();
        let otpauth_uri = build_otpauth_uri(&issuer, &account_name, &totp_secret);

        if let Some(pool) = self.active_pool() {
            if self.ensure_schema_and_seed().await.is_ok() {
                let inserted = sqlx::query(
                    "INSERT INTO users (username, password_salt, password_hash, totp_secret, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, NOW(), NOW())
                     ON CONFLICT (username) DO NOTHING
                     RETURNING username",
                )
                .bind(&account_name)
                .bind(&password_salt)
                .bind(&password_hash)
                .bind(&totp_secret)
                .fetch_optional(pool)
                .await;

                match inserted {
                    Ok(None) => return Err(AuthError::UserAlreadyExists),
                    Ok(Some(_)) => {
                        let mut users = self.users.write().expect("auth users lock poisoned");
                        users.insert(
                            account_name.clone(),
                            UserRecord {
                                username: account_name.clone(),
                                password_salt: password_salt.clone(),
                                password_hash: password_hash.clone(),
                                totp_secret: totp_secret.clone(),
                            },
                        );
                    }
                    Err(error) => {
                        self.disable_db(&format!("register insert failed: {error}"));
                    }
                }
            }
        }

        if self.active_pool().is_none() {
            let mut users = self.users.write().expect("auth users lock poisoned");
            if users.contains_key(&account_name) {
                return Err(AuthError::UserAlreadyExists);
            }
            users.insert(
                account_name.clone(),
                UserRecord {
                    username: account_name.clone(),
                    password_salt,
                    password_hash,
                    totp_secret: totp_secret.clone(),
                },
            );
        }

        Ok(RegisterOk {
            issuer,
            account_name,
            secret: totp_secret,
            otpauth_uri,
        })
    }

    pub async fn change_password(
        &self,
        username: &str,
        current_password: &str,
        new_password: &str,
        otp: &str,
    ) -> Result<(), AuthError> {
        if new_password.len() < 8 {
            return Err(AuthError::WeakPassword);
        }

        let user = self
            .get_user(username)
            .await
            .ok_or(AuthError::InvalidCredentials)?;

        if !verify_password(current_password, &user.password_salt, &user.password_hash) {
            return Err(AuthError::InvalidCredentials);
        }
        if !verify_totp(&user.totp_secret, otp) {
            return Err(AuthError::InvalidOtp);
        }

        let new_salt = generate_password_salt();
        let new_hash = derive_password_hash(new_password, &new_salt);

        if let Some(pool) = self.active_pool() {
            if self.ensure_schema_and_seed().await.is_ok() {
                if let Err(error) = sqlx::query(
                    "UPDATE users SET password_salt = $1, password_hash = $2, updated_at = NOW() WHERE username = $3",
                )
                .bind(&new_salt)
                .bind(&new_hash)
                .bind(username)
                .execute(pool)
                .await
                {
                    self.disable_db(&format!("change password update failed: {error}"));
                }
            }
        }

        let mut users = self.users.write().expect("auth users lock poisoned");
        if let Some(record) = users.get_mut(username) {
            record.password_salt = new_salt;
            record.password_hash = new_hash;
        }

        Ok(())
    }

    pub async fn delete_account(
        &self,
        username: &str,
        password: &str,
        otp: &str,
    ) -> Result<(), AuthError> {
        let user = self
            .get_user(username)
            .await
            .ok_or(AuthError::InvalidCredentials)?;

        if !verify_password(password, &user.password_salt, &user.password_hash) {
            return Err(AuthError::InvalidCredentials);
        }
        if !verify_totp(&user.totp_secret, otp) {
            return Err(AuthError::InvalidOtp);
        }

        if let Some(pool) = self.active_pool() {
            if self.ensure_schema_and_seed().await.is_ok() {
                let count = sqlx::query("SELECT COUNT(*) AS count FROM users")
                    .fetch_one(pool)
                    .await
                    .map(|row| row.get::<i64, _>("count"));

                match count {
                    Ok(total) if total <= 1 => return Err(AuthError::Forbidden),
                    Ok(_) => {
                        if let Err(error) = sqlx::query("DELETE FROM sessions WHERE username = $1")
                            .bind(username)
                            .execute(pool)
                            .await
                        {
                            self.disable_db(&format!(
                                "delete account clear sessions failed: {error}"
                            ));
                        }
                        if let Err(error) = sqlx::query("DELETE FROM users WHERE username = $1")
                            .bind(username)
                            .execute(pool)
                            .await
                        {
                            self.disable_db(&format!("delete account delete user failed: {error}"));
                        }
                    }
                    Err(error) => {
                        self.disable_db(&format!("delete account count failed: {error}"));
                    }
                }
            }
        } else {
            let users = self.users.read().expect("auth users lock poisoned");
            if users.len() <= 1 {
                return Err(AuthError::Forbidden);
            }
            drop(users);
        }

        {
            let mut users = self.users.write().expect("auth users lock poisoned");
            users.remove(username);
        }
        {
            let mut sessions = self.sessions.write().expect("auth sessions lock poisoned");
            sessions.retain(|_, session| session.username != username);
        }

        Ok(())
    }

    fn active_pool(&self) -> Option<&PgPool> {
        if self.db_disabled.load(Ordering::Relaxed) {
            return None;
        }
        self.db_pool.as_ref()
    }

    fn disable_db(&self, reason: &str) {
        tracing::warn!("disabling auth db persistence: {reason}");
        self.db_disabled.store(true, Ordering::Relaxed);
    }

    async fn ensure_schema_and_seed(&self) -> Result<(), sqlx::Error> {
        let Some(pool) = self.active_pool() else {
            return Ok(());
        };

        self.schema_ready
            .get_or_try_init(|| async {
                sqlx::query(
                    "CREATE TABLE IF NOT EXISTS users (
                        username TEXT PRIMARY KEY,
                        password_salt TEXT NOT NULL,
                        password_hash TEXT NOT NULL,
                        totp_secret TEXT NOT NULL,
                        created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                        updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                    )",
                )
                .execute(pool)
                .await?;

                sqlx::query(
                    "CREATE TABLE IF NOT EXISTS sessions (
                        token TEXT PRIMARY KEY,
                        username TEXT NOT NULL REFERENCES users(username) ON DELETE CASCADE,
                        expires_at TIMESTAMPTZ NOT NULL,
                        created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                    )",
                )
                .execute(pool)
                .await?;

                sqlx::query(
                    "INSERT INTO users (username, password_salt, password_hash, totp_secret, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, NOW(), NOW())
                     ON CONFLICT (username) DO NOTHING",
                )
                .bind(&self.default_admin.username)
                .bind(&self.default_admin.password_salt)
                .bind(&self.default_admin.password_hash)
                .bind(&self.default_admin.totp_secret)
                .execute(pool)
                .await?;

                let rows = sqlx::query(
                    "SELECT username, password_salt, password_hash, totp_secret FROM users",
                )
                .fetch_all(pool)
                .await?;

                let mut users = self.users.write().expect("auth users lock poisoned");
                users.clear();
                for row in rows {
                    users.insert(
                        row.get::<String, _>("username"),
                        UserRecord {
                            username: row.get("username"),
                            password_salt: row.get("password_salt"),
                            password_hash: row.get("password_hash"),
                            totp_secret: row.get("totp_secret"),
                        },
                    );
                }

                Ok(())
            })
            .await
            .map(|_| ())
    }

    async fn get_user(&self, username: &str) -> Option<UserRecord> {
        if let Some(pool) = self.active_pool() {
            if self.ensure_schema_and_seed().await.is_ok() {
                match sqlx::query(
                    "SELECT username, password_salt, password_hash, totp_secret FROM users WHERE username = $1",
                )
                .bind(username)
                .fetch_optional(pool)
                .await
                {
                    Ok(Some(row)) => {
                        let record = UserRecord {
                            username: row.get("username"),
                            password_salt: row.get("password_salt"),
                            password_hash: row.get("password_hash"),
                            totp_secret: row.get("totp_secret"),
                        };
                        let mut users = self.users.write().expect("auth users lock poisoned");
                        users.insert(record.username.clone(), record.clone());
                        return Some(record);
                    }
                    Ok(None) => return None,
                    Err(error) => self.disable_db(&format!("get user failed: {error}")),
                }
            }
        }

        let users = self.users.read().expect("auth users lock poisoned");
        users.get(username).cloned()
    }
}

fn derive_password_hash(password: &str, salt: &str) -> String {
    let mut material = format!("{salt}:{password}");
    for _ in 0..PASSWORD_HASH_ROUNDS {
        let mut hasher = Sha256::new();
        hasher.update(material.as_bytes());
        material = format!("{:x}", hasher.finalize());
    }
    material
}

fn verify_password(password: &str, salt: &str, expected_hash: &str) -> bool {
    derive_password_hash(password, salt) == expected_hash
}

fn generate_password_salt() -> String {
    Uuid::new_v4().to_string()
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
