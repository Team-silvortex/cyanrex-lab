use std::sync::Arc;

use axum::{
    http::{header, HeaderValue, Method},
    middleware,
    routing::{get, post},
    Router,
};
use tower_http::cors::CorsLayer;

use crate::{routes, AppState};

pub fn build_allowed_origins() -> Vec<String> {
    let frontend_port = std::env::var("CYANREX_FRONTEND_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(3000);
    let bind_address =
        std::env::var("CYANREX_BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1".to_string());

    let configured = std::env::var("CYANREX_CORS_ORIGINS")
        .ok()
        .into_iter()
        .flat_map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|origin| origin.parse::<HeaderValue>().is_ok())
        .collect::<Vec<_>>();

    let origins = if configured.is_empty() {
        let mut defaults = vec![format!("http://localhost:{frontend_port}")];
        if bind_address != "localhost" {
            defaults.push(format!("http://{bind_address}:{frontend_port}"));
        }
        defaults
    } else {
        configured
    };

    origins
        .into_iter()
        .map(|origin| origin.trim_end_matches('/').to_string())
        .filter(|origin| !origin.is_empty())
        .collect()
}

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .merge(public_routes())
        .merge(csrf_protected_public_routes(state.clone()))
        .merge(authenticated_routes(state.clone()))
        .merge(staff_routes(state.clone()))
        .merge(admin_routes(state.clone()))
        .layer(cors_layer())
        .with_state(state)
}

fn public_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(routes::index::index))
        .route("/health", get(routes::health::health))
        .route("/auth/login", post(routes::auth::login))
        .route("/auth/totp/bootstrap", post(routes::auth::bootstrap_totp))
        .route("/auth/register", post(routes::auth::register))
        .route("/auth/me", get(routes::auth::me))
}

fn csrf_protected_public_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/auth/logout", post(routes::auth::logout))
        .layer(middleware::from_fn_with_state(
            state,
            routes::auth::csrf_guard,
        ))
}

fn authenticated_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/auth/password/change", post(routes::auth::change_password))
        .route("/auth/delete", post(routes::auth::delete_account))
        .route("/events", get(routes::events::list_events))
        .route("/events/export", get(routes::events::export_events))
        .route("/events/unread-count", get(routes::events::unread_count))
        .route("/events/mark-read", post(routes::events::mark_read))
        .route("/events/delete", post(routes::events::delete_events))
        .route(
            "/settings/events",
            get(routes::settings::get_event_settings).post(routes::settings::update_event_settings),
        )
        .route("/ws/events", get(routes::events::ws_events))
        .route(
            "/helper/environment",
            get(routes::helper::environment_report),
        )
        .route("/scripts", get(routes::scripts::list_scripts))
        .route("/scripts/save", post(routes::scripts::save_script))
        .route("/scripts/delete", post(routes::scripts::delete_script))
        .route("/ebpf/run", post(routes::ebpf::run_ebpf))
        .route("/ebpf/check", post(routes::ebpf::check_ebpf))
        .route("/ebpf/complete", post(routes::ebpf::complete_ebpf))
        .route("/ebpf/detach", post(routes::ebpf::detach_ebpf))
        .route("/ebpf/attachments", get(routes::ebpf::list_attachments))
        .route(
            "/ebpf/attachments/details",
            get(routes::ebpf::list_attachment_details),
        )
        .route("/ebpf/templates", get(routes::ebpf::list_templates))
        .route(
            "/modules/c-headers/selected-metadata",
            get(routes::c_headers::selected_metadata),
        )
        .layer(middleware::from_fn_with_state(
            state,
            routes::auth::auth_and_csrf_guard,
        ))
}

fn staff_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/modules", get(routes::modules::list_modules))
        .route(
            "/modules/c-headers/catalog",
            get(routes::c_headers::list_headers),
        )
        .layer(middleware::from_fn_with_state(
            state,
            routes::auth::teacher_or_admin_and_csrf_guard,
        ))
}

fn admin_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/settings/compiler",
            get(routes::settings::get_compiler_settings)
                .post(routes::settings::update_compiler_settings),
        )
        .route(
            "/settings/performance",
            get(routes::settings::get_performance_metrics),
        )
        .route("/modules/start", post(routes::modules::start_module))
        .route("/modules/stop", post(routes::modules::stop_module))
        .route("/command", post(routes::command::dispatch_command))
        .route(
            "/modules/c-headers/download",
            post(routes::c_headers::download_header),
        )
        .route(
            "/modules/c-headers/delete",
            post(routes::c_headers::delete_header),
        )
        .route(
            "/modules/c-headers/select",
            post(routes::c_headers::select_header),
        )
        .layer(middleware::from_fn_with_state(
            state,
            routes::auth::admin_and_csrf_guard,
        ))
}

fn cors_layer() -> CorsLayer {
    let origins = build_allowed_origins()
        .into_iter()
        .filter_map(|origin| origin.parse::<HeaderValue>().ok())
        .collect::<Vec<_>>();

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::COOKIE])
        .allow_credentials(true)
}
