use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    config::runtime_instance_id,
    models::{
        ebpf::{EbpfRunResponse, EbpfRuntimeBackend},
        runner::{RunnerLeaseView, RunnerOverview, RunnerStatus},
    },
    services::{
        ebpf_loader::EbpfLoader,
        runner_driver::{LocalProcessRunnerDriver, RunnerDriver, RunnerExecutionRequest},
    },
};

#[derive(Debug, Clone)]
pub struct RunnerConfig {
    pub max_concurrent: usize,
    pub max_per_user: usize,
    pub execution_timeout: Duration,
    pub instance_id: String,
}

impl RunnerConfig {
    pub fn from_env() -> Self {
        let max_concurrent = env_usize("CYANREX_RUNNER_MAX_CONCURRENT", 2).clamp(1, 32);
        let max_per_user = env_usize("CYANREX_RUNNER_MAX_PER_USER", 1).clamp(1, max_concurrent);
        let timeout_seconds = env_u64("CYANREX_RUNNER_TIMEOUT_SECS", 45).clamp(5, 300);
        Self {
            max_concurrent,
            max_per_user,
            execution_timeout: Duration::from_secs(timeout_seconds),
            instance_id: runtime_instance_id(),
        }
    }
}

#[derive(Clone)]
pub struct RunnerManager {
    inner: Arc<RunnerManagerInner>,
}

struct RunnerManagerInner {
    config: RunnerConfig,
    active: Mutex<HashMap<String, ActiveLease>>,
    driver: Arc<dyn RunnerDriver>,
}

#[derive(Clone)]
struct ActiveLease {
    username: String,
    runtime_backend: String,
    started_at: DateTime<Utc>,
    deadline: DateTime<Utc>,
}

pub struct RunnerLease {
    manager: RunnerManager,
    runner_id: String,
    deadline: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerAcquireError {
    GlobalCapacity,
    UserCapacity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerExecutionError {
    InvalidLease,
    Timeout,
}

impl RunnerAcquireError {
    pub fn message(self) -> &'static str {
        match self {
            Self::GlobalCapacity => "all eBPF runner slots are busy",
            Self::UserCapacity => "this user already has the maximum active eBPF jobs",
        }
    }
}

impl RunnerExecutionError {
    pub fn message(self) -> &'static str {
        match self {
            Self::InvalidLease => "runner execution lease is no longer active",
            Self::Timeout => "runner execution exceeded its lease deadline",
        }
    }
}

impl RunnerManager {
    pub fn new(config: RunnerConfig, driver: Arc<dyn RunnerDriver>) -> Self {
        Self {
            inner: Arc::new(RunnerManagerInner {
                config,
                active: Mutex::new(HashMap::new()),
                driver,
            }),
        }
    }

    pub fn local(config: RunnerConfig, loader: EbpfLoader) -> Self {
        Self::new(config, Arc::new(LocalProcessRunnerDriver::new(loader)))
    }

    pub fn from_env(loader: EbpfLoader) -> Result<Self, String> {
        let mode =
            std::env::var("CYANREX_RUNNER_MODE").unwrap_or_else(|_| "local_process".to_string());
        Self::from_mode(RunnerConfig::from_env(), loader, &mode)
    }

    pub fn from_mode(config: RunnerConfig, loader: EbpfLoader, mode: &str) -> Result<Self, String> {
        match mode.trim().to_ascii_lowercase().as_str() {
            "local" | "local_process" => Ok(Self::local(config, loader)),
            unsupported => Err(format!(
                "unsupported CYANREX_RUNNER_MODE `{unsupported}`; this build supports only `local_process`"
            )),
        }
    }

