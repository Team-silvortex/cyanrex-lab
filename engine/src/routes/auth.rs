use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    models::auth::{
        ChangePasswordRequest, DeleteAccountRequest, LoginRequest, LoginResponse, RegisterRequest,
        RegisterResponse, SessionResponse, TotpBootstrapRequest, TotpBootstrapResponse,
    },
    services::auth_service::AuthError,
    AppState,
};

pub const SESSION_COOKIE_NAME: &str = "cyanrex_session";

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(request): Json<LoginRequest>,
) -> Response {
    match state
        .auth_service
        .login(&request.username, &request.password, &request.otp)
        .await
    {
        Ok(ok) => {
            let cookie_value = build_session_cookie(&ok.token, 12 * 60 * 60);
            let mut response = Json(LoginResponse {
                ok: true,
                message: "login success".to_string(),
                username: Some(ok.username),
                expires_at: Some(ok.expires_at),
            })
            .into_response();

            response.headers_mut().insert(
                header::SET_COOKIE,
                HeaderValue::from_str(&cookie_value)
                    .expect("generated session cookie should always be valid"),
            );

            response
        }
        Err(AuthError::InvalidCredentials) => (
            StatusCode::UNAUTHORIZED,
            Json(LoginResponse {
                ok: false,
                message: "invalid username or password".to_string(),
                username: None,
                expires_at: None,
            }),
        )
            .into_response(),
        Err(AuthError::InvalidOtp) => (
            StatusCode::UNAUTHORIZED,
            Json(LoginResponse {
                ok: false,
                message: "invalid otp".to_string(),
                username: None,
                expires_at: None,
            }),
        )
            .into_response(),
        Err(
            AuthError::UserAlreadyExists
            | AuthError::InvalidInput
            | AuthError::WeakPassword
            | AuthError::Forbidden,
        ) => (
            StatusCode::BAD_REQUEST,
            Json(LoginResponse {
                ok: false,
                message: "login failed".to_string(),
                username: None,
                expires_at: None,
            }),
        )
            .into_response(),
    }
}

pub async fn me(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Json<SessionResponse> {
    if let Some(session) = current_session_from_headers(&state, &headers).await {
        return Json(SessionResponse {
            authenticated: true,
            username: Some(session.username),
            expires_at: Some(session.expires_at),
        });
    }

    Json(SessionResponse {
        authenticated: false,
        username: None,
        expires_at: None,
    })
}

pub async fn bootstrap_totp(
    State(state): State<Arc<AppState>>,
    Json(request): Json<TotpBootstrapRequest>,
) -> Response {
    match state
        .auth_service
        .bootstrap_totp(&request.username, &request.password)
        .await
    {
        Ok(payload) => Json(TotpBootstrapResponse {
            ok: true,
            message: "totp bootstrap ready".to_string(),
            issuer: Some(payload.issuer),
            account_name: Some(payload.account_name),
            secret: Some(payload.secret),
            otpauth_uri: Some(payload.otpauth_uri),
        })
        .into_response(),
        Err(AuthError::InvalidCredentials) => (
            StatusCode::UNAUTHORIZED,
            Json(TotpBootstrapResponse {
                ok: false,
                message: "invalid username or password".to_string(),
                issuer: None,
                account_name: None,
                secret: None,
                otpauth_uri: None,
            }),
        )
            .into_response(),
        Err(AuthError::InvalidOtp) => (
            StatusCode::BAD_REQUEST,
            Json(TotpBootstrapResponse {
                ok: false,
                message: "invalid request".to_string(),
                issuer: None,
                account_name: None,
                secret: None,
                otpauth_uri: None,
            }),
        )
            .into_response(),
        Err(
            AuthError::UserAlreadyExists
            | AuthError::InvalidInput
            | AuthError::WeakPassword
            | AuthError::Forbidden,
        ) => (
            StatusCode::BAD_REQUEST,
            Json(TotpBootstrapResponse {
                ok: false,
                message: "totp bootstrap failed".to_string(),
                issuer: None,
                account_name: None,
                secret: None,
                otpauth_uri: None,
            }),
        )
            .into_response(),
    }
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RegisterRequest>,
) -> Response {
    match state
        .auth_service
        .register(&request.username, &request.password)
        .await
    {
        Ok(registered) => (
            StatusCode::CREATED,
            Json(RegisterResponse {
                ok: true,
                message: "register success".to_string(),
                issuer: Some(registered.issuer),
                account_name: Some(registered.account_name),
                secret: Some(registered.secret),
                otpauth_uri: Some(registered.otpauth_uri),
            }),
        )
            .into_response(),
        Err(AuthError::UserAlreadyExists) => (
            StatusCode::CONFLICT,
            Json(RegisterResponse {
                ok: false,
                message: "username already exists".to_string(),
                issuer: None,
                account_name: None,
                secret: None,
                otpauth_uri: None,
            }),
        )
            .into_response(),
        Err(AuthError::InvalidInput) => (
            StatusCode::BAD_REQUEST,
            Json(RegisterResponse {
                ok: false,
                message: "username must be at least 3 characters".to_string(),
                issuer: None,
                account_name: None,
                secret: None,
                otpauth_uri: None,
            }),
        )
            .into_response(),
        Err(AuthError::WeakPassword) => (
            StatusCode::BAD_REQUEST,
            Json(RegisterResponse {
                ok: false,
                message: "password must be at least 8 characters".to_string(),
                issuer: None,
                account_name: None,
                secret: None,
                otpauth_uri: None,
            }),
        )
            .into_response(),
        Err(AuthError::InvalidCredentials | AuthError::InvalidOtp | AuthError::Forbidden) => (
            StatusCode::BAD_REQUEST,
            Json(RegisterResponse {
                ok: false,
                message: "register failed".to_string(),
                issuer: None,
                account_name: None,
                secret: None,
                otpauth_uri: None,
            }),
        )
            .into_response(),
    }
}

pub async fn logout(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if let Some(token) = extract_session_token(&headers) {
        state.auth_service.logout(&token).await;
    }

    let mut response =
        Json(serde_json::json!({"ok": true, "message": "logged out"})).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static("cyanrex_session=; HttpOnly; Path=/; SameSite=Lax; Max-Age=0"),
    );

    response
}

