use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    models::{
        auth::AuthRole,
        learning::{SaveTeacherFeedbackRequest, TeacherStudentAttempts},
    },
    services::learning_store::LearningFeedbackError,
    AppState,
};

#[derive(Deserialize)]
pub struct TeacherAttemptsQuery {
    username: String,
    limit: Option<usize>,
}

#[derive(Deserialize)]
pub struct AttemptQuery {
    attempt_id: String,
}

pub async fn get_attempt(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<AttemptQuery>,
) -> Response {
    let Some(session) =
        crate::routes::auth::current_session_from_headers(state.as_ref(), &headers).await
    else {
        return auth_error().into_response();
    };
    let response = if uuid::Uuid::parse_str(&query.attempt_id).is_err() {
        learning_error(StatusCode::BAD_REQUEST, "invalid attempt id")
    } else {
        match state
            .learning_store
            .attempt_for_user(&session.username, &query.attempt_id)
            .await
        {
            Ok(Some(attempt)) => Json(attempt).into_response(),
            Ok(None) => learning_error(StatusCode::NOT_FOUND, "attempt not found"),
            Err(_) => {
                tracing::warn!("failed to load learning attempt");
                learning_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to load attempt")
            }
        }
    };
    ([("cache-control", "no-store")], response).into_response()
}

pub async fn list_labs(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let Some(session) =
        crate::routes::auth::current_session_from_headers(state.as_ref(), &headers).await
    else {
        return auth_error().into_response();
    };
    learning_read_response(
        state
            .learning_store
            .progress_for_user(&session.username)
            .await,
    )
}

pub async fn list_attempts(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let Some(session) =
        crate::routes::auth::current_session_from_headers(state.as_ref(), &headers).await
    else {
        return auth_error().into_response();
    };
    learning_read_response(
        state
            .learning_store
            .attempts_for_user(&session.username)
            .await,
    )
}

pub async fn teacher_overview(State(state): State<Arc<AppState>>) -> Response {
    let overview = state
        .learning_store
        .teacher_overview()
        .await
        .map(|mut overview| {
            overview.students.retain(|student| {
                matches!(
                    state.auth_service.role_for_username(&student.username),
                    AuthRole::Student
                )
            });
            overview.active_students = overview.students.len() as u32;
            overview
        });
    learning_read_response(overview)
}

pub async fn teacher_attempts(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TeacherAttemptsQuery>,
) -> Response {
    let username = query.username.trim();
    if !valid_username(username)
        || !matches!(
            state.auth_service.role_for_username(username),
            AuthRole::Student
        )
    {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"ok": false, "message": "student not found"})),
        )
            .into_response();
    }
    let attempts = state
        .learning_store
        .recent_attempts_for_user(username, query.limit.unwrap_or(20))
        .await;
    learning_read_response(attempts.map(|attempts| TeacherStudentAttempts {
        username: username.to_string(),
        attempts,
    }))
}

fn learning_read_response<T: Serialize>(result: Result<T, String>) -> Response {
    let response = match result {
        Ok(value) => Json(value).into_response(),
        Err(_) => {
            // Storage/parser errors can contain paths or stored values; never expose them here.
            tracing::warn!("failed to load learning records");
            learning_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to load learning records",
            )
        }
    };
    ([("cache-control", "no-store")], response).into_response()
}

fn valid_username(username: &str) -> bool {
    !username.is_empty()
        && username.len() <= 64
        && username.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
}

pub async fn save_teacher_feedback(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<SaveTeacherFeedbackRequest>,
) -> Response {
    let Some(session) =
        crate::routes::auth::current_session_from_headers(state.as_ref(), &headers).await
    else {
        return auth_error().into_response();
    };
    if !valid_username(&request.username)
        || !matches!(
            state.auth_service.role_for_username(&request.username),
            AuthRole::Student
        )
    {
        return learning_error(StatusCode::NOT_FOUND, "student not found");
    }
    match state
        .learning_store
        .save_teacher_feedback(&session.username, &request)
        .await
    {
        Ok(feedback) => Json(feedback).into_response(),
        Err(LearningFeedbackError::InvalidInput(message)) => {
            learning_error(StatusCode::BAD_REQUEST, message)
        }
        Err(LearningFeedbackError::NotFound) => {
            learning_error(StatusCode::NOT_FOUND, "student attempt not found")
        }
        Err(LearningFeedbackError::Conflict) => learning_error(
            StatusCode::CONFLICT,
            "feedback changed; reload before editing",
        ),
        Err(LearningFeedbackError::Storage(error)) => {
            tracing::warn!("failed to save teacher feedback: {error}");
            learning_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to save teacher feedback",
            )
        }
    }
}

fn learning_error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({"ok": false, "message": message})),
    )
        .into_response()
}

fn auth_error() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({
            "ok": false,
            "message": "invalid auth session",
        })),
    )
}