    pub fn try_acquire(
        &self,
        username: &str,
        runtime_backend: EbpfRuntimeBackend,
    ) -> Result<RunnerLease, RunnerAcquireError> {
        let mut active = self.active();
        if active.len() >= self.inner.config.max_concurrent {
            return Err(RunnerAcquireError::GlobalCapacity);
        }
        let active_for_user = active
            .values()
            .filter(|lease| lease.username == username)
            .count();
        if active_for_user >= self.inner.config.max_per_user {
            return Err(RunnerAcquireError::UserCapacity);
        }

        let runner_id = format!("runner-{}", Uuid::new_v4().simple());
        let started_at = Utc::now();
        let deadline = started_at
            + chrono::Duration::from_std(self.inner.config.execution_timeout)
                .expect("runner timeout is within chrono range");
        active.insert(
            runner_id.clone(),
            ActiveLease {
                username: username.to_string(),
                runtime_backend: backend_name(runtime_backend).to_string(),
                started_at,
                deadline,
            },
        );
        drop(active);

        Ok(RunnerLease {
            manager: self.clone(),
            runner_id,
            deadline: Instant::now() + self.inner.config.execution_timeout,
        })
    }

    pub fn status_for(&self, username: &str) -> RunnerStatus {
        let active = self.active();
        self.build_status(&active, username)
    }

    pub fn overview(&self) -> RunnerOverview {
        let active = self.active();
        let mut active_leases = active
            .iter()
            .map(|(runner_id, lease)| RunnerLeaseView {
                runner_id: runner_id.clone(),
                username: lease.username.clone(),
                runtime_backend: lease.runtime_backend.clone(),
                started_at: lease.started_at,
                deadline: lease.deadline,
            })
            .collect::<Vec<_>>();
        active_leases.sort_by_key(|lease| lease.started_at);
        RunnerOverview {
            status: self.build_status(&active, ""),
            active_leases,
        }
    }

    pub fn execution_timeout(&self) -> Duration {
        self.inner.config.execution_timeout
    }

    pub async fn execute(
        &self,
        lease: &RunnerLease,
        request: RunnerExecutionRequest<'_>,
    ) -> Result<EbpfRunResponse, RunnerExecutionError> {
        if !Arc::ptr_eq(&self.inner, &lease.manager.inner) {
            return Err(RunnerExecutionError::InvalidLease);
        }
        {
            let mut active = self.active();
            let Some(active_lease) = active.get_mut(lease.runner_id()) else {
                return Err(RunnerExecutionError::InvalidLease);
            };
            active_lease.runtime_backend = backend_name(request.runtime_backend).to_string();
        }

        let remaining = lease.remaining();
        if remaining.is_zero() {
            return Err(RunnerExecutionError::Timeout);
        }
        tokio::time::timeout(remaining, self.inner.driver.execute(request))
            .await
            .map_err(|_| RunnerExecutionError::Timeout)
    }

    fn build_status(&self, active: &HashMap<String, ActiveLease>, username: &str) -> RunnerStatus {
        let descriptor = self.inner.driver.descriptor();
        RunnerStatus {
            mode: descriptor.mode.to_string(),
            isolation: descriptor.isolation.to_string(),
            instance_id: self.inner.config.instance_id.clone(),
            max_concurrent: self.inner.config.max_concurrent,
            max_per_user: self.inner.config.max_per_user,
            active_total: active.len(),
            active_for_current_user: active
                .values()
                .filter(|lease| lease.username == username)
                .count(),
            available_slots: self
                .inner
                .config
                .max_concurrent
                .saturating_sub(active.len()),
            execution_timeout_seconds: self.inner.config.execution_timeout.as_secs(),
        }
    }

    fn active(&self) -> MutexGuard<'_, HashMap<String, ActiveLease>> {
        self.inner
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl RunnerLease {
    pub fn runner_id(&self) -> &str {
        &self.runner_id
    }

    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }
}

impl Drop for RunnerLease {
    fn drop(&mut self) {
        self.manager.active().remove(&self.runner_id);
    }
}

fn backend_name(backend: EbpfRuntimeBackend) -> &'static str {
    match backend {
        EbpfRuntimeBackend::Bpftool => "bpftool",
        EbpfRuntimeBackend::Aya => "aya",
    }
}

