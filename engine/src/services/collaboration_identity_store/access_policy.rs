use super::*;
use crate::services::legacy_workspace::LegacyAction;

/// Trusted adapter evidence about an existing resource, never a browser ownership claim.
/// Owner roles are deliberately absent: the preview resolves current roles from storage.
#[derive(Debug, Clone, Copy)]
pub enum LegacyPolicyResource {
    PrivateArtifact {
        workspace: WorkspaceRef,
        owner_id: PrincipalId,
    },
    Attempt {
        workspace: WorkspaceRef,
        owner_id: PrincipalId,
    },
    Deployment {
        authority_id: AuthorityId,
    },
}

impl CollaborationIdentityStore {
    pub(super) async fn active_legacy_principal(
        connection: &mut PgConnection,
        scope: WorkspaceRef,
        principal: PrincipalId,
    ) -> Result<bool> {
        Ok(Self::bound_principal(connection, scope, principal)
            .await?
            .is_some_and(|identity| {
                identity.retired_at.is_none()
                    && identity.principal.status == PrincipalStatus::Active
            }))
    }

    /// Fresh durable policy preview only. Does not authenticate, verify resource existence,
    /// authorize mutations of policy, or reserve permission for an action after this transaction.
    /// The future live adapter must combine policy and the protected operation in one reviewed
    /// authority path, retaining session/CSRF/TOTP and revocation semantics.
    pub async fn preview_legacy_access(
        &self,
        scope: WorkspaceRef,
        actor: PrincipalRef,
        action: LegacyAction,
        resource: &LegacyPolicyResource,
    ) -> Result<bool> {
        bounded(async {
            let mut tx = self.transaction().await?;
            let workspace = Self::access_scope(&mut tx, scope, false).await?;
            let allowed = if actor.authority_id != scope.authority_id
                || !Self::active_legacy_principal(&mut tx, scope, actor.principal_id).await?
            {
                false
            } else if let Some(access) =
                Self::access_record(&mut tx, scope, actor.principal_id).await?
            {
                match (action, *resource) {
                    (
                        LegacyAction::ManageDeployment,
                        LegacyPolicyResource::Deployment { authority_id },
                    ) => authority_id == scope.authority_id && access.policy.deployment_granted,
                    _ if workspace.status != WorkspaceStatus::Active
                        || access.policy.membership.status != MembershipStatus::Active =>
                    {
                        false
                    }
                    (
                        LegacyAction::ReadPrivateArtifact,
                        LegacyPolicyResource::PrivateArtifact {
                            workspace,
                            owner_id,
                        },
                    )
                    | (
                        LegacyAction::RestoreAttempt,
                        LegacyPolicyResource::Attempt {
                            workspace,
                            owner_id,
                        },
                    ) => workspace == scope && owner_id == actor.principal_id,
                    (
                        LegacyAction::ReviewStudentAttempt,
                        LegacyPolicyResource::Attempt {
                            workspace,
                            owner_id,
                        },
                    ) => {
                        if workspace != scope
                            || !access.policy.has_role("cyanrex.teaching.teacher")
                            || !Self::active_legacy_principal(&mut tx, scope, owner_id).await?
                        {
                            false
                        } else {
                            Self::access_record(&mut tx, scope, owner_id)
                                .await?
                                .is_some_and(|owner| {
                                    owner.policy.membership.status == MembershipStatus::Active
                                        && owner.policy.has_role("cyanrex.teaching.learner")
                                })
                        }
                    }
                    _ => false,
                }
            } else {
                false
            };
            tx.commit().await?;
            Ok(allowed)
        })
        .await
    }
}
