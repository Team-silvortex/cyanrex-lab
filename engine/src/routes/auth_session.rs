use crate::{
    models::auth::AuthRole,
    services::auth_service::SessionRecord,
};

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
) -> Option<SessionRecord> {
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

pub async fn admin_guard(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let Some(session) = current_session_from_headers(state.as_ref(), request.headers()).await else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"ok": false, "message": "invalid auth session"})),
        )
            .into_response();
    };
    if !matches!(
        state.auth_service.role_for_username(&session.username),
        AuthRole::Admin
    ) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"ok": false, "message": "administrator access required"})),
        )
            .into_response();
    }
    next.run(request).await
}

pub async fn teacher_or_admin_guard(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let Some(session) = current_session_from_headers(state.as_ref(), request.headers()).await else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"ok": false, "message": "invalid auth session"})),
        )
            .into_response();
    };

    if !matches!(
        state.auth_service.role_for_username(&session.username),
        AuthRole::Admin | AuthRole::Teacher
    ) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"ok": false, "message": "insufficient module privileges"})),
        )
            .into_response();
    }

    next.run(request).await
}

pub async fn csrf_guard(
    State(_state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    if is_csrf_safe_method(request.method()) {
        return next.run(request).await;
    }

    let allow_missing_origin = env_flag("CYANREX_ALLOW_MISSING_ORIGIN");

    match extract_origin_from_request(request.headers()) {
        None if !allow_missing_origin => (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "ok": false,
                "message": "request origin is required",
            })),
        )
            .into_response(),
        Some(origin) if !is_origin_allowed(&origin, &crate::build_allowed_origins()) => (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "ok": false,
                "message": "request rejected by CSRF policy",
            })),
        )
            .into_response(),
        _ => next.run(request).await,
    }
}

pub async fn auth_and_csrf_guard(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    if let Err((status, payload)) = require_authenticated(state.as_ref(), request.headers()).await {
        return (status, payload).into_response();
    }

    csrf_guard(State(state), request, next).await
}

pub async fn admin_and_csrf_guard(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let Some(session) = current_session_from_headers(state.as_ref(), request.headers()).await else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"ok": false, "message": "invalid auth session"})),
        )
            .into_response();
    };

    if !matches!(
        state.auth_service.role_for_username(&session.username),
        AuthRole::Admin
    ) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"ok": false, "message": "administrator access required"})),
        )
            .into_response();
    }

    csrf_guard(State(state), request, next).await
}

pub async fn teacher_or_admin_and_csrf_guard(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let Some(session) = current_session_from_headers(state.as_ref(), request.headers()).await else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"ok": false, "message": "invalid auth session"})),
        )
            .into_response();
    };

    if !matches!(
        state.auth_service.role_for_username(&session.username),
        AuthRole::Admin | AuthRole::Teacher
    ) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"ok": false, "message": "insufficient module privileges"})),
        )
            .into_response();
    }

    csrf_guard(State(state), request, next).await
}

fn is_csrf_safe_method(method: &Method) -> bool {
    matches!(method, &Method::GET | &Method::HEAD | &Method::OPTIONS)
}

fn extract_origin_from_request(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::ORIGIN)
        .and_then(|origin| {
            let value = origin.to_str().ok()?.trim();
            if value.is_empty() {
                None
            } else {
                Some(normalize_origin(value))
            }
        })
        .or_else(|| headers.get(header::REFERER).and_then(|referer| {
            let value = referer.to_str().ok()?.trim();
            parse_origin_from_referer(value)
        }))
}

fn parse_origin_from_referer(value: &str) -> Option<String> {
    let (scheme, rest) = value.split_once("://")?;
    let host_and_port = rest.split('/').next().unwrap_or("");
    if host_and_port.is_empty() {
        return None;
    }
    Some(normalize_origin(&format!("{scheme}://{host_and_port}")))
}

fn is_origin_allowed(origin: &str, allowed_origins: &[String]) -> bool {
    let normalized_origin = normalize_origin(origin);
    allowed_origins.iter().any(|allowed| normalize_origin(allowed) == normalized_origin)
}

fn normalize_origin(value: &str) -> String {
    value.trim().trim_end_matches('/').to_ascii_lowercase()
}

pub(crate) fn build_session_cookie(token: &str, max_age_seconds: i64) -> String {
    let mut value = format!(
        "{SESSION_COOKIE_NAME}={token}; Path=/; SameSite=Strict; Max-Age={max_age_seconds}; HttpOnly"
    );
    if env_flag("CYANREX_SECURE_COOKIES") {
        value.push_str("; Secure");
    }
    value
}

pub(crate) fn clear_session_cookie() -> String {
    build_session_cookie("", 0)
}

pub(crate) fn env_flag(name: &str) -> bool {
    std::env::var(name).ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
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
