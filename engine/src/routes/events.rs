use std::sync::Arc;

use axum::{extract::State, Json};

use crate::{models::event::Event, AppState};

pub async fn list_events(State(state): State<Arc<AppState>>) -> Json<Vec<Event>> {
    Json(state.event_bus.snapshot())
}
