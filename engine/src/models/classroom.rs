use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize)]
pub struct ClassroomDiscovery {
    pub service: String,
    pub classroom_id: String,
    pub display_name: String,
    pub product_version: String,
    pub protocol_min: u16,
    pub protocol_max: u16,
    pub join_url: String,
    pub capabilities: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassroomInviteRequest {
    pub username: String,
}

#[derive(Clone, Serialize)]
pub struct ClassroomInvitationView {
    pub invite_id: String,
    pub username: String,
    pub expires_at: DateTime<Utc>,
}

// Deliberately not Debug: the URL contains a one-time credential in its fragment.
#[derive(Serialize)]
pub struct ClassroomInvitation {
    #[serde(flatten)]
    pub invitation: ClassroomInvitationView,
    pub join_url: String,
}

#[derive(Serialize)]
pub struct ClassroomInvitations {
    pub invitations: Vec<ClassroomInvitationView>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassroomRevokeRequest {
    pub invite_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassroomJoinRequest {
    pub classroom_id: String,
    pub protocol_version: u16,
    pub client_version: String,
    pub required_capabilities: Vec<String>,
    pub invite_token: String,
    pub username: String,
    pub password: String,
}
