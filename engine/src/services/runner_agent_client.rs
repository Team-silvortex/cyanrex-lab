use std::{collections::BTreeMap, fs, time::Duration};

use chrono::Utc;
use reqwest::{redirect::Policy, StatusCode, Url};
use serde::{de::DeserializeOwned, Serialize};
use tokio::time::sleep;
use uuid::Uuid;

use crate::{
    models::{
        runner_agent::{
            RunnerAgentHeartbeatRequest, RunnerAgentIsolation, RunnerAgentRegisterRequest,
            RunnerAgentRegistrationResponse, RunnerAgentState, RunnerAgentView,
        },
        runner_job::{
            RunnerJobClaim, RunnerJobClaimRequest, RunnerJobClaimResponse, RunnerJobLeaseReference,
            RunnerJobResultRequest, RunnerJobResultState, RunnerJobSyncRequest,
            RunnerJobSyncResponse, RunnerJobView,
        },
    },
    services::{
        runner_agent_authenticator::sign_runner_agent_request,
        runner_agent_executor::{execute_runner_job, kernel_release, RunnerCompileExecutorConfig},
    },
};

const MAX_RESPONSE_BYTES: u64 = 640 * 1024;

#[derive(Clone)]
pub struct RunnerAgentClientConfig {
    pub engine_url: String,
    pub bootstrap_token: String,
    pub agent_id: String,
    pub isolation: RunnerAgentIsolation,
    pub max_concurrent: u16,
    pub capabilities: Vec<String>,
    pub compile_check: Option<RunnerCompileExecutorConfig>,
    pub labels: BTreeMap<String, String>,
    pub poll_interval: Duration,
    pub request_timeout: Duration,
    pub run_once: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum RunnerAgentClientError {
    #[error("invalid Runner Agent configuration: {0}")]
    Config(String),
    #[error("Runner Agent transport failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("Runner Agent server returned HTTP {status}: {message}")]
    Server { status: StatusCode, message: String },
    #[error("Runner Agent protocol failed: {0}")]
    Protocol(String),
}

pub struct RunnerAgentClient {
    config: RunnerAgentClientConfig,
    http: reqwest::Client,
    credential: Option<String>,
}

impl RunnerAgentClientConfig {
    pub fn from_env() -> Result<Self, RunnerAgentClientError> {
        let allow_insecure = env_bool("CYANREX_AGENT_ALLOW_INSECURE_HTTP", false);
        let engine_url = validate_engine_url(
            &std::env::var("CYANREX_AGENT_ENGINE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string()),
            allow_insecure,
        )?;
        let bootstrap_token = read_bootstrap_token()?;
        if !(32..=512).contains(&bootstrap_token.len()) {
            return config_error("bootstrap token must contain 32-512 characters");
        }
        let agent_id = configured_agent_id()?;
        let isolation = parse_isolation(
            &std::env::var("CYANREX_AGENT_ISOLATION")
                .unwrap_or_else(|_| "shared_kernel".to_string()),
        )?;
        let max_concurrent = env_u64("CYANREX_AGENT_MAX_CONCURRENT", 1);
        if !(1..=32).contains(&max_concurrent) {
            return config_error("CYANREX_AGENT_MAX_CONCURRENT must be between 1 and 32");
        }
        let poll_seconds = env_u64("CYANREX_AGENT_POLL_SECS", 5);
        if !(1..=30).contains(&poll_seconds) {
            return config_error("CYANREX_AGENT_POLL_SECS must be between 1 and 30");
        }
        let timeout_seconds = env_u64("CYANREX_AGENT_REQUEST_TIMEOUT_SECS", 10);
        if !(2..=60).contains(&timeout_seconds) {
            return config_error("CYANREX_AGENT_REQUEST_TIMEOUT_SECS must be between 2 and 60");
        }
        let compile_check_enabled = env_bool("CYANREX_AGENT_ENABLE_COMPILE_CHECK", false);
        validate_compile_policy(isolation, compile_check_enabled)?;
        let capabilities = parse_capabilities(compile_check_enabled)?;
        let compile_check = if compile_check_enabled {
            Some(RunnerCompileExecutorConfig::from_env().map_err(RunnerAgentClientError::Config)?)
        } else {
            None
        };
        let mut labels = BTreeMap::new();
        labels.insert("arch".to_string(), std::env::consts::ARCH.to_string());
        labels.insert("os".to_string(), std::env::consts::OS.to_string());
        Ok(Self {
            engine_url,
            bootstrap_token,
            agent_id,
            isolation,
            max_concurrent: max_concurrent as u16,
            capabilities,
            compile_check,
            labels,
            poll_interval: Duration::from_secs(poll_seconds),
            request_timeout: Duration::from_secs(timeout_seconds),
            run_once: env_bool("CYANREX_AGENT_ONCE", false),
        })
    }
}

impl RunnerAgentClient {
    pub fn new(config: RunnerAgentClientConfig) -> Result<Self, RunnerAgentClientError> {
        validate_compile_policy(config.isolation, config.compile_check.is_some())?;
        let advertises_compile = config
            .capabilities
            .iter()
            .any(|capability| capability == "clang_check");
        if advertises_compile != config.compile_check.is_some() {
            return config_error("clang_check capability and executor setting must match");
        }
        let http = reqwest::Client::builder()
            .redirect(Policy::none())
            .no_proxy()
            .connect_timeout(config.request_timeout)
            .timeout(config.request_timeout)
            .user_agent(format!(
                "cyanrex-runner-agent/{}",
                env!("CARGO_PKG_VERSION")
            ))
            .build()?;
        Ok(Self {
            config,
            http,
            credential: None,
        })
    }