fn env_usize(name: &str, fallback: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

fn env_u64(name: &str, fallback: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::{RunnerAcquireError, RunnerConfig, RunnerManager};
    use crate::{
        models::ebpf::{EbpfRunResponse, EbpfRuntimeBackend},
        services::{
            ebpf_loader::EbpfLoader,
            runner_driver::{
                RunnerDriver, RunnerDriverDescriptor, RunnerExecutionFuture, RunnerExecutionRequest,
            },
        },
    };
    use std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        time::Duration,
    };

    struct RecordingDriver {
        calls: Arc<AtomicUsize>,
    }

    impl RunnerDriver for RecordingDriver {
        fn descriptor(&self) -> RunnerDriverDescriptor {
            RunnerDriverDescriptor {
                mode: "test_driver",
                isolation: "test_vm",
            }
        }

        fn execute<'a>(
            &'a self,
            _request: RunnerExecutionRequest<'a>,
        ) -> RunnerExecutionFuture<'a> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Box::pin(async { EbpfRunResponse::validation_error("recorded") })
        }
    }

    fn manager(max_concurrent: usize, max_per_user: usize) -> RunnerManager {
        RunnerManager::local(
            RunnerConfig {
                max_concurrent,
                max_per_user,
                execution_timeout: Duration::from_secs(45),
                instance_id: "test".to_string(),
            },
            EbpfLoader::default(),
        )
    }

    #[test]
    fn enforces_per_user_capacity_and_releases_on_drop() {
        let manager = manager(2, 1);
        let first = manager
            .try_acquire("alice", EbpfRuntimeBackend::Bpftool)
            .unwrap();
        assert_eq!(
            manager
                .try_acquire("alice", EbpfRuntimeBackend::Bpftool)
                .err(),
            Some(RunnerAcquireError::UserCapacity)
        );
        assert!(manager.try_acquire("bob", EbpfRuntimeBackend::Aya).is_ok());
        drop(first);
        assert!(manager
            .try_acquire("alice", EbpfRuntimeBackend::Bpftool)
            .is_ok());
    }

    #[test]
    fn reports_global_capacity_without_exposing_other_user_count() {
        let manager = manager(1, 1);
        let _lease = manager
            .try_acquire("alice", EbpfRuntimeBackend::Bpftool)
            .unwrap();
        assert_eq!(
            manager.try_acquire("bob", EbpfRuntimeBackend::Aya).err(),
            Some(RunnerAcquireError::GlobalCapacity)
        );
        let status = manager.status_for("bob");
        assert_eq!(status.active_total, 1);
        assert_eq!(status.active_for_current_user, 0);
        assert_eq!(status.available_slots, 0);
    }

    #[test]
    fn unsupported_driver_mode_fails_closed() {
        let config = RunnerConfig {
            max_concurrent: 2,
            max_per_user: 1,
            execution_timeout: Duration::from_secs(45),
            instance_id: "test".to_string(),
        };
        let error = RunnerManager::from_mode(config, EbpfLoader::default(), "remote_vm")
            .err()
            .expect("unsupported mode should fail");
        assert!(error.contains("supports only `local_process`"));
    }

    #[tokio::test]
    async fn execution_delegates_to_driver_and_updates_runtime_metadata() {
        let calls = Arc::new(AtomicUsize::new(0));
        let manager = RunnerManager::new(
            RunnerConfig {
                max_concurrent: 1,
                max_per_user: 1,
                execution_timeout: Duration::from_secs(45),
                instance_id: "test".to_string(),
            },
            Arc::new(RecordingDriver {
                calls: calls.clone(),
            }),
        );
        let lease = manager
            .try_acquire("alice", EbpfRuntimeBackend::Bpftool)
            .unwrap();
        let response = manager
            .execute(
                &lease,
                RunnerExecutionRequest {
                    owner_username: "alice",
                    code: "int main(void) { return 0; }",
                    program_name: Some("test"),
                    runtime_backend: EbpfRuntimeBackend::Aya,
                    selected_headers: &[],
                    debug_breakpoints: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(response.message, "recorded");
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        let overview = manager.overview();
        assert_eq!(overview.status.mode, "test_driver");
        assert_eq!(overview.status.isolation, "test_vm");
        assert_eq!(overview.active_leases[0].runtime_backend, "aya");
    }
}
