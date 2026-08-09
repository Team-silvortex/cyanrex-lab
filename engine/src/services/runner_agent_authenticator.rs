use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use chrono::Utc;
use data_encoding::HEXLOWER;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const SIGNATURE_SCHEME: &str = "hmac-sha256-v1";
const MAX_CREDENTIALS: usize = 256;
const MAX_NONCES_PER_AGENT: usize = 1024;

#[derive(Clone)]
pub struct RunnerAgentAuthenticator {
    inner: Arc<RunnerAgentAuthenticatorInner>,
}

struct RunnerAgentAuthenticatorInner {
    freshness_window: Duration,
    credentials: Mutex<HashMap<String, AgentCredential>>,
}

struct AgentCredential {
    secret: String,
    observed_nonces: HashMap<String, i64>,
}

pub struct RunnerAgentSignedRequest<'a> {
    pub agent_id: &'a str,
    pub method: &'a str,
    pub path: &'a str,
    pub timestamp: &'a str,
    pub nonce: &'a str,
    pub signature: &'a str,
    pub body: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerAgentSignatureError {
    UnknownAgent,
    Invalid,
    Stale,
    Replay,
}

impl RunnerAgentAuthenticator {
    pub fn from_env() -> Self {
        let freshness_window = Duration::from_secs(
            env_u64("CYANREX_RUNNER_AGENT_SIGNATURE_WINDOW_SECS", 60).clamp(15, 300),
        );
        Self::new(freshness_window)
    }

