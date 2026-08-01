pub mod config;
pub mod models;
pub mod routes;
pub mod services;

use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc,
};

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

fn parse_frontend_port() -> u16 {
    std::env::var("CYANREX_FRONTEND_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(3000)
}

fn build_cors_origins() -> Vec<HeaderValue> {
    let mut origins: Vec<HeaderValue> = Vec::new();
    let port = parse_frontend_port();
    let bind_address =
        std::env::var("CYANREX_BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1".to_string());
    let preferred = std::env::var("CYANREX_CORS_ORIGINS").ok();

    if let Some(raw) = preferred {
        for item in raw
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
        {
            if let Ok(origin) = item.parse::<HeaderValue>() {
                origins.push(origin);
            }
        }
    }

    if origins.is_empty() {
        let localhost = format!("http://localhost:{port}");
        let bind = format!("http://{bind_address}:{port}");
        if let Ok(origin) = localhost.parse::<HeaderValue>() {
            origins.push(origin);
        }
        if bind_address != "localhost" {
            if let Ok(origin) = bind.parse::<HeaderValue>() {
                origins.push(origin);
            }
        }
    }

    if origins.is_empty() {
        origins.push(HeaderValue::from_static("http://localhost:3000"));
    }

    origins
}

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
    pub performance_metrics: Arc<PerformanceMetrics>,
}

#[derive(Debug, Default)]
pub struct PerformanceMetrics {
    check_total_requests: Arc<AtomicU64>,
    check_cache_hits: Arc<AtomicU64>,
    check_cache_misses: Arc<AtomicU64>,
    check_errors: Arc<AtomicU64>,
    check_rejected: Arc<AtomicU64>,
    check_in_flight: Arc<AtomicUsize>,
    check_in_flight_peak: Arc<AtomicUsize>,
    check_total_duration_nanos: Arc<AtomicU64>,

    completion_total_requests: Arc<AtomicU64>,
    completion_cache_hits: Arc<AtomicU64>,
    completion_cache_misses: Arc<AtomicU64>,
    completion_errors: Arc<AtomicU64>,
    completion_rejected: Arc<AtomicU64>,
    completion_in_flight: Arc<AtomicUsize>,
    completion_in_flight_peak: Arc<AtomicUsize>,
    completion_total_duration_nanos: Arc<AtomicU64>,
}

impl PerformanceMetrics {
    fn avg_duration_ms(total_duration_nanos: u64, total_requests: u64) -> f64 {
        if total_requests == 0 {
            0.0
        } else {
            total_duration_nanos as f64 / total_requests as f64 / 1_000_000.0
        }
    }

    fn start_check_request(&self) {
        self.check_total_requests.fetch_add(1, Ordering::Relaxed);
        let in_flight = self.check_in_flight.fetch_add(1, Ordering::Relaxed) + 1;
        Self::maybe_update_peak(&self.check_in_flight_peak, in_flight);
    }

    fn end_check_request(
        &self,
        duration_nanos: u64,
        cache_hit: Option<bool>,
        ok: bool,
        rejected: bool,
    ) {
        if let Some(was_hit) = cache_hit {
            if was_hit {
                self.check_cache_hits.fetch_add(1, Ordering::Relaxed);
            } else {
                self.check_cache_misses.fetch_add(1, Ordering::Relaxed);
            }
        }
        if !ok {
            self.check_errors.fetch_add(1, Ordering::Relaxed);
        }
        if rejected {
            self.check_rejected.fetch_add(1, Ordering::Relaxed);
        }
        self.check_total_duration_nanos
            .fetch_add(duration_nanos, Ordering::Relaxed);
        self.check_in_flight.fetch_sub(1, Ordering::Relaxed);
    }

    fn start_completion_request(&self) {
        self.completion_total_requests
            .fetch_add(1, Ordering::Relaxed);
        let in_flight = self.completion_in_flight.fetch_add(1, Ordering::Relaxed) + 1;
        Self::maybe_update_peak(&self.completion_in_flight_peak, in_flight);
    }

    fn end_completion_request(
        &self,
        duration_nanos: u64,
        cache_hit: Option<bool>,
        ok: bool,
        rejected: bool,
    ) {
        if let Some(was_hit) = cache_hit {
            if was_hit {
                self.completion_cache_hits.fetch_add(1, Ordering::Relaxed);
            } else {
                self.completion_cache_misses.fetch_add(1, Ordering::Relaxed);
            }
        }
        if !ok {
            self.completion_errors.fetch_add(1, Ordering::Relaxed);
        }
        if rejected {
            self.completion_rejected.fetch_add(1, Ordering::Relaxed);
        }
        self.completion_total_duration_nanos
            .fetch_add(duration_nanos, Ordering::Relaxed);
        self.completion_in_flight.fetch_sub(1, Ordering::Relaxed);
    }

    fn maybe_update_peak(peak: &AtomicUsize, current: usize) {
        let mut previous = peak.load(Ordering::Relaxed);
        while current > previous {
            match peak.compare_exchange_weak(
                previous,
                current,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(next) => previous = next,
            }
        }
    }
}

impl AppState {
    pub fn record_check_request(&self) {
        self.performance_metrics.start_check_request();
    }

    pub fn finish_check_request(
        &self,
        duration_nanos: u64,
        cache_hit: Option<bool>,
        ok: bool,
        rejected: bool,
    ) {
        self.performance_metrics
            .end_check_request(duration_nanos, cache_hit, ok, rejected);
    }

