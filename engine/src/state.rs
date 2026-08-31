use std::sync::Arc;

use crate::{
    metrics::PerformanceMetrics,
    models::settings::PerformanceMetricsResponse,
    services::{
        auth_service::AuthService, c_header_module::CHeaderModule,
        command_dispatcher::CommandDispatcher, ebpf_loader::EbpfLoader,
        environment_checker::EnvironmentChecker, event_bus::EventBus,
        learning_store::LearningStore, module_manager::ModuleManager,
        runner_agent_authenticator::RunnerAgentAuthenticator,
        runner_agent_registry::RunnerAgentRegistry, runner_job_queue::RunnerJobQueue,
        runner_manager::RunnerManager, script_store::ScriptStore,
    },
};

#[derive(Clone)]
pub struct AppState {
    pub auth_service: AuthService,
    pub module_manager: ModuleManager,
    pub event_bus: EventBus,
    pub command_dispatcher: CommandDispatcher,
    pub ebpf_loader: EbpfLoader,
    pub script_store: ScriptStore,
    pub learning_store: LearningStore,
    pub runner_agent_authenticator: RunnerAgentAuthenticator,
    pub runner_agent_registry: RunnerAgentRegistry,
    pub runner_job_queue: RunnerJobQueue,
    pub runner_manager: RunnerManager,
    pub environment_checker: EnvironmentChecker,
    pub c_header_module: CHeaderModule,
    performance_metrics: Arc<PerformanceMetrics>,
}

pub fn build_state() -> Arc<AppState> {
    let module_manager = ModuleManager::default();
    let ebpf_loader = EbpfLoader::default();
    let runner_manager = RunnerManager::from_env(ebpf_loader.clone())
        .unwrap_or_else(|error| panic!("invalid Runner configuration: {error}"));
    let runner_agent_registry = RunnerAgentRegistry::from_env()
        .unwrap_or_else(|error| panic!("invalid Runner Agent configuration: {error}"));
    let runner_agent_authenticator = RunnerAgentAuthenticator::from_env();

    Arc::new(AppState {
        auth_service: AuthService::new_with_default_admin(),
        command_dispatcher: CommandDispatcher::new(module_manager.clone()),
        module_manager,
        event_bus: EventBus::new(1024),
        ebpf_loader,
        script_store: ScriptStore::default(),
        learning_store: LearningStore::default(),
        runner_agent_authenticator,
        runner_agent_registry,
        runner_job_queue: RunnerJobQueue::default(),
        runner_manager,
        environment_checker: EnvironmentChecker,
        c_header_module: CHeaderModule::default(),
        performance_metrics: Arc::new(PerformanceMetrics::default()),
    })
}

impl AppState {
    pub fn record_check_request(&self) {
        self.performance_metrics.start_check();
    }

    pub fn finish_check_request(
        &self,
        duration_nanos: u64,
        cache_hit: Option<bool>,
        ok: bool,
        rejected: bool,
    ) {
        self.performance_metrics
            .finish_check(duration_nanos, cache_hit, ok, rejected);
    }

    pub fn record_completion_request(&self) {
        self.performance_metrics.start_completion();
    }

    pub fn finish_completion_request(
        &self,
        duration_nanos: u64,
        cache_hit: Option<bool>,
        ok: bool,
        rejected: bool,
    ) {
        self.performance_metrics
            .finish_completion(duration_nanos, cache_hit, ok, rejected);
    }

    pub fn performance_snapshot(&self) -> PerformanceMetricsResponse {
        self.performance_metrics.snapshot()
    }
}
