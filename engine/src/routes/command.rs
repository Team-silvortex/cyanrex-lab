use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};

use crate::{
    models::command::{CommandRequest, CommandResponse},
    AppState,
};

pub async fn dispatch_command(
    State(state): State<Arc<AppState>>,
    Json(command): Json<CommandRequest>,
) -> (StatusCode, Json<CommandResponse>) {
    let response = state.command_dispatcher.dispatch(command).await;
    let status = if response.ok {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(response))
}
