use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use chrono::{Duration, Utc};
use reqwest::Url;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::models::classroom::{
    ClassroomDiscovery, ClassroomInvitation, ClassroomInvitationView, ClassroomJoinRequest,
};

pub const JOIN_PROTOCOL: u16 = 1;
pub const INVITATION_TTL_SECONDS: i64 = 600;
const MAX_INVITATIONS: usize = 256;
const CAPABILITY: &str = "student-invite-v1";

#[derive(Clone, Default)]
pub struct ClassroomService {
    discovery: Option<ClassroomDiscovery>,
    invitations: Arc<Mutex<HashMap<[u8; 32], ClassroomInvitationView>>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ClassroomError {
    Disabled,
    InvalidInput,
    WrongClassroom,
    Incompatible,
    InvalidInvitation,
    Capacity,
}

impl ClassroomService {
    pub fn from_env() -> Result<Self, String> {
        let setting = |name| std::env::var(name).unwrap_or_default();
        let url = setting("CYANREX_CLASSROOM_PUBLIC_URL");
        let id = setting("CYANREX_CLASSROOM_ID");
        let name = setting("CYANREX_CLASSROOM_NAME");
        if url.is_empty() && id.is_empty() && name.is_empty() {
            return Ok(Self::default());
        }
        Self::configured(&id, &name, &url)
    }

    pub fn configured(id: &str, name: &str, public_url: &str) -> Result<Self, String> {
        let parsed_id = Uuid::parse_str(id).map_err(|_| "classroom ID must be a canonical UUID")?;
        if parsed_id.is_nil() || parsed_id.to_string() != id {
            return Err("classroom ID must be a non-nil canonical UUID".into());
        }
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
            return Err("classroom name must contain 1–64 printable characters".into());
        }
        let url = Url::parse(public_url)
            .map_err(|_| "classroom public URL must be an absolute origin")?;
        let host = url.host_str().unwrap_or_default().trim_matches(['[', ']']);
        let loopback = host == "localhost"
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback());
        if !(url.scheme() == "https" || url.scheme() == "http" && loopback)
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
            || public_url.chars().any(char::is_control)
        {
            return Err("classroom URL requires HTTPS (HTTP only on loopback), with no credentials, path, query or fragment".into());
        }
        Ok(Self {
            discovery: Some(ClassroomDiscovery {
                service: "cyanrex-classroom".into(),
                classroom_id: id.into(),
                display_name: name.into(),
                product_version: env!("CARGO_PKG_VERSION").into(),
                protocol_min: JOIN_PROTOCOL,
                protocol_max: JOIN_PROTOCOL,
                join_url: format!("{}/join", url.origin().ascii_serialization()),
                capabilities: vec![CAPABILITY.into()],
            }),
            ..Self::default()
        })
    }

    pub fn discovery(&self) -> Result<ClassroomDiscovery, ClassroomError> {
        self.discovery.clone().ok_or(ClassroomError::Disabled)
    }

    pub fn issue(&self, username: &str) -> Result<ClassroomInvitation, ClassroomError> {
        let discovery = self.discovery()?;
        let username = normalize_student_username(username)?;
        let mut invitations = self
            .invitations
            .lock()
            .expect("classroom invitations lock poisoned");
        let now = Utc::now();
        invitations.retain(|_, invitation| invitation.expires_at > now);
        if invitations.len() >= MAX_INVITATIONS {
            return Err(ClassroomError::Capacity);
        }
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let invitation = ClassroomInvitationView {
            invite_id: Uuid::new_v4().to_string(),
            username,
            expires_at: now + Duration::seconds(INVITATION_TTL_SECONDS),
        };
        invitations.insert(token_hash(&token), invitation.clone());
        let join_url = format!(
            "{}#invite={token}&classroom={}&protocol={JOIN_PROTOCOL}",
            discovery.join_url, discovery.classroom_id
        );
        Ok(ClassroomInvitation {
            invitation,
            join_url,
        })
    }

    pub fn inventory(&self) -> Result<Vec<ClassroomInvitationView>, ClassroomError> {
        self.discovery()?;
        let mut invitations = self
            .invitations
            .lock()
            .expect("classroom invitations lock poisoned");
        invitations.retain(|_, invitation| invitation.expires_at > Utc::now());
        let mut values = invitations.values().cloned().collect::<Vec<_>>();
        values.sort_by(|a, b| {
            a.expires_at
                .cmp(&b.expires_at)
                .then(a.invite_id.cmp(&b.invite_id))
        });
        Ok(values)
    }

    pub fn revoke(&self, id: &str) -> Result<(), ClassroomError> {
        self.discovery()?;
        let id = Uuid::parse_str(id)
            .map_err(|_| ClassroomError::InvalidInput)?
            .to_string();
        // Idempotent and safe after an uncertain response or concurrent redemption.
        self.invitations
            .lock()
            .expect("classroom invitations lock poisoned")
            .retain(|_, invitation| invitation.invite_id != id);
        Ok(())
    }

    pub fn redeem(&self, request: &ClassroomJoinRequest) -> Result<String, ClassroomError> {
        let discovery = self.discovery()?;
        if request.classroom_id != discovery.classroom_id {
            return Err(ClassroomError::WrongClassroom);
        }
        if request.protocol_version != JOIN_PROTOCOL
            || request.required_capabilities.len() > 16
            || request
                .required_capabilities
                .iter()
                .any(|capability| capability != CAPABILITY)
        {
            return Err(ClassroomError::Incompatible);
        }
        if request.client_version.is_empty()
            || request.client_version.len() > 64
            || !request
                .client_version
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".-+".contains(&b))
            || !(8..=256).contains(&request.password.len())
        {
            return Err(ClassroomError::InvalidInput);
        }
        let username = normalize_student_username(&request.username)?;
        if request.invite_token.len() != 64
            || !request.invite_token.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(ClassroomError::InvalidInvitation);
        }
        let hash = token_hash(&request.invite_token);
        let mut invitations = self
            .invitations
            .lock()
            .expect("classroom invitations lock poisoned");
        invitations.retain(|_, invitation| invitation.expires_at > Utc::now());
        match invitations.get(&hash) {
            Some(invitation) if invitation.username == username => {}
            _ => return Err(ClassroomError::InvalidInvitation),
        }
        // Consume atomically BEFORE asynchronous password hashing/storage. Failure/cancellation
        // never restores a credential whose use may have committed; a teacher must reissue it.
        invitations.remove(&hash);
        Ok(username)
    }
}

