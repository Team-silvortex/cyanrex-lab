pub mod config;
pub mod models;
pub mod routes;
pub mod services;

use std::sync::Arc;

use axum::{http::HeaderValue, routing::get, Router};
use services::{
    c_header_module::CHeaderModule, command_dispatcher::CommandDispatcher, ebpf_loader::EbpfLoader,
    environment_checker::EnvironmentChecker, event_bus::EventBus, module_manager::ModuleManager,
};
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
pub struct AppState {
    pub module_manager: ModuleManager,
    pub event_bus: EventBus,
    pub command_dispatcher: CommandDispatcher,
    pub ebpf_loader: EbpfLoader,
    pub environment_checker: EnvironmentChecker,
    pub c_header_module: CHeaderModule,
}

pub fn build_state() -> Arc<AppState> {
    let event_bus = EventBus::new(1024);
    let module_manager = ModuleManager::default();
    let command_dispatcher = CommandDispatcher::new(module_manager.clone(), event_bus.clone());
    let ebpf_loader = EbpfLoader;
    let environment_checker = EnvironmentChecker;
    let c_header_module = CHeaderModule::default();

    Arc::new(AppState {
        module_manager,
        event_bus,
        command_dispatcher,
        ebpf_loader,
        environment_checker,
        c_header_module,
    })
}

pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(HeaderValue::from_static("http://localhost:3000"))
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/", get(routes::index::index))
        .route("/health", get(routes::health::health))
        .route("/modules", get(routes::modules::list_modules))
        .route(
            "/modules/start",
            axum::routing::post(routes::modules::start_module),
        )
        .route(
            "/modules/stop",
            axum::routing::post(routes::modules::stop_module),
        )
        .route("/events", get(routes::events::list_events))
        .route(
            "/command",
            axum::routing::post(routes::command::dispatch_command),
        )
        .route("/ebpf/run", axum::routing::post(routes::ebpf::run_ebpf))
        .route(
            "/helper/environment",
            axum::routing::get(routes::helper::environment_report),
        )
        .route(
            "/modules/c-headers/catalog",
            axum::routing::get(routes::c_headers::list_headers),
        )
        .route(
            "/modules/c-headers/download",
            axum::routing::post(routes::c_headers::download_header),
        )
        .route(
            "/modules/c-headers/delete",
            axum::routing::post(routes::c_headers::delete_header),
        )
        .route(
            "/modules/c-headers/select",
            axum::routing::post(routes::c_headers::select_header),
        )
        .route(
            "/modules/c-headers/selected-metadata",
            axum::routing::get(routes::c_headers::selected_metadata),
        )
        .layer(cors)
        .with_state(state)
}
