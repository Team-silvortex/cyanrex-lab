use super::{config::Config, private_file::Reserved, Result};
use cyanrex_engine::services::auth_service::durable_source::AuthorityBootstrap;
use serde_json::json;
use std::path::Path;

pub struct Enrollment(Reserved);
impl Enrollment {
    pub fn reserve(path: &Path, confirmation: &str) -> Result<Self> {
        let marker = serde_json::to_vec(&json!({"format_version":1,"status":"pending_reconciliation",
            "confirmation":confirmation,"note":"not a commit receipt; never automatically retry or discard this marker"}))
            .map_err(|_| "enrollment_unavailable")?;
        Reserved::create(path, &marker).map(Self)
    }
    pub fn publish(
        &mut self,
        config: &Config,
        confirmation: &str,
        result: &AuthorityBootstrap,
    ) -> Result<()> {
        // Called only after confirmed COMMIT. No password/session token and no console secret.
        // Database + filesystem cannot commit atomically: all subsequent failures need reconciliation.
        let enrollment = serde_json::to_vec(&json!({"format_version":1,"status":"committed",
            "confirmation":confirmation,"target":config.public_target(),
            "account_id":result.registration.account.account_id,
            "principal_id":result.identity.principal.reference.principal_id,
            "totp":{"issuer":result.registration.bootstrap.issuer,
                "account_name":result.registration.bootstrap.account_name,
                "secret":result.registration.bootstrap.secret,"otpauth_uri":result.registration.bootstrap.otpauth_uri},
            "session_issued":false,"live_runtime_changed":false}))
            .map_err(|_| "committed_delivery_unconfirmed")?;
        self.0
            .write(&enrollment)
            .map_err(|_| "committed_delivery_unconfirmed")
    }
}
