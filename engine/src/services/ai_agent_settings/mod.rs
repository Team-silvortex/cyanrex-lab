use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

use crate::models::ai_agent::{AiAgentSettings, UpdateAiAgentSettingsRequest};

#[cfg(unix)]
mod file;
#[cfg(all(test, unix))]
mod tests;
mod validation;

pub const MAX_AI_AGENT_SETTINGS_BYTES: usize = 32 * 1024;
type Result<T> = std::result::Result<T, AiAgentSettingsError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AiAgentSettingsError {
    #[error("invalid AI Agent settings")]
    InvalidInput,
    #[error("AI Agent settings revision conflict")]
    Conflict,
    #[error("AI Agent settings operation could not be confirmed")]
    StorageUnavailable,
}

#[derive(Clone)]
pub struct AiAgentSettingsStore {
    directory: PathBuf,
    private_parent: Option<PathBuf>,
    gate: Arc<Mutex<()>>,
}

impl AiAgentSettingsStore {
    /// Trusted local configuration only. Construction performs no filesystem I/O.
    /// Clones share admission; independently constructed stores/processes do not.
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            private_parent: None,
            gate: Arc::new(Mutex::new(())),
        }
    }

    pub fn from_env() -> Self {
        let root = std::env::var("CYANREX_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./data"));
        let parent = root.join("ai-agents");
        let mut store = Self::new(parent.join(crate::config::runtime_instance_id()));
        store.private_parent = Some(parent);
        store
    }

    pub async fn get(&self) -> Result<AiAgentSettings> {
        self.execute(None).await
    }

    pub async fn update(&self, request: UpdateAiAgentSettingsRequest) -> Result<AiAgentSettings> {
        validation::validate(&request.default_profile_id, &request.profiles)?;
        if serde_json::to_vec(&request)
            .map_err(|_| AiAgentSettingsError::InvalidInput)?
            .len()
            > MAX_AI_AGENT_SETTINGS_BYTES
        {
            return Err(AiAgentSettingsError::InvalidInput);
        }
        self.execute(Some(request)).await
    }

    async fn execute(
        &self,
        update: Option<UpdateAiAgentSettingsRequest>,
    ) -> Result<AiAgentSettings> {
        let guard = self.gate.clone().lock_owned().await;
        let directory = self.directory.clone();
        let private_parent = self.private_parent.clone();
        // Dispatch owns the guard through read/compare/rename and fsync. Dropping the
        // caller after this point cannot release admission or guarantee rollback.
        tokio::task::spawn_blocking(move || {
            let _guard = guard;
            #[cfg(unix)]
            {
                file::execute(&directory, private_parent.as_deref(), update)
            }
            #[cfg(not(unix))]
            {
                let _ = (directory, private_parent, update);
                Err(AiAgentSettingsError::StorageUnavailable)
            }
        })
        .await
        .map_err(|_| AiAgentSettingsError::StorageUnavailable)?
    }
}
