use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::{
    config::runtime_instance_id,
    models::{
        ebpf::{
            EbpfAttachmentDetail, EbpfCheckResponse, EbpfCompletionResponse, EbpfRunResponse,
            EbpfRuntimeBackend,
        },
        runner::{RunnerLeaseView, RunnerOverview, RunnerStatus},
    },
    services::{
        ebpf_loader::EbpfLoader,
        runner_driver::{
            LocalProcessRunnerDriver, RunnerCheckRequest, RunnerCompilerOutcome,
            RunnerCompletionRequest, RunnerDetachOutcome, RunnerDetachRequest, RunnerDriver,
            RunnerExecutionRequest, RunnerOperationError,
        },
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
    check_slots: Semaphore,
    completion_slots: Semaphore,
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
                check_slots: Semaphore::new(2),
                completion_slots: Semaphore::new(3),
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
            if active_lease.username != request.owner_username {
                return Err(RunnerExecutionError::InvalidLease);
            }
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

    pub async fn list_attachments(
        &self,
        owner_username: &str,
    ) -> Result<Vec<EbpfAttachmentDetail>, RunnerOperationError> {
        tokio::time::timeout(
            self.inner.config.execution_timeout,
            self.inner.driver.list_attachments(owner_username),
        )
        .await
        .map_err(|_| RunnerOperationError::Timeout)?
    }

    pub async fn check(
        &self,
        request: RunnerCheckRequest<'_>,
    ) -> Result<RunnerCompilerOutcome<EbpfCheckResponse>, RunnerOperationError> {
        validate_compiler_owner(request.owner_username)?;
        let _permit = self
            .inner
            .check_slots
            .try_acquire()
            .map_err(|_| RunnerOperationError::Busy)?;
        tokio::time::timeout(
            self.inner
                .config
                .execution_timeout
                .min(Duration::from_secs(15)),
            self.inner.driver.check(request),
        )
        .await
        .map_err(|_| RunnerOperationError::Timeout)?
    }

    pub async fn complete(
        &self,
        request: RunnerCompletionRequest<'_>,
    ) -> Result<RunnerCompilerOutcome<EbpfCompletionResponse>, RunnerOperationError> {
        validate_compiler_owner(request.owner_username)?;
        let _permit = self
            .inner
            .completion_slots
            .try_acquire()
            .map_err(|_| RunnerOperationError::Busy)?;
        tokio::time::timeout(
            self.inner
                .config
                .execution_timeout
                .min(Duration::from_secs(8)),
            self.inner.driver.complete(request),
        )
        .await
        .map_err(|_| RunnerOperationError::Timeout)?
    }

    pub async fn detach(
        &self,
        request: RunnerDetachRequest<'_>,
    ) -> Result<RunnerDetachOutcome, RunnerOperationError> {
        tokio::time::timeout(
            self.inner.config.execution_timeout,
            self.inner.driver.detach(request),
        )
        .await
        .map_err(|_| RunnerOperationError::Timeout)?
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

fn validate_compiler_owner(owner: &str) -> Result<(), RunnerOperationError> {
    if owner.trim().is_empty() {
        Err(RunnerOperationError::InvalidRequest(
            "runner compiler request requires an owner".into(),
        ))
    } else {
        Ok(())
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
#[path = "runner_manager/tests.rs"]
mod tests;
