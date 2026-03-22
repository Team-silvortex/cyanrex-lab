pub mod config;
pub mod models;
pub mod routes;
pub mod services;

use std::sync::Arc;

use axum::{routing::get, Router};
use services::{command_dispatcher::CommandDispatcher, event_bus::EventBus, module_manager::ModuleManager};

#[derive(Clone)]
pub struct AppState {
    pub module_manager: ModuleManager,
    pub event_bus: EventBus,
    pub command_dispatcher: CommandDispatcher,
}

pub fn build_state() -> Arc<AppState> {
    let event_bus = EventBus::new(1024);
    let module_manager = ModuleManager::default();
    let command_dispatcher = CommandDispatcher::new(module_manager.clone(), event_bus.clone());

    Arc::new(AppState {
        module_manager,
        event_bus,
        command_dispatcher,
    })
}

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(routes::index::index))
        .route("/health", get(routes::health::health))
        .route("/modules", get(routes::modules::list_modules))
        .route("/modules/start", axum::routing::post(routes::modules::start_module))
        .route("/modules/stop", axum::routing::post(routes::modules::stop_module))
        .route("/events", get(routes::events::list_events))
        .route("/command", axum::routing::post(routes::command::dispatch_command))
        .with_state(state)
}
