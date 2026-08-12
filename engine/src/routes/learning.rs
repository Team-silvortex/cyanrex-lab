use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use crate::{
    models::{auth::AuthRole, learning::TeacherStudentAttempts},
    AppState,
};

#[derive(Deserialize)]
pub struct TeacherAttemptsQuery {
    username: String,
    limit: Option<usize>,
}

pub async fn list_labs(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let Some(session) =
        crate::routes::auth::current_session_from_headers(state.as_ref(), &headers).await
    else {
        return auth_error().into_response();
    };
    Json(
        state
            .learning_store
            .progress_for_user(&session.username)
            .await,
    )
    .into_response()
}

pub async fn list_attempts(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let Some(session) =
        crate::routes::auth::current_session_from_headers(state.as_ref(), &headers).await
    else {
        return auth_error().into_response();
    };
    Json(
        state
            .learning_store
            .attempts_for_user(&session.username)
            .await,
    )
    .into_response()
}

pub async fn teacher_overview(State(state): State<Arc<AppState>>) -> Response {
    let mut overview = state.learning_store.teacher_overview().await;
    overview.students.retain(|student| {
        matches!(
            state.auth_service.role_for_username(&student.username),
            AuthRole::Student
        )
    });
    overview.active_students = overview.students.len() as u32;
    Json(overview).into_response()
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
    Json(TeacherStudentAttempts {
        username: username.to_string(),
        attempts,
    })
    .into_response()
}

fn valid_username(username: &str) -> bool {
    !username.is_empty()
        && username.len() <= 64
        && username.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
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
