pub mod config;
pub mod models;
pub mod routes;
pub mod services;

use std::sync::Arc;

use axum::{
    http::{header, HeaderValue, Method},
    middleware,
    routing::get,
    Router,
};
use services::{
    auth_service::AuthService, c_header_module::CHeaderModule,
    command_dispatcher::CommandDispatcher, ebpf_loader::EbpfLoader,
    environment_checker::EnvironmentChecker, event_bus::EventBus, module_manager::ModuleManager,
    script_store::ScriptStore,
};
use tower_http::cors::CorsLayer;

#[derive(Clone)]
pub struct AppState {
    pub auth_service: AuthService,
    pub module_manager: ModuleManager,
    pub event_bus: EventBus,
    pub command_dispatcher: CommandDispatcher,
    pub ebpf_loader: EbpfLoader,
    pub script_store: ScriptStore,
    pub environment_checker: EnvironmentChecker,
    pub c_header_module: CHeaderModule,
}

pub fn build_state() -> Arc<AppState> {
    let auth_service = AuthService::new_with_default_admin();
    let event_bus = EventBus::new(1024);
    let module_manager = ModuleManager::default();
    let command_dispatcher = CommandDispatcher::new(module_manager.clone(), event_bus.clone());
    let ebpf_loader = EbpfLoader::default();
    let script_store = ScriptStore::default();
    let environment_checker = EnvironmentChecker;
    let c_header_module = CHeaderModule::default();

    Arc::new(AppState {
        auth_service,
        module_manager,
        event_bus,
        command_dispatcher,
        ebpf_loader,
        script_store,
        environment_checker,
        c_header_module,
    })
}

pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(HeaderValue::from_static("http://localhost:3000"))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::COOKIE])
        .allow_credentials(true);

    let protected = Router::new()
        .route(
            "/auth/password/change",
            axum::routing::post(routes::auth::change_password),
        )
        .route(
            "/auth/delete",
            axum::routing::post(routes::auth::delete_account),
        )
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
        .route("/events/export", get(routes::events::export_events))
        .route("/events/unread-count", get(routes::events::unread_count))
        .route(
            "/events/mark-read",
            axum::routing::post(routes::events::mark_read),
        )
        .route(
            "/events/delete",
            axum::routing::post(routes::events::delete_events),
        )
        .route("/ws/events", get(routes::events::ws_events))
        .route(
            "/command",
            axum::routing::post(routes::command::dispatch_command),
        )
        .route("/ebpf/run", axum::routing::post(routes::ebpf::run_ebpf))
        .route(
            "/ebpf/detach",
            axum::routing::post(routes::ebpf::detach_ebpf),
        )
        .route(
            "/ebpf/attachments",
            axum::routing::get(routes::ebpf::list_attachments),
        )
        .route(
            "/ebpf/attachments/details",
            axum::routing::get(routes::ebpf::list_attachment_details),
        )
        .route(
            "/ebpf/templates",
            axum::routing::get(routes::ebpf::list_templates),
        )
        .route(
            "/helper/environment",
            axum::routing::get(routes::helper::environment_report),
        )
        .route(
            "/scripts",
            axum::routing::get(routes::scripts::list_scripts),
        )
        .route(
            "/scripts/save",
            axum::routing::post(routes::scripts::save_script),
        )
        .route(
            "/scripts/delete",
            axum::routing::post(routes::scripts::delete_script),
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
        .layer(middleware::from_fn_with_state(
            state.clone(),
            routes::auth::auth_guard,
        ));

    Router::new()
        .route("/", get(routes::index::index))
        .route("/health", get(routes::health::health))
        .route("/auth/login", axum::routing::post(routes::auth::login))
        .route(
            "/auth/totp/bootstrap",
            axum::routing::post(routes::auth::bootstrap_totp),
        )
        .route(
            "/auth/register",
            axum::routing::post(routes::auth::register),
        )
        .route("/auth/me", get(routes::auth::me))
        .route("/auth/logout", axum::routing::post(routes::auth::logout))
        .merge(protected)
        .layer(cors)
        .with_state(state)
}