    pub fn new(freshness_window: Duration) -> Self {
        Self {
            inner: Arc::new(RunnerAgentAuthenticatorInner {
                freshness_window,
                credentials: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub fn issue(&self, agent_id: &str, active_agent_ids: &[String]) -> Result<String, String> {
        let active = active_agent_ids.iter().collect::<HashSet<_>>();
        let mut credentials = self.credentials();
        credentials.retain(|id, _| active.contains(id));
        if !credentials.contains_key(agent_id) && credentials.len() >= MAX_CREDENTIALS {
            return Err("runner agent credential capacity reached".to_string());
        }
        let secret = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        credentials.insert(
            agent_id.to_string(),
            AgentCredential {
                secret: secret.clone(),
                observed_nonces: HashMap::new(),
            },
        );
        Ok(secret)
    }

    pub fn verify(
        &self,
        request: RunnerAgentSignedRequest<'_>,
    ) -> Result<(), RunnerAgentSignatureError> {
        validate_signed_fields(&request)?;
        let timestamp = request
            .timestamp
            .parse::<i64>()
            .map_err(|_| RunnerAgentSignatureError::Invalid)?;
        let now = Utc::now().timestamp();
        if now.abs_diff(timestamp) > self.inner.freshness_window.as_secs() {
            return Err(RunnerAgentSignatureError::Stale);
        }

        let mut credentials = self.credentials();
        let credential = credentials
            .get_mut(request.agent_id)
            .ok_or(RunnerAgentSignatureError::UnknownAgent)?;
        let canonical = canonical_request(&request);
        let signature = HEXLOWER
            .decode(request.signature.as_bytes())
            .map_err(|_| RunnerAgentSignatureError::Invalid)?;
        let mut mac = Hmac::<Sha256>::new_from_slice(credential.secret.as_bytes())
            .expect("HMAC accepts arbitrary key lengths");
        mac.update(canonical.as_bytes());
        mac.verify_slice(&signature)
            .map_err(|_| RunnerAgentSignatureError::Invalid)?;

        let oldest = now - self.inner.freshness_window.as_secs() as i64;
        credential
            .observed_nonces
            .retain(|_, observed_at| *observed_at >= oldest);
        if credential.observed_nonces.contains_key(request.nonce) {
            return Err(RunnerAgentSignatureError::Replay);
        }
        if credential.observed_nonces.len() >= MAX_NONCES_PER_AGENT {
            return Err(RunnerAgentSignatureError::Invalid);
        }
        credential
            .observed_nonces
            .insert(request.nonce.to_string(), now);
        Ok(())
    }

    pub fn signature_scheme(&self) -> &'static str {
        SIGNATURE_SCHEME
    }

    fn credentials(&self) -> MutexGuard<'_, HashMap<String, AgentCredential>> {
        self.inner
            .credentials
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn validate_signed_fields(
    request: &RunnerAgentSignedRequest<'_>,
) -> Result<(), RunnerAgentSignatureError> {
    if request.agent_id.is_empty()
        || request.agent_id.len() > 64
        || request.method.is_empty()
        || request.method.len() > 16
        || request.path.is_empty()
        || request.path.len() > 128
        || !(16..=64).contains(&request.nonce.len())
        || !request
            .nonce
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_-".contains(character))
        || request.signature.len() != 64
    {
        return Err(RunnerAgentSignatureError::Invalid);
    }
    Ok(())
}

fn canonical_request(request: &RunnerAgentSignedRequest<'_>) -> String {
    let body_hash = HEXLOWER.encode(&Sha256::digest(request.body));
    format!(
        "CYANREX-RUNNER-V1\n{}\n{}\n{}\n{}\n{}\n{}",
        request.method, request.path, request.agent_id, request.timestamp, request.nonce, body_hash
    )
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

    fn signed<'a>(
        secret: &str,
        timestamp: &'a str,
        nonce: &'a str,
        body: &'a [u8],
    ) -> (String, RunnerAgentSignedRequest<'a>) {
        let mut request = RunnerAgentSignedRequest {
            agent_id: "lab-vm-01",
            method: "POST",
            path: "/runner/agent/heartbeat",
            timestamp,
            nonce,
            signature: "",
            body,
        };
        let canonical = canonical_request(&request);
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(canonical.as_bytes());
        let signature = HEXLOWER.encode(&mac.finalize().into_bytes());
        request.signature = "";
        (signature, request)
    }

    #[test]
    fn valid_signature_is_accepted_once() {
        let auth = RunnerAgentAuthenticator::new(Duration::from_secs(60));
        let secret = auth.issue("lab-vm-01", &["lab-vm-01".to_string()]).unwrap();
        let timestamp = Utc::now().timestamp().to_string();
        let (signature, mut request) =
            signed(&secret, &timestamp, "nonce-1234567890", br#"{"ok":true}"#);
        request.signature = &signature;
        assert_eq!(auth.verify(request), Ok(()));

        let (_, mut replay) = signed(&secret, &timestamp, "nonce-1234567890", br#"{"ok":true}"#);
        replay.signature = &signature;
        assert_eq!(auth.verify(replay), Err(RunnerAgentSignatureError::Replay));
    }

    #[test]
    fn tampering_rotation_and_stale_timestamps_are_rejected() {
        let auth = RunnerAgentAuthenticator::new(Duration::from_secs(15));
        let ids = ["lab-vm-01".to_string()];
        let secret = auth.issue("lab-vm-01", &ids).unwrap();
        let timestamp = Utc::now().timestamp().to_string();
        let (signature, mut tampered) =
            signed(&secret, &timestamp, "nonce-1234567891", br#"{"ok":false}"#);
        tampered.signature = &signature;
        tampered.body = br#"{"ok":true}"#;
        assert_eq!(
            auth.verify(tampered),
            Err(RunnerAgentSignatureError::Invalid)
        );

        let old_secret = auth.issue("lab-vm-01", &ids).unwrap();
        let new_secret = auth.issue("lab-vm-01", &ids).unwrap();
        assert_ne!(old_secret, new_secret);
        let (rotated_signature, mut rotated) =
            signed(&old_secret, &timestamp, "nonce-1234567893", br#"{}"#);
        rotated.signature = &rotated_signature;
        assert_eq!(
            auth.verify(rotated),
            Err(RunnerAgentSignatureError::Invalid)
        );
        let stale_timestamp = (Utc::now().timestamp() - 16).to_string();
        let (stale_signature, mut stale) =
            signed(&new_secret, &stale_timestamp, "nonce-1234567892", br#"{}"#);
        stale.signature = &stale_signature;
        assert_eq!(auth.verify(stale), Err(RunnerAgentSignatureError::Stale));
    }
}
