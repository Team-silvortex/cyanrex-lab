use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
};

use crate::models::auth::AuthRole;
use crate::sqlx_compat as sqlx;
use crate::sqlx_compat::{PgPool, PgPoolOptions, Row};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{DateTime, Duration, Utc};
use data_encoding::BASE32;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use tokio::sync::OnceCell;
use uuid::Uuid;

type HmacSha1 = Hmac<Sha1>;

const DEFAULT_ADMIN_USERNAME: &str = "admin";
const DEFAULT_ADMIN_PASSWORD: &str = "cyanrex-admin";
const DEFAULT_ADMIN_TOTP_SECRET: &str = "JBSWY3DPEHPK3PXP";
const SESSION_HOURS: i64 = 12;
const TOTP_DIGITS: u32 = 6;
const TOTP_STEP_SECONDS: i64 = 30;
const LEGACY_PASSWORD_HASH_ROUNDS: usize = 120_000;
const USERNAME_MAX_LEN: usize = 64;
const USERNAME_MIN_LEN: usize = 3;

#[derive(Clone)]
pub struct AuthService {
    users: Arc<RwLock<HashMap<String, UserRecord>>>,
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    login_attempts: Arc<RwLock<HashMap<String, LoginAttempt>>>,
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
struct LoginAttempt {
    failures: u32,
    blocked_until: Option<DateTime<Utc>>,
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
    RateLimited,
}

include!("auth_service/service.inc.rs");
include!("auth_service/crypto.inc.rs");

impl AuthService {
    pub fn role_for_username(&self, username: &str) -> AuthRole {
        let normalized = normalize_role_username(username);
        let mut admin_users = parse_role_usernames("CYANREX_ADMIN_USERNAMES");
        admin_users.insert(self.default_admin.username.to_ascii_lowercase());
        if admin_users.contains(&normalized) {
            return AuthRole::Admin;
        }

        if parse_role_usernames("CYANREX_TEACHER_USERNAMES").contains(&normalized) {
            return AuthRole::Teacher;
        }

        AuthRole::Student
    }

    pub fn is_admin_username(&self, username: &str) -> bool {
        matches!(self.role_for_username(username), AuthRole::Admin)
    }
}

fn parse_role_usernames(name: &str) -> HashSet<String> {
    std::env::var(name)
        .ok()
        .map(|raw| {
            raw.split([',', ';', ' '].as_ref())
                .map(normalize_role_username)
                .filter(|item| !item.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_role_username(username: &str) -> String {
    username.trim().to_ascii_lowercase()
}

fn sanitize_username(username: &str) -> Result<String, ()> {
    let normalized = username.trim().to_ascii_lowercase();
    if normalized.len() < USERNAME_MIN_LEN || normalized.len() > USERNAME_MAX_LEN {
        return Err(());
    }

    if !normalized
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(());
    }

    Ok(normalized)
}