fn token_hash(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

fn normalize_student_username(username: &str) -> Result<String, ClassroomError> {
    let value = username.trim().to_ascii_lowercase();
    if !(3..=64).contains(&value.len())
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
    {
        return Err(ClassroomError::InvalidInput);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    const ID: &str = "23d40d83-19de-43d3-85fe-506d86592a70";

    #[test]
    fn classroom_configuration_fails_closed() {
        for url in [
            "http://192.168.1.2",
            "https://user:password@teacher.local",
            "https://teacher.local/path",
            "https://teacher.local/?token=secret",
            "https://teacher.local/#secret",
            "file:///tmp/x",
        ] {
            assert!(
                ClassroomService::configured(ID, "Lab", url).is_err(),
                "{url}"
            );
        }
        for url in [
            "https://teacher.local",
            "http://127.0.0.1:3000",
            "http://[::1]:3000",
            "http://localhost:3000",
        ] {
            assert!(
                ClassroomService::configured(ID, "Lab", url).is_ok(),
                "{url}"
            );
        }
        assert!(ClassroomService::configured("default", "Lab", "https://teacher.local").is_err());
        assert!(ClassroomService::configured(ID, "Lab\nsecret", "https://teacher.local").is_err());
    }

    #[test]
    fn classroom_invitations_are_bounded_expire_and_do_not_survive_restart() {
        let service = ClassroomService::configured(ID, "Lab", "https://teacher.local").unwrap();
        for index in 0..MAX_INVITATIONS {
            service.issue(&format!("student-{index}")).unwrap();
        }
        assert!(matches!(
            service.issue("overflow"),
            Err(ClassroomError::Capacity)
        ));
        for invite in service.invitations.lock().unwrap().values_mut() {
            invite.expires_at = Utc::now() - Duration::seconds(1);
        }
        assert!(service.inventory().unwrap().is_empty());
        service.issue("new-student").unwrap();
        assert!(
            ClassroomService::configured(ID, "Lab", "https://teacher.local")
                .unwrap()
                .inventory()
                .unwrap()
                .is_empty()
        );
    }
}
