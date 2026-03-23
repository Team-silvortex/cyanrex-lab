use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    pub otp: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub ok: bool,
    pub message: String,
    pub username: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub authenticated: bool,
    pub username: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct TotpBootstrapRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct TotpBootstrapResponse {
    pub ok: bool,
    pub message: String,
    pub issuer: Option<String>,
    pub account_name: Option<String>,
    pub secret: Option<String>,
    pub otpauth_uri: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub ok: bool,
    pub message: String,
    pub issuer: Option<String>,
    pub account_name: Option<String>,
    pub secret: Option<String>,
    pub otpauth_uri: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
    pub otp: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteAccountRequest {
    pub password: String,
    pub otp: String,
}
