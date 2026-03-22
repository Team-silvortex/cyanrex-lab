use std::sync::Arc;

use axum::{extract::State, Json};

use crate::{models::command::CommandRequest, AppState};

pub async fn dispatch_command(
    State(state): State<Arc<AppState>>,
    Json(command): Json<CommandRequest>,
) -> Json<serde_json::Value> {
    let result = state.command_dispatcher.dispatch(command).await;
    Json(serde_json::json!({"ok": result}))
}
