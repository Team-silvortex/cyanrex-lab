use super::SessionLifecycleHttpState;
use crate::{
    platform_http::{admission, errors::HttpError, security},
    services::auth_service::durable_source::DurableAuthError,
};
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

enum Endpoint {
    Session,
    Logout,
}

fn endpoint(request: &Request) -> Result<Endpoint, HttpError> {
    let (selected, method, allow) = match request.uri().path() {
        "/platform/v1/session" => (Endpoint::Session, Method::GET, "GET"),
        "/platform/v1/session/logout" => (Endpoint::Logout, Method::POST, "POST"),
        _ => return Err(HttpError::new(StatusCode::NOT_FOUND, "not_found")),
    };
    if *request.method() != method {
        return Err(HttpError::method_not_allowed(allow));
    }
    if request.uri().query().is_some() {
        return Err(HttpError::invalid_request());
    }
    Ok(selected)
}

#[derive(Serialize)]
struct SessionResponse {
    schema_version: u8,
    authenticated: bool,
}

#[derive(Serialize)]
struct LogoutResponse {
    schema_version: u8,
    ok: bool,
}

fn source_error(_: DurableAuthError) -> HttpError {
    // A backend failure establishes neither rollback nor an absent Session. Keep all storage,
    // schema and authority diagnostics private, without a cookie-clearing fallback.
    HttpError::dispatched(StatusCode::SERVICE_UNAVAILABLE, "service_unavailable")
}

fn cleared_cookie(origin: &str) -> HeaderValue {
    // This host-only policy is frozen for a future issuer; this adapter never issues a Session.
    // The trusted constructor accepts only canonical HTTPS or explicit loopback HTTP origins.
    if origin.starts_with("https://") {
        HeaderValue::from_static(
            "cyanrex_platform_session=; Path=/platform/v1; HttpOnly; SameSite=Strict; Max-Age=0; Secure",
        )
    } else {
        HeaderValue::from_static(
            "cyanrex_platform_session=; Path=/platform/v1; HttpOnly; SameSite=Strict; Max-Age=0",
        )
    }
}

pub(super) async fn dispatch(
    State(state): State<SessionLifecycleHttpState>,
    request: Request,
) -> Response {
    let selected = match endpoint(&request) {
        Ok(selected) => selected,
        Err(error) => return error.into_response(),
    };
    let token = match security::credentials(request.headers(), request.method(), &state.origin) {
        Ok(token) => token,
        Err(error) => return error.into_response(),
    };
    let permit = match admission::acquire(&state.admission) {
        Ok(permit) => permit,
        Err(error) => return error.into_response(),
    };
    let result = handle(state, request, selected, token).await;
    let response = match result {
        Ok(response) => response,
        Err(error) => error.into_response(),
    };
    admission::response(response, permit)
}

async fn handle(
    state: SessionLifecycleHttpState,
    request: Request,
    selected: Endpoint,
    token: String,
) -> Result<Response, HttpError> {
    security::request_bytes(request, true).await?;
    match selected {
        Endpoint::Session => {
            // One selected backend operation verifies the original relations, exact account
            // incarnation and fresh expiry, and confirms its read transaction before return.
            let session = state
                .backend
                .validate_session(&token)
                .await
                .map_err(source_error)?;
            if session.is_none() {
                return Err(HttpError::dispatched(
                    StatusCode::UNAUTHORIZED,
                    "invalid_session",
                ));
            }
            Ok(Json(SessionResponse {
                schema_version: 1,
                authenticated: true,
            })
            .into_response())
        }
        Endpoint::Logout => {
            // Do not pre-validate: canonical unknown/expired tokens may confirm original-source
            // absence. Only this operation's confirmed success authorizes browser cookie removal.
            state.backend.logout(&token).await.map_err(source_error)?;
            let mut response = Json(LogoutResponse {
                schema_version: 1,
                ok: true,
            })
            .into_response();
            response
                .headers_mut()
                .insert(header::SET_COOKIE, cleared_cookie(&state.origin));
            Ok(response)
        }
    }
}
