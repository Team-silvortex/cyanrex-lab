use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::{models::auth::AuthRole, AppState};

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

fn auth_error() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({
            "ok": false,
            "message": "invalid auth session",
        })),
    )
}
