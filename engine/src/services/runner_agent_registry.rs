use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::models::runner_agent::{
    RunnerAgentHeartbeatRequest, RunnerAgentInventory, RunnerAgentIsolation,
    RunnerAgentRegisterRequest, RunnerAgentState, RunnerAgentView,
};

const AGENT_PROTOCOL_VERSION: u16 = 1;
const MAX_REGISTERED_AGENTS: usize = 256;
const TOKEN_COMPARISON_CONTEXT: &[u8] = b"cyanrex-runner-agent-auth-v1";

#[derive(Clone)]
pub struct RunnerAgentRegistry {
    inner: Arc<RunnerAgentRegistryInner>,
}

struct RunnerAgentRegistryInner {
    token: Option<String>,
    heartbeat_ttl: Duration,
    retention: Duration,
    agents: Mutex<HashMap<String, AgentRecord>>,
}

#[derive(Clone)]
struct AgentRecord {
    agent_id: String,
    protocol_version: u16,
    agent_version: String,
    isolation: RunnerAgentIsolation,
    state: RunnerAgentState,
    max_concurrent: u16,
    active_jobs: u16,
    available_slots: u16,
    capabilities: Vec<String>,
    labels: BTreeMap<String, String>,
    kernel_release: Option<String>,
    message: Option<String>,
    registered_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerAgentAccessError {
    Disabled,
    Unauthorized,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerAgentRegistryError {
    Invalid(String),
    NotFound,
}

impl RunnerAgentRegistry {
    pub fn from_env() -> Result<Self, String> {
        let token = std::env::var("CYANREX_RUNNER_AGENT_TOKEN")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let heartbeat_ttl =
            Duration::from_secs(env_u64("CYANREX_RUNNER_AGENT_TTL_SECS", 30).clamp(10, 300));
        let retention = Duration::from_secs(
            env_u64("CYANREX_RUNNER_AGENT_RETENTION_SECS", 300)
                .clamp(heartbeat_ttl.as_secs(), 3600),
        );
        Self::new(token, heartbeat_ttl, retention)
    }

    pub fn new(
        token: Option<String>,
        heartbeat_ttl: Duration,
        retention: Duration,
    ) -> Result<Self, String> {
        if token
            .as_ref()
            .is_some_and(|value| !(32..=512).contains(&value.len()))
        {
            return Err("CYANREX_RUNNER_AGENT_TOKEN must contain 32-512 characters".to_string());
        }
        if retention < heartbeat_ttl {
            return Err("runner agent retention must be at least the heartbeat TTL".to_string());
        }
        Ok(Self {
            inner: Arc::new(RunnerAgentRegistryInner {
                token,
                heartbeat_ttl,
                retention,
                agents: Mutex::new(HashMap::new()),
            }),
        })
    }

    pub fn authorize(&self, presented_token: Option<&str>) -> Result<(), RunnerAgentAccessError> {
        let Some(expected) = self.inner.token.as_deref() else {
            return Err(RunnerAgentAccessError::Disabled);
        };
        let Some(presented) = presented_token.filter(|value| value.len() <= 512) else {
            return Err(RunnerAgentAccessError::Unauthorized);
        };
        if constant_time_token_match(expected, presented) {
            Ok(())
        } else {
            Err(RunnerAgentAccessError::Unauthorized)
        }
    }

    pub fn register(
        &self,
        mut request: RunnerAgentRegisterRequest,
    ) -> Result<RunnerAgentView, RunnerAgentRegistryError> {
        validate_registration(&request)?;
        request.capabilities.sort();
        request.capabilities.dedup();
        let now = Utc::now();
        let record = AgentRecord {
            agent_id: request.agent_id.clone(),
            protocol_version: request.protocol_version,
            agent_version: request.agent_version,
            isolation: request.isolation,
            state: RunnerAgentState::Healthy,
            max_concurrent: request.max_concurrent,
            active_jobs: 0,
            available_slots: request.max_concurrent,
            capabilities: request.capabilities,
            labels: request.labels,
            kernel_release: None,
            message: None,
            registered_at: now,
            last_seen_at: now,
        };
        let view = self.view(&record, now);
        let mut agents = self.agents();
        self.prune(&mut agents, now);
        if !agents.contains_key(&request.agent_id) && agents.len() >= MAX_REGISTERED_AGENTS {
            return Err(RunnerAgentRegistryError::Invalid(
                "runner agent registry capacity reached".to_string(),
            ));
        }
        agents.insert(request.agent_id, record);
        Ok(view)
    }

    pub fn heartbeat(
        &self,
        request: RunnerAgentHeartbeatRequest,
    ) -> Result<RunnerAgentView, RunnerAgentRegistryError> {
        validate_heartbeat(&request)?;
        let now = Utc::now();
        let mut agents = self.agents();
        self.prune(&mut agents, now);
        let record = agents
            .get_mut(&request.agent_id)
            .ok_or(RunnerAgentRegistryError::NotFound)?;
        if request.active_jobs > record.max_concurrent
            || request.available_slots > record.max_concurrent
            || request.active_jobs.saturating_add(request.available_slots) > record.max_concurrent
        {
            return Err(RunnerAgentRegistryError::Invalid(
                "heartbeat capacity exceeds registered maximum".to_string(),
            ));
        }
        record.state = request.state;
        record.active_jobs = request.active_jobs;
        record.available_slots = request.available_slots;
        record.kernel_release = trim_optional(request.kernel_release);
        record.message = trim_optional(request.message);
        record.last_seen_at = now;
        Ok(self.view(record, now))
    }

    pub fn inventory(&self) -> RunnerAgentInventory {
        let now = Utc::now();
        let mut agents = self.agents();
        self.prune(&mut agents, now);
        let mut views = agents
            .values()
            .map(|record| self.view(record, now))
            .collect::<Vec<_>>();
        views.sort_by(|left, right| left.agent_id.cmp(&right.agent_id));
        RunnerAgentInventory {
            generated_at: now,
            total_agents: views.len(),
            online_agents: views
                .iter()
                .filter(|agent| agent.state != RunnerAgentState::Offline)
                .count(),
            agents: views,
        }
    }

    pub fn active_agent_ids(&self) -> Vec<String> {
        let now = Utc::now();
        let mut agents = self.agents();
        self.prune(&mut agents, now);
        agents.keys().cloned().collect()
    }

    pub fn agent(&self, agent_id: &str) -> Option<RunnerAgentView> {
        let now = Utc::now();
        let mut agents = self.agents();
        self.prune(&mut agents, now);
        agents.get(agent_id).map(|record| self.view(record, now))
    }

    fn view(&self, record: &AgentRecord, now: DateTime<Utc>) -> RunnerAgentView {
        let expires_at = record.last_seen_at
            + chrono::Duration::from_std(self.inner.heartbeat_ttl)
                .expect("agent heartbeat TTL is within chrono range");
        RunnerAgentView {
            agent_id: record.agent_id.clone(),
            protocol_version: record.protocol_version,
            agent_version: record.agent_version.clone(),
            isolation: record.isolation,
            state: if now > expires_at {
                RunnerAgentState::Offline
            } else {
                record.state
            },
            max_concurrent: record.max_concurrent,
            active_jobs: record.active_jobs,
            available_slots: record.available_slots,
            capabilities: record.capabilities.clone(),
            labels: record.labels.clone(),
            kernel_release: record.kernel_release.clone(),
            message: record.message.clone(),
            registered_at: record.registered_at,
            last_seen_at: record.last_seen_at,
            expires_at,
        }
    }

    fn prune(&self, agents: &mut HashMap<String, AgentRecord>, now: DateTime<Utc>) {
        let retention = chrono::Duration::from_std(self.inner.retention)
            .expect("agent retention is within chrono range");
        agents.retain(|_, record| now - record.last_seen_at <= retention);
    }

    fn agents(&self) -> MutexGuard<'_, HashMap<String, AgentRecord>> {
        self.inner
            .agents
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn validate_registration(
    request: &RunnerAgentRegisterRequest,
) -> Result<(), RunnerAgentRegistryError> {
    validate_agent_id(&request.agent_id)?;
    if request.protocol_version != AGENT_PROTOCOL_VERSION {
        return invalid("unsupported runner agent protocol version");
    }
    validate_name("agent_version", &request.agent_version, 32)?;
    if !(1..=32).contains(&request.max_concurrent) {
        return invalid("max_concurrent must be between 1 and 32");
    }
    if request.capabilities.is_empty() || request.capabilities.len() > 32 {
        return invalid("capabilities must contain 1-32 entries");
    }
    for capability in &request.capabilities {
        validate_name("capability", capability, 32)?;
    }
    if !request
        .capabilities
        .iter()
        .any(|value| matches!(value.as_str(), "control_probe" | "bpftool" | "aya"))
    {
        return invalid("agent must advertise `control_probe`, `bpftool`, or `aya`");
    }
    if request.labels.len() > 16 {
        return invalid("labels must contain at most 16 entries");
    }
    for (key, value) in &request.labels {
        validate_name("label key", key, 32)?;
        if value.is_empty() || value.len() > 64 || value.chars().any(char::is_control) {
            return invalid("label values must contain 1-64 printable characters");
        }
    }
    Ok(())
}

fn validate_heartbeat(
    request: &RunnerAgentHeartbeatRequest,
) -> Result<(), RunnerAgentRegistryError> {
    validate_agent_id(&request.agent_id)?;
    if request.state == RunnerAgentState::Offline {
        return invalid("agents cannot submit the offline state");
    }
    validate_optional("kernel_release", request.kernel_release.as_deref(), 128)?;
    validate_optional("message", request.message.as_deref(), 256)
}

fn validate_agent_id(value: &str) -> Result<(), RunnerAgentRegistryError> {
    validate_name("agent_id", value, 64)?;
    if value.len() < 3 {
        return invalid("agent_id must contain at least 3 characters");
    }
    Ok(())
}

fn validate_name(field: &str, value: &str, max_len: usize) -> Result<(), RunnerAgentRegistryError> {
    if value.is_empty()
        || value.len() > max_len
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_-.".contains(character))
    {
        return invalid(&format!(
            "{field} contains unsupported characters or length"
        ));
    }
    Ok(())
}

fn validate_optional(
    field: &str,
    value: Option<&str>,
    max_len: usize,
) -> Result<(), RunnerAgentRegistryError> {
    if value.is_some_and(|item| item.len() > max_len || item.chars().any(char::is_control)) {
        return invalid(&format!("{field} exceeds its printable text limit"));
    }
    Ok(())
}

fn invalid<T>(message: &str) -> Result<T, RunnerAgentRegistryError> {
    Err(RunnerAgentRegistryError::Invalid(message.to_string()))
}

fn trim_optional(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn constant_time_token_match(expected: &str, presented: &str) -> bool {
    let mut expected_mac = Hmac::<Sha256>::new_from_slice(expected.as_bytes())
        .expect("HMAC accepts arbitrary key lengths");
    expected_mac.update(TOKEN_COMPARISON_CONTEXT);
    let expected_tag = expected_mac.finalize().into_bytes();

    let mut presented_mac = Hmac::<Sha256>::new_from_slice(presented.as_bytes())
        .expect("HMAC accepts arbitrary key lengths");
    presented_mac.update(TOKEN_COMPARISON_CONTEXT);
    presented_mac.verify_slice(&expected_tag).is_ok()
}

fn env_u64(name: &str, fallback: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "runner-agent-test-token-with-32-characters";

    fn registration() -> RunnerAgentRegisterRequest {
        RunnerAgentRegisterRequest {
            agent_id: "lab-vm-01".to_string(),
            protocol_version: AGENT_PROTOCOL_VERSION,
            agent_version: "0.2.0".to_string(),
            isolation: RunnerAgentIsolation::VirtualMachine,
            max_concurrent: 2,
            capabilities: vec!["bpftool".to_string(), "btf".to_string()],
            labels: BTreeMap::new(),
        }
    }

    #[test]
    fn token_authentication_is_disabled_or_constant_time_checked() {
        let disabled =
            RunnerAgentRegistry::new(None, Duration::from_secs(10), Duration::from_secs(20))
                .expect("disabled registry should be valid");
        assert_eq!(
            disabled.authorize(Some(TOKEN)),
            Err(RunnerAgentAccessError::Disabled)
        );

        let enabled = RunnerAgentRegistry::new(
            Some(TOKEN.to_string()),
            Duration::from_secs(10),
            Duration::from_secs(20),
        )
        .expect("enabled registry should be valid");
        assert_eq!(enabled.authorize(Some(TOKEN)), Ok(()));
        assert_eq!(
            enabled.authorize(Some("wrong-token")),
            Err(RunnerAgentAccessError::Unauthorized)
        );
    }

    #[test]
    fn short_tokens_and_invalid_retention_are_rejected() {
        assert!(RunnerAgentRegistry::new(
            Some("too-short".to_string()),
            Duration::from_secs(10),
            Duration::from_secs(20),
        )
        .is_err());
        assert!(RunnerAgentRegistry::new(
            Some(TOKEN.to_string()),
            Duration::from_secs(20),
            Duration::from_secs(10),
        )
        .is_err());
    }

    #[test]
    fn stale_agents_become_offline_and_are_then_pruned() {
        let registry = RunnerAgentRegistry::new(
            Some(TOKEN.to_string()),
            Duration::from_secs(10),
            Duration::from_secs(20),
        )
        .expect("registry should be valid");
        registry
            .register(registration())
            .expect("registration should succeed");

        {
            let mut agents = registry.agents();
            agents.get_mut("lab-vm-01").unwrap().last_seen_at =
                Utc::now() - chrono::Duration::seconds(11);
        }
        let inventory = registry.inventory();
        assert_eq!(inventory.total_agents, 1);
        assert_eq!(inventory.online_agents, 0);
        assert_eq!(inventory.agents[0].state, RunnerAgentState::Offline);

        {
            let mut agents = registry.agents();
            agents.get_mut("lab-vm-01").unwrap().last_seen_at =
                Utc::now() - chrono::Duration::seconds(21);
        }
        assert_eq!(registry.inventory().total_agents, 0);
    }

    #[test]
    fn heartbeat_cannot_overstate_registered_capacity() {
        let registry = RunnerAgentRegistry::new(
            Some(TOKEN.to_string()),
            Duration::from_secs(10),
            Duration::from_secs(20),
        )
        .expect("registry should be valid");
        registry
            .register(registration())
            .expect("registration should succeed");
        let result = registry.heartbeat(RunnerAgentHeartbeatRequest {
            agent_id: "lab-vm-01".to_string(),
            state: RunnerAgentState::Healthy,
            active_jobs: 2,
            available_slots: 1,
            kernel_release: None,
            message: None,
        });
        assert!(matches!(result, Err(RunnerAgentRegistryError::Invalid(_))));
    }

    #[test]
    fn identifiers_cannot_contain_whitespace() {
        let mut request = registration();
        request.agent_id = "lab vm 01".to_string();
        assert!(matches!(
            validate_registration(&request),
            Err(RunnerAgentRegistryError::Invalid(_))
        ));
    }

    #[test]
    fn registry_has_a_hard_agent_count_limit() {
        let registry = RunnerAgentRegistry::new(
            Some(TOKEN.to_string()),
            Duration::from_secs(10),
            Duration::from_secs(20),
        )
        .expect("registry should be valid");
        for index in 0..MAX_REGISTERED_AGENTS {
            let mut request = registration();
            request.agent_id = format!("agent-{index:03}");
            registry.register(request).expect("agent should fit");
        }
        let mut overflow = registration();
        overflow.agent_id = "agent-overflow".to_string();
        assert!(matches!(
            registry.register(overflow),
            Err(RunnerAgentRegistryError::Invalid(_))
        ));
    }
}
