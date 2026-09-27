//! Pure C-M1 permission preview for a frozen legacy instance. No HTTP, SQL, environment reads,
//! ID allocation or writes. AuthService and the current route guards remain the runtime authority.
//!
//! The caller supplies verified identity mappings, roles and resource ownership from a trusted
//! snapshot. Never interpret browser/Agent claims as these inputs. This deliberately covers only
//! the four migration-sensitive actions below; it is not the general C1 policy engine.

use crate::models::{
    auth::AuthRole,
    collaboration::{
        AuthorityId, ContractError, InstanceDeploymentGrant, LegacyIdentityBinding, Membership,
        MembershipStatus, PrincipalId, PrincipalRef, WorkspaceRef,
    },
};

#[derive(Debug, Clone, Copy)]
pub struct LegacyWorkspaceProjection {
    workspace: WorkspaceRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyAccessPreview {
    membership: Membership,
    deployment_grant: Option<InstanceDeploymentGrant>,
}

#[derive(Debug, Clone, Copy)]
pub enum LegacyAction {
    ReadPrivateArtifact,
    RestoreAttempt,
    ReviewStudentAttempt,
    ManageDeployment,
}

#[derive(Debug, Clone, Copy)]
pub enum LegacyResource {
    PrivateArtifact {
        workspace: WorkspaceRef,
        owner_id: PrincipalId,
    },
    Attempt {
        workspace: WorkspaceRef,
        owner_id: PrincipalId,
        owner_role: AuthRole,
    },
    Deployment {
        authority_id: AuthorityId,
    },
}

impl LegacyWorkspaceProjection {
    /// `workspace` must be the operator-confirmed fixed legacy mapping, not request input.
    pub fn new(workspace: WorkspaceRef) -> Self {
        Self { workspace }
    }

    pub fn preview(
        &self,
        binding: &LegacyIdentityBinding,
        role: AuthRole,
    ) -> Result<LegacyAccessPreview, ContractError> {
        if binding.authority_id != self.workspace.authority_id {
            return Err(ContractError("identity belongs to a different authority"));
        }
        let teacher = matches!(role, AuthRole::Admin | AuthRole::Teacher);
        let role_ref = if teacher {
            "cyanrex.teaching.teacher"
        } else {
            "cyanrex.teaching.learner"
        };
        Ok(LegacyAccessPreview {
            membership: Membership {
                workspace: self.workspace,
                principal_id: binding.principal_id,
                role_refs: vec![role_ref.parse()?],
                status: MembershipStatus::Active,
            },
            deployment_grant: teacher.then_some(InstanceDeploymentGrant {
                authority_id: binding.authority_id,
                principal_id: binding.principal_id,
            }),
        })
    }
}

impl LegacyAccessPreview {
    pub fn membership(&self) -> &Membership {
        &self.membership
    }

    pub fn deployment_grant(&self) -> Option<&InstanceDeploymentGrant> {
        self.deployment_grant.as_ref()
    }

    /// Offline comparison only: the live policy must additionally verify authentication, current
    /// roles/revocation, resource existence and scope, and CSRF/password/OTP where appropriate.
    pub fn permits(
        &self,
        actor: PrincipalRef,
        action: LegacyAction,
        target: &LegacyResource,
    ) -> bool {
        if actor.authority_id != self.membership.workspace.authority_id
            || actor.principal_id != self.membership.principal_id
            || self.membership.status != MembershipStatus::Active
        {
            return false;
        }
        match (action, *target) {
            (
                LegacyAction::ReadPrivateArtifact,
                LegacyResource::PrivateArtifact {
                    workspace,
                    owner_id,
                },
            )
            | (
                LegacyAction::RestoreAttempt,
                LegacyResource::Attempt {
                    workspace,
                    owner_id,
                    ..
                },
            ) => workspace == self.membership.workspace && owner_id == actor.principal_id,
            (
                LegacyAction::ReviewStudentAttempt,
                LegacyResource::Attempt {
                    workspace,
                    owner_role,
                    ..
                },
            ) => {
                workspace == self.membership.workspace
                    && owner_role == AuthRole::Student
                    && self
                        .membership
                        .role_refs
                        .iter()
                        .any(|role| role.as_str() == "cyanrex.teaching.teacher")
            }
            (LegacyAction::ManageDeployment, LegacyResource::Deployment { authority_id }) => {
                self.deployment_grant.is_some_and(|grant| {
                    grant.authority_id == authority_id && grant.principal_id == actor.principal_id
                })
            }
            _ => false,
        }
    }
}