    pub fn is_registered(&self) -> bool {
        self.credential.is_some()
    }

    pub fn clear_registration(&mut self) {
        self.credential = None;
    }

    pub async fn register(&mut self) -> Result<RunnerAgentView, RunnerAgentClientError> {
        let request = RunnerAgentRegisterRequest {
            agent_id: self.config.agent_id.clone(),
            protocol_version: 1,
            agent_version: env!("CARGO_PKG_VERSION").to_string(),
            isolation: self.config.isolation,
            max_concurrent: self.config.max_concurrent,
            capabilities: self.config.capabilities.clone(),
            labels: self.config.labels.clone(),
        };
        let response = self
            .http
            .post(self.url("/runner/agent/register"))
            .bearer_auth(&self.config.bootstrap_token)
            .json(&request)
            .send()
            .await?;
        let registration: RunnerAgentRegistrationResponse = decode_response(response).await?;
        if registration.signature_scheme != "hmac-sha256-v1"
            || !(32..=512).contains(&registration.credential.len())
            || registration.agent.agent_id != self.config.agent_id
        {
            return Err(RunnerAgentClientError::Protocol(
                "registration response has invalid identity or credential".to_string(),
            ));
        }
        self.credential = Some(registration.credential);
        Ok(registration.agent)
    }

    pub async fn heartbeat(
        &self,
        state: RunnerAgentState,
        active_jobs: u16,
    ) -> Result<RunnerAgentView, RunnerAgentClientError> {
        let request = RunnerAgentHeartbeatRequest {
            agent_id: self.config.agent_id.clone(),
            state,
            active_jobs,
            available_slots: self.config.max_concurrent.saturating_sub(active_jobs),
            kernel_release: kernel_release(),
            message: None,
        };
        self.signed_post("/runner/agent/heartbeat", &request).await
    }

    pub async fn poll_once(&self) -> Result<Option<RunnerJobView>, RunnerAgentClientError> {
        self.heartbeat(RunnerAgentState::Healthy, 0).await?;
        let response: RunnerJobClaimResponse = self
            .signed_post(
                "/runner/agent/jobs/claim",
                &RunnerJobClaimRequest {
                    agent_id: self.config.agent_id.clone(),
                },
            )
            .await?;
        let Some(job) = response.job else {
            return Ok(None);
        };
        self.heartbeat(RunnerAgentState::Healthy, 1).await?;
        let result = self.process_job(&job).await;
        let _ = self.heartbeat(RunnerAgentState::Healthy, 0).await;
        result.map(Some)
    }

    async fn process_job(
        &self,
        job: &RunnerJobClaim,
    ) -> Result<RunnerJobView, RunnerAgentClientError> {
        let sync = self.sync(job).await?;
        let cancelled = sync.cancel_job_ids.iter().any(|id| id == &job.job_id);
        let execution = if cancelled {
            crate::services::runner_agent_executor::RunnerJobExecution {
                state: RunnerJobResultState::Cancelled,
                message: "cancel acknowledged".to_string(),
                output: None,
            }
        } else {
            execute_runner_job(
                &self.config.agent_id,
                job,
                self.config.compile_check.as_ref(),
            )
            .await
        };
        let (state, message, output) = (execution.state, execution.message, execution.output);
        let request = RunnerJobResultRequest {
            agent_id: self.config.agent_id.clone(),
            job_id: job.job_id.clone(),
            lease_token: job.lease_token.clone(),
            state,
            message: Some(message),
            output,
        };
        match self
            .signed_post("/runner/agent/jobs/result", &request)
            .await
        {
            Err(error) if error.is_conflict() && state != RunnerJobResultState::Cancelled => {
                let sync = self.sync(job).await?;
                if sync.cancel_job_ids.iter().any(|id| id == &job.job_id) {
                    let mut cancelled = request;
                    cancelled.state = RunnerJobResultState::Cancelled;
                    cancelled.message = Some("cancel acknowledged".to_string());
                    cancelled.output = None;
                    self.signed_post("/runner/agent/jobs/result", &cancelled)
                        .await
                } else {
                    Err(error)
                }
            }
            result => result,
        }
    }

