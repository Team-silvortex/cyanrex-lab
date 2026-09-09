use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::{
    models::{
        auth::RegisterResponse,
        classroom::{
            ClassroomInvitations, ClassroomInviteRequest, ClassroomJoinRequest,
            ClassroomRevokeRequest,
        },
    },
    services::{auth_service::AuthError, classroom::ClassroomError},
    AppState,
};

pub async fn no_store(request: Request, next: Next) -> Response {
    let private = request.uri().path().starts_with("/classroom/")
        || request.uri().path() == "/.well-known/cyanrex-classroom";
    let mut response = next.run(request).await;
    if private {
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response.headers_mut().insert(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        );
    }
    response
}

pub async fn discovery(State(state): State<Arc<AppState>>) -> Response {
    match state.classroom.discovery() {
        Ok(discovery) => Json(discovery).into_response(),
        Err(_) => failure(StatusCode::NOT_FOUND, "classroom discovery is disabled"),
    }
}

pub async fn invitations(State(state): State<Arc<AppState>>) -> Response {
    match state.classroom.inventory() {
        Ok(invitations) => Json(ClassroomInvitations { invitations }).into_response(),
        Err(error) => classroom_error(error),
    }
}

pub async fn issue(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ClassroomInviteRequest>,
) -> Response {
    if state.auth_service.is_teacher_username(&request.username) {
        return failure(
            StatusCode::FORBIDDEN,
            "classroom invitations are only for unreserved student names",
        );
    }
    match state.classroom.issue(&request.username) {
        Ok(invitation) => (StatusCode::CREATED, Json(invitation)).into_response(),
        Err(error) => classroom_error(error),
    }
}

pub async fn revoke(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ClassroomRevokeRequest>,
) -> Response {
    match state.classroom.revoke(&request.invite_id) {
        Ok(()) => Json(json!({"ok": true, "message": "invitation revoked; existing accounts and sessions are unchanged"})).into_response(),
        Err(error) => classroom_error(error),
    }
}

pub async fn join(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ClassroomJoinRequest>,
) -> Response {
    let username = match state.classroom.redeem(&request) {
        Ok(username) => username,
        Err(error) => return classroom_error(error),
    };
    if state.auth_service.is_teacher_username(&username) {
        return failure(
            StatusCode::FORBIDDEN,
            "classroom invitations cannot grant teacher authority",
        );
    }
    match state
        .auth_service
        .register(&username, &request.password)
        .await
    {
        Ok(registered) => (
            StatusCode::CREATED,
            Json(RegisterResponse {
                ok: true,
                message: "student account created; save TOTP and log in with password plus OTP"
                    .into(),
                issuer: Some(registered.issuer),
                account_name: Some(registered.account_name),
                secret: Some(registered.secret),
                otpauth_uri: Some(registered.otpauth_uri),
            }),
        )
            .into_response(),
        Err(AuthError::UserAlreadyExists) => failure(
            StatusCode::CONFLICT,
            "account already exists; invitation consumed, use normal login",
        ),
        Err(_) => failure(
            StatusCode::BAD_REQUEST,
            "enrollment failed; invitation consumed, ask the teacher before retrying",
        ),
    }
}

fn classroom_error(error: ClassroomError) -> Response {
    let (status, message) = match error {
        ClassroomError::Disabled => (
            StatusCode::SERVICE_UNAVAILABLE,
            "classroom joining is disabled",
        ),
        ClassroomError::InvalidInput => (StatusCode::BAD_REQUEST, "invalid classroom request"),
        ClassroomError::WrongClassroom => (
            StatusCode::CONFLICT,
            "classroom identity changed; confirm with the teacher",
        ),
        ClassroomError::Incompatible => (
            StatusCode::UPGRADE_REQUIRED,
            "incompatible join protocol or required capabilities; no automatic downgrade",
        ),
        ClassroomError::InvalidInvitation => (
            StatusCode::FORBIDDEN,
            "invitation is invalid, expired, revoked, used or bound to another username",
        ),
        ClassroomError::Capacity => (
            StatusCode::TOO_MANY_REQUESTS,
            "too many outstanding invitations",
        ),
    };
    failure(status, message)
}

fn failure(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({"ok": false, "message": message}))).into_response()
}
