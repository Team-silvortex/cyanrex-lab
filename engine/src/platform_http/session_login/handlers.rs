use super::{headers, SessionLoginHttpState};
use crate::{
    models::collaboration::{CoreSchemaVersion, LegacyUsername},
    platform_http::{admission, contract, errors::HttpError, security},
    services::auth_service::durable_source::DurableAuthError,
};
use axum::{
    extract::{Request, State},
    http::{header, Method, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

const MAX_REQUEST_BYTES: usize = 32 * 1024;

fn endpoint(request: &Request) -> Result<(), HttpError> {
    if request.uri().path() != "/platform/v1/session/login" {
        return Err(HttpError::new(StatusCode::NOT_FOUND, "not_found"));
    }
    if request.method() != Method::POST {
        return Err(HttpError::method_not_allowed("POST"));
    }
    if request.uri().query().is_some() {
        return Err(HttpError::invalid_request());
    }
    Ok(())
}

// No Debug/Serialize: raw credentials are private to this one admitted request.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginRequest {
    schema_version: CoreSchemaVersion,
    username: LegacyUsername,
    password: String,
    otp: String,
}

#[derive(Serialize)]
struct LoginResponse {
    schema_version: u8,
    authenticated: bool,
}

fn source_error(error: DurableAuthError) -> HttpError {
    let (status, code) = match error {
        DurableAuthError::InvalidCredentials => (StatusCode::UNAUTHORIZED, "invalid_credentials"),
        DurableAuthError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "rate_limited"),
        _ => (StatusCode::SERVICE_UNAVAILABLE, "service_unavailable"),
    };
    // Once dispatched, errors never establish rollback, absent Session or safe retry. Do not
    // expose account/storage details, clear another cookie, or automatically undo an unknown write.
    HttpError::dispatched(status, code)
}

pub(super) async fn dispatch(
    State(state): State<SessionLoginHttpState>,
    request: Request,
) -> Response {
    if let Err(error) = endpoint(&request)
        .and_then(|_| headers::preflight(request.headers(), &state.origin))
        .and_then(|_| state.rate.admit())
    {
        return error.into_response();
    }
    let permit = match admission::acquire(&state.admission) {
        Ok(permit) => permit,
        Err(error) => return error.into_response(),
    };
    let response = match handle(&state, request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    };
    admission::response(response, permit)
}

async fn handle(state: &SessionLoginHttpState, request: Request) -> Result<Response, HttpError> {
    // The two-second body budget ends before dispatch; do not add a handler timeout around a
    // possible confirmed COMMIT or its independent graceful connection close.
    let bytes = security::bounded_request_bytes(request, false, MAX_REQUEST_BYTES).await?;
    let input: LoginRequest = contract::parse(&bytes)?;
    if input.password.len() > 4096 || input.otp.len() > 64 {
        return Err(HttpError::invalid_request());
    }
    let _ = input.schema_version;
    let login = state
        .backend
        .login(&input.username, &input.password, &input.otp)
        .await
        .map_err(source_error)?;
    // The issuer alone confirms the database command. No later SQL/FS/validation, secondary
    // issuer or response serialization of DurableLogin may turn this into another operation.
    let cookie = headers::issued_cookie(&login.token, &state.origin)?;
    let mut response = Json(LoginResponse {
        schema_version: 1,
        authenticated: true,
    })
    .into_response();
    response.headers_mut().insert(header::SET_COOKIE, cookie);
    Ok(response)
}