    pub fn record_completion_request(&self) {
        self.performance_metrics.start_completion_request();
    }

    pub fn finish_completion_request(
        &self,
        duration_nanos: u64,
        cache_hit: Option<bool>,
        ok: bool,
        rejected: bool,
    ) {
        self.performance_metrics
            .end_completion_request(duration_nanos, cache_hit, ok, rejected);
    }

    pub fn performance_snapshot(&self) -> crate::models::settings::PerformanceMetricsResponse {
        let check_total_requests = self
            .performance_metrics
            .check_total_requests
            .load(Ordering::Relaxed);
        let check_total_duration_nanos = self
            .performance_metrics
            .check_total_duration_nanos
            .load(Ordering::Relaxed);
        let completion_total_requests = self
            .performance_metrics
            .completion_total_requests
            .load(Ordering::Relaxed);
        let completion_total_duration_nanos = self
            .performance_metrics
            .completion_total_duration_nanos
            .load(Ordering::Relaxed);

        crate::models::settings::PerformanceMetricsResponse {
            check: crate::models::settings::CompilerOperationMetricsResponse {
                total_requests: self
                    .performance_metrics
                    .check_total_requests
                    .load(Ordering::Relaxed),
                cache_hits: self
                    .performance_metrics
                    .check_cache_hits
                    .load(Ordering::Relaxed),
                cache_misses: self
                    .performance_metrics
                    .check_cache_misses
                    .load(Ordering::Relaxed),
                errors: self
                    .performance_metrics
                    .check_errors
                    .load(Ordering::Relaxed),
                rejected: self
                    .performance_metrics
                    .check_rejected
                    .load(Ordering::Relaxed),
                in_flight: self
                    .performance_metrics
                    .check_in_flight
                    .load(Ordering::Relaxed) as u64,
                in_flight_peak: self
                    .performance_metrics
                    .check_in_flight_peak
                    .load(Ordering::Relaxed) as u64,
                avg_duration_ms: PerformanceMetrics::avg_duration_ms(
                    check_total_duration_nanos,
                    check_total_requests,
                ),
            },
            completion: crate::models::settings::CompilerOperationMetricsResponse {
                total_requests: self
                    .performance_metrics
                    .completion_total_requests
                    .load(Ordering::Relaxed),
                cache_hits: self
                    .performance_metrics
                    .completion_cache_hits
                    .load(Ordering::Relaxed),
                cache_misses: self
                    .performance_metrics
                    .completion_cache_misses
                    .load(Ordering::Relaxed),
                errors: self
                    .performance_metrics
                    .completion_errors
                    .load(Ordering::Relaxed),
                rejected: self
                    .performance_metrics
                    .completion_rejected
                    .load(Ordering::Relaxed),
                in_flight: self
                    .performance_metrics
                    .completion_in_flight
                    .load(Ordering::Relaxed) as u64,
                in_flight_peak: self
                    .performance_metrics
                    .completion_in_flight_peak
                    .load(Ordering::Relaxed) as u64,
                avg_duration_ms: PerformanceMetrics::avg_duration_ms(
                    completion_total_duration_nanos,
                    completion_total_requests,
                ),
            },
        }
    }
}

pub fn build_state() -> Arc<AppState> {
    let auth_service = AuthService::new_with_default_admin();
    let event_bus = EventBus::new(1024);
    let module_manager = ModuleManager::default();
    let command_dispatcher = CommandDispatcher::new(module_manager.clone());
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
        performance_metrics: Arc::new(PerformanceMetrics::default()),
    })
}

pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(build_cors_origins())
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
        .route(
            "/settings/events",
            axum::routing::get(routes::settings::get_event_settings)
                .post(routes::settings::update_event_settings),
        )
        .route("/ws/events", get(routes::events::ws_events))
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
        .route("/ebpf/run", axum::routing::post(routes::ebpf::run_ebpf))
        .route("/ebpf/check", axum::routing::post(routes::ebpf::check_ebpf))
        .route(
            "/ebpf/complete",
            axum::routing::post(routes::ebpf::complete_ebpf),
        )
        .route(
            "/ebpf/detach",
            axum::routing::post(routes::ebpf::detach_ebpf),
        )
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
            state.clone(),
            routes::auth::auth_guard,
        ));

    let module_read_only = Router::new()
        .route("/modules", get(routes::modules::list_modules))
        .route(
            "/modules/c-headers/catalog",
            get(routes::c_headers::list_headers),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            routes::auth::teacher_or_admin_guard,
        ));

    let admin_only = Router::new()
        .route(
            "/settings/compiler",
            get(routes::settings::get_compiler_settings)
                .post(routes::settings::update_compiler_settings),
        )
        .route(
            "/settings/performance",
            get(routes::settings::get_performance_metrics),
        )
        .route(
            "/modules/start",
            axum::routing::post(routes::modules::start_module),
        )
        .route(
            "/modules/stop",
            axum::routing::post(routes::modules::stop_module),
        )
        .route(
            "/command",
            axum::routing::post(routes::command::dispatch_command),
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
        .layer(middleware::from_fn_with_state(
            state.clone(),
            routes::auth::admin_guard,
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
        .merge(module_read_only)
        .merge(admin_only)
        .layer(cors)
        .with_state(state)
}