    async fn sync(
        &self,
        job: &RunnerJobClaim,
    ) -> Result<RunnerJobSyncResponse, RunnerAgentClientError> {
        self.signed_post(
            "/runner/agent/jobs/sync",
            &RunnerJobSyncRequest {
                agent_id: self.config.agent_id.clone(),
                leases: vec![RunnerJobLeaseReference {
                    job_id: job.job_id.clone(),
                    lease_token: job.lease_token.clone(),
                }],
            },
        )
        .await
    }

    async fn signed_post<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        request: &T,
    ) -> Result<R, RunnerAgentClientError> {
        let credential = self.credential.as_deref().ok_or_else(|| {
            RunnerAgentClientError::Protocol("Agent is not registered".to_string())
        })?;
        let body = serde_json::to_vec(request)
            .map_err(|error| RunnerAgentClientError::Protocol(error.to_string()))?;
        let timestamp = Utc::now().timestamp().to_string();
        let nonce = format!("nonce-{}", Uuid::new_v4().simple());
        let signature = sign_runner_agent_request(
            credential,
            &self.config.agent_id,
            "POST",
            path,
            &timestamp,
            &nonce,
            &body,
        )
        .map_err(|_| RunnerAgentClientError::Protocol("request signing failed".to_string()))?;
        let response = self
            .http
            .post(self.url(path))
            .header("content-type", "application/json")
            .header("x-cyanrex-agent-id", &self.config.agent_id)
            .header("x-cyanrex-agent-timestamp", timestamp)
            .header("x-cyanrex-agent-nonce", nonce)
            .header("x-cyanrex-agent-signature", signature)
            .body(body)
            .send()
            .await?;
        decode_response(response).await
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.engine_url, path)
    }
}

impl RunnerAgentClientError {
    fn is_conflict(&self) -> bool {
        matches!(self, Self::Server { status, .. } if *status == StatusCode::CONFLICT)
    }

    fn requires_registration(&self) -> bool {
        matches!(self, Self::Server { status, .. } if matches!(*status, StatusCode::UNAUTHORIZED | StatusCode::NOT_FOUND))
    }

    fn retryable(&self) -> bool {
        matches!(self, Self::Transport(_))
            || matches!(self, Self::Server { status, .. } if status.is_server_error() || matches!(*status, StatusCode::TOO_MANY_REQUESTS | StatusCode::CONFLICT))
    }
}

pub async fn run_runner_agent(
    config: RunnerAgentClientConfig,
) -> Result<(), RunnerAgentClientError> {
    let run_once = config.run_once;
    let poll_interval = config.poll_interval;
    let mut client = RunnerAgentClient::new(config)?;
    loop {
        if !client.is_registered() {
            match client.register().await {
                Ok(agent) => tracing::info!(agent_id = %agent.agent_id, "Runner Agent registered"),
                Err(error) if error.retryable() => {
                    tracing::warn!(%error, "Runner Agent registration will retry");
                    sleep(poll_interval).await;
                    continue;
                }
                Err(error) => return Err(error),
            }
        }
        match client.poll_once().await {
            Ok(Some(job)) => {
                tracing::info!(job_id = %job.job_id, state = ?job.state, "Runner Agent probe finished")
            }
            Ok(None) => {}
            Err(error) if error.requires_registration() => {
                tracing::warn!(%error, "Runner Agent credential is no longer accepted; re-registering");
                client.clear_registration();
            }
            Err(error) if error.retryable() => {
                tracing::warn!(%error, "Runner Agent poll will retry")
            }
            Err(error) => return Err(error),
        }
        if run_once {
            return Ok(());
        }
        tokio::select! {
            _ = sleep(poll_interval) => {}
            _ = tokio::signal::ctrl_c() => {
                let _ = client.heartbeat(RunnerAgentState::Draining, 0).await;
                return Ok(());
            }
        }
    }
}

async fn decode_response<T: DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, RunnerAgentClientError> {
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|size| size > MAX_RESPONSE_BYTES)
    {
        return Err(RunnerAgentClientError::Protocol(
            "server response exceeds 640 KiB".to_string(),
        ));
    }
    let bytes = response.bytes().await?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(RunnerAgentClientError::Protocol(
            "server response exceeds 640 KiB".to_string(),
        ));
    }
    if !status.is_success() {
        let message = serde_json::from_slice::<serde_json::Value>(&bytes)
            .ok()
            .and_then(|value| value["message"].as_str().map(str::to_string))
            .unwrap_or_else(|| String::from_utf8_lossy(&bytes).chars().take(256).collect());
        return Err(RunnerAgentClientError::Server { status, message });
    }
    serde_json::from_slice(&bytes).map_err(|error| {
        RunnerAgentClientError::Protocol(format!("invalid server JSON response: {error}"))
    })
}

