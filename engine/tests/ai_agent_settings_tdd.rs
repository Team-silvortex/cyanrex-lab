#![cfg(unix)]

use cyanrex_engine::{
    models::ai_agent::{AiAgentProfile, AiAgentProtocol, UpdateAiAgentSettingsRequest},
    services::ai_agent_settings::{AiAgentSettingsError, AiAgentSettingsStore},
};

#[path = "ai_agent_settings_tdd/persistence.rs"]
mod persistence;
#[path = "ai_agent_settings_tdd/routes.rs"]
mod routes;
#[path = "ai_agent_settings_tdd/validation.rs"]
mod validation;

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("cyanrex-ai-settings-{}", uuid::Uuid::new_v4())))
    }
    fn store(&self) -> AiAgentSettingsStore {
        AiAgentSettingsStore::new(self.0.clone())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if self.0.is_dir() {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

fn update(revision: u32) -> UpdateAiAgentSettingsRequest {
    UpdateAiAgentSettingsRequest {
        expected_revision: revision,
        default_profile_id: Some("local".into()),
        profiles: vec![AiAgentProfile {
            id: "local".into(),
            name: "Local model".into(),
            protocol: AiAgentProtocol::OpenaiResponses,
            base_url: "http://127.0.0.1:11434/v1".into(),
            model: "example-model".into(),
            credential_ref: Some("LOCAL_MODEL_KEY".into()),
            enabled: true,
        }],
    }
}

#[tokio::test]
async fn successful_update_persists_a_new_revision() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let result = store.update(update(0)).await;
    let persisted = fixture.store().get().await;
    assert_eq!(result.as_ref().map(|settings| settings.revision), Ok(1));
    assert_eq!(persisted, result);
}
