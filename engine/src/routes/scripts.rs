use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};

use crate::{
    models::script::{
        DeleteScriptRequest, DeleteScriptResponse, SaveScriptRequest, SaveScriptResponse,
        UserScript,
    },
    AppState,
};

pub async fn list_scripts(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Json<Vec<UserScript>> {
    let username =
        match crate::routes::auth::current_session_from_headers(state.as_ref(), &headers).await {
            Some(session) => session.username,
            None => return Json(Vec::new()),
        };

    Json(state.script_store.list_for_user(&username).await)
}

pub async fn save_script(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<SaveScriptRequest>,
) -> (StatusCode, Json<SaveScriptResponse>) {
    let username =
        match crate::routes::auth::current_session_from_headers(state.as_ref(), &headers).await {
            Some(session) => session.username,
            None => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(SaveScriptResponse {
                        ok: false,
                        message: "missing auth session".to_string(),
                        record: None,
                    }),
                )
            }
        };

    match state
        .script_store
        .save_for_user(&username, &payload.title, &payload.script)
        .await
    {
        Ok(record) => (
            StatusCode::OK,
            Json(SaveScriptResponse {
                ok: true,
                message: "script saved".to_string(),
                record: Some(record),
            }),
        ),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(SaveScriptResponse {
                ok: false,
                message: error,
                record: None,
            }),
        ),
    }
}

pub async fn delete_script(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<DeleteScriptRequest>,
) -> (StatusCode, Json<DeleteScriptResponse>) {
    let username =
        match crate::routes::auth::current_session_from_headers(state.as_ref(), &headers).await {
            Some(session) => session.username,
            None => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(DeleteScriptResponse {
                        ok: false,
                        message: "missing auth session".to_string(),
                    }),
                )
            }
        };

    match state
        .script_store
        .delete_for_user(&username, &payload.id)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(DeleteScriptResponse {
                ok: true,
                message: "script deleted".to_string(),
            }),
        ),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(DeleteScriptResponse {
                ok: false,
                message: error,
            }),
        ),
    }
}