fn validate_engine_url(raw: &str, allow_insecure: bool) -> Result<String, RunnerAgentClientError> {
    let url = Url::parse(raw.trim()).map_err(|_| {
        RunnerAgentClientError::Config("CYANREX_AGENT_ENGINE_URL is invalid".to_string())
    })?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return config_error("Engine URL cannot contain credentials, query, fragment, or a path");
    }
    match url.scheme() {
        "https" => {}
        "http" if allow_insecure || is_loopback_host(url.host_str()) => {}
        "http" => {
            return config_error(
                "non-loopback HTTP requires CYANREX_AGENT_ALLOW_INSECURE_HTTP=true",
            )
        }
        _ => return config_error("Engine URL scheme must be http or https"),
    }
    Ok(raw.trim().trim_end_matches('/').to_string())
}

fn read_bootstrap_token() -> Result<String, RunnerAgentClientError> {
    let direct = std::env::var("CYANREX_AGENT_BOOTSTRAP_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let file = std::env::var("CYANREX_AGENT_BOOTSTRAP_TOKEN_FILE")
        .ok()
        .filter(|value| !value.trim().is_empty());
    match (direct, file) {
        (Some(_), Some(_)) => {
            config_error("configure the bootstrap token directly or by file, not both")
        }
        (Some(value), None) => Ok(value.trim().to_string()),
        (None, Some(path)) => {
            let metadata = fs::metadata(&path).map_err(|error| {
                RunnerAgentClientError::Config(format!(
                    "cannot inspect bootstrap token file: {error}"
                ))
            })?;
            if metadata.len() > 4096 {
                return config_error("bootstrap token file exceeds 4096 bytes");
            }
            fs::read_to_string(path)
                .map(|value| value.trim().to_string())
                .map_err(|error| {
                    RunnerAgentClientError::Config(format!(
                        "cannot read bootstrap token file: {error}"
                    ))
                })
        }
        (None, None) => {
            config_error("CYANREX_AGENT_BOOTSTRAP_TOKEN or its file variant is required")
        }
    }
}

fn configured_agent_id() -> Result<String, RunnerAgentClientError> {
    let raw = std::env::var("CYANREX_AGENT_ID")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "cyanrex-agent".to_string());
    let value = raw.trim();
    if !(3..=64).contains(&value.len())
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_-.".contains(character))
    {
        return config_error("CYANREX_AGENT_ID must be a 3-64 character safe identifier");
    }
    Ok(value.to_string())
}

fn parse_isolation(raw: &str) -> Result<RunnerAgentIsolation, RunnerAgentClientError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "shared_kernel" => Ok(RunnerAgentIsolation::SharedKernel),
        "container" => Ok(RunnerAgentIsolation::Container),
        "virtual_machine" => Ok(RunnerAgentIsolation::VirtualMachine),
        "dedicated_host" => Ok(RunnerAgentIsolation::DedicatedHost),
        _ => config_error("CYANREX_AGENT_ISOLATION is unsupported"),
    }
}

fn validate_compile_policy(
    isolation: RunnerAgentIsolation,
    enabled: bool,
) -> Result<(), RunnerAgentClientError> {
    if enabled && isolation == RunnerAgentIsolation::SharedKernel {
        return config_error("compile checking cannot run with shared_kernel isolation");
    }
    Ok(())
}

fn parse_capabilities(compile_check_enabled: bool) -> Result<Vec<String>, RunnerAgentClientError> {
    let raw =
        std::env::var("CYANREX_AGENT_CAPABILITIES").unwrap_or_else(|_| "control_probe".to_string());
    let mut values = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    let advertises_compile = values.iter().any(|value| value == "clang_check");
    if compile_check_enabled && !advertises_compile {
        values.push("clang_check".to_string());
        values.sort();
    } else if !compile_check_enabled && advertises_compile {
        return config_error("clang_check requires CYANREX_AGENT_ENABLE_COMPILE_CHECK=true");
    }
    if values.is_empty()
        || values.len() > 32
        || !values.iter().any(|value| value == "control_probe")
    {
        return config_error(
            "Agent capabilities must include control_probe and contain at most 32 entries",
        );
    }
    Ok(values)
}

fn is_loopback_host(host: Option<&str>) -> bool {
    matches!(host, Some("localhost" | "127.0.0.1" | "::1"))
}

fn env_u64(name: &str, fallback: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

fn env_bool(name: &str, fallback: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(fallback)
}

fn config_error<T>(message: &str) -> Result<T, RunnerAgentClientError> {
    Err(RunnerAgentClientError::Config(message.to_string()))
}

#[cfg(test)]
include!("runner_agent_client/tests.inc.rs");
