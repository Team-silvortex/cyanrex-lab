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

include!("auth_service/service.inc.rs");
include!("auth_service/crypto.inc.rs");
