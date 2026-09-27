use serde::{Deserialize, Serialize};

use super::{AuthorityId, LegacyUsername, PrincipalId, PrincipalRef, QualifiedName, WorkspaceRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalKind {
    Human,
    Agent,
    Service,
    Machine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalStatus {
    Active,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Principal {
    pub reference: PrincipalRef,
    pub kind: PrincipalKind,
    pub display_name: String,
    pub status: PrincipalStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceStatus {
    Active,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workspace {
    pub reference: WorkspaceRef,
    pub title: String,
    pub status: WorkspaceStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MembershipStatus {
    Active,
    Suspended,
}

/// Roles name workspace-local presets; the names themselves confer no deployment authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Membership {
    pub workspace: WorkspaceRef,
    pub principal_id: PrincipalId,
    pub role_refs: Vec<QualifiedName>,
    pub status: MembershipStatus,
}

/// Offline mapping input, not a new authentication method or a client-supplied login claim.
/// C1 must persist its uniqueness and handle retired/recreated accounts before using it at runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyIdentityBinding {
    pub authority_id: AuthorityId,
    pub username: LegacyUsername,
    pub principal_id: PrincipalId,
}

/// Preview of the authority already held by a legacy deployment teacher, limited to that instance.
/// This is not a bearer credential, an active grant store, or authority on a new instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstanceDeploymentGrant {
    pub authority_id: AuthorityId,
    pub principal_id: PrincipalId,
}