pub async fn change_password(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<ChangePasswordRequest>,
) -> Response {
    let session = match current_session_from_headers(&state, &headers).await {
        Some(s) => s,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"ok": false, "message": "missing auth session"})),
            )
                .into_response()
        }
    };

    match state.auth_service.change_password(
        &session.username,
        &request.current_password,
        &request.new_password,
        &request.otp,
    )
    .await
    {
        Ok(()) => Json(serde_json::json!({"ok": true, "message": "password changed"})).into_response(),
        Err(AuthError::InvalidCredentials) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"ok": false, "message": "invalid current password"})),
        )
            .into_response(),
        Err(AuthError::InvalidOtp) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"ok": false, "message": "invalid otp"})),
        )
            .into_response(),
        Err(AuthError::WeakPassword) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"ok": false, "message": "new password must be at least 8 characters"})),
        )
            .into_response(),
        Err(AuthError::UserAlreadyExists | AuthError::InvalidInput | AuthError::Forbidden) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"ok": false, "message": "password change failed"})),
        )
            .into_response(),
    }
}

pub async fn delete_account(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<DeleteAccountRequest>,
) -> Response {
    let session = match current_session_from_headers(&state, &headers).await {
        Some(s) => s,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"ok": false, "message": "missing auth session"})),
            )
                .into_response()
        }
    };

    match state
        .auth_service
        .delete_account(&session.username, &request.password, &request.otp)
        .await
    {
        Ok(()) => {
            let mut response =
                Json(serde_json::json!({"ok": true, "message": "account deleted"})).into_response();
            response.headers_mut().insert(
                header::SET_COOKIE,
                HeaderValue::from_static(
                    "cyanrex_session=; HttpOnly; Path=/; SameSite=Lax; Max-Age=0",
                ),
            );
            response
        }
        Err(AuthError::InvalidCredentials) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"ok": false, "message": "invalid password"})),
        )
            .into_response(),
        Err(AuthError::InvalidOtp) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"ok": false, "message": "invalid otp"})),
        )
            .into_response(),
        Err(AuthError::Forbidden) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"ok": false, "message": "cannot delete the last user"})),
        )
            .into_response(),
        Err(AuthError::WeakPassword | AuthError::UserAlreadyExists | AuthError::InvalidInput) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"ok": false, "message": "delete account failed"})),
        )
            .into_response(),
    }
}

pub async fn require_authenticated(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let token = extract_session_token(headers).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"ok": false, "message": "missing auth session"})),
        )
    })?;

    if state.auth_service.validate_session(&token).await.is_none() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"ok": false, "message": "invalid or expired auth session"})),
        ));
    }

    Ok(())
}

pub async fn current_session_from_headers(
    state: &AppState,
    headers: &HeaderMap,
) -> Option<crate::services::auth_service::SessionRecord> {
    let token = extract_session_token(headers)?;
    state.auth_service.validate_session(&token).await
}

pub async fn auth_guard(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    if let Err((status, payload)) = require_authenticated(state.as_ref(), request.headers()).await {
        return (status, payload).into_response();
    }

    next.run(request).await
}

fn build_session_cookie(token: &str, max_age_seconds: i64) -> String {
    format!(
        "{SESSION_COOKIE_NAME}={token}; HttpOnly; Path=/; SameSite=Lax; Max-Age={max_age_seconds}"
    )
}

pub fn extract_session_token(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;

    cookie_header
        .split(';')
        .filter_map(|part| {
            let mut pair = part.trim().splitn(2, '=');
            let key = pair.next()?.trim();
            let value = pair.next()?.trim();
            if key == SESSION_COOKIE_NAME {
                Some(value.to_string())
            } else {
                None
            }
        })
        .next()
}
