use cyanrex_engine::{
    models::{auth::AuthRole, collaboration::*},
    services::legacy_workspace::{LegacyAction, LegacyResource, LegacyWorkspaceProjection},
};

const INSTANCE: &str = "71d5c1b0-5ccc-48ea-b3bb-11be648fa19a";
const OTHER_INSTANCE: &str = "c4190142-03af-4d04-b3fd-451dc551fe09";
const WORKSPACE: &str = "76099d60-96af-43c3-9f72-5674b18c21de";
const OTHER_WORKSPACE: &str = "f89b0612-25d5-46c5-a989-f4160c3c04c4";
const SUBJECT: &str = "407cc7a3-275e-402e-a239-6ed77c87c5d1";
const PEER: &str = "40554f64-f62e-4f82-b006-5c79f7886db8";

fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: INSTANCE.parse().unwrap(),
        workspace_id: WORKSPACE.parse().unwrap(),
    }
}

fn binding() -> LegacyIdentityBinding {
    LegacyIdentityBinding {
        authority_id: INSTANCE.parse().unwrap(),
        username: "synthetic-user".parse().unwrap(),
        principal_id: SUBJECT.parse().unwrap(),
    }
}

fn actor() -> PrincipalRef {
    PrincipalRef {
        authority_id: INSTANCE.parse().unwrap(),
        principal_id: SUBJECT.parse().unwrap(),
    }
}

#[test]
fn legacy_roles_map_to_scoped_memberships_and_explicit_deployment_grants() {
    let projection = LegacyWorkspaceProjection::new(scope());
    for role in [AuthRole::Admin, AuthRole::Teacher, AuthRole::Student] {
        let preview = projection.preview(&binding(), role).unwrap();
        let membership = preview.membership();
        assert_eq!(membership.workspace, scope());
        assert_eq!(membership.principal_id, actor().principal_id);
        assert_eq!(membership.status, MembershipStatus::Active);
        let teacher = role != AuthRole::Student;
        assert_eq!(
            membership.role_refs[0].as_str(),
            if teacher {
                "cyanrex.teaching.teacher"
            } else {
                "cyanrex.teaching.learner"
            }
        );
        assert_eq!(preview.deployment_grant().is_some(), teacher);
        if let Some(grant) = preview.deployment_grant() {
            assert_eq!(grant.authority_id, actor().authority_id);
            assert_eq!(grant.principal_id, actor().principal_id);
        }
        assert_eq!(
            preview.permits(
                actor(),
                LegacyAction::ManageDeployment,
                &LegacyResource::Deployment {
                    authority_id: scope().authority_id
                }
            ),
            teacher
        );
    }
}

#[test]
fn all_legacy_roles_keep_private_content_and_restore_bound_to_owner() {
    let projection = LegacyWorkspaceProjection::new(scope());
    for role in [AuthRole::Admin, AuthRole::Teacher, AuthRole::Student] {
        let preview = projection.preview(&binding(), role).unwrap();
        for owner in [SUBJECT, PEER] {
            let private = LegacyResource::PrivateArtifact {
                workspace: scope(),
                owner_id: owner.parse().unwrap(),
            };
            let attempt = LegacyResource::Attempt {
                workspace: scope(),
                owner_id: owner.parse().unwrap(),
                owner_role: AuthRole::Student,
            };
            assert_eq!(
                preview.permits(actor(), LegacyAction::ReadPrivateArtifact, &private),
                owner == SUBJECT
            );
            assert_eq!(
                preview.permits(actor(), LegacyAction::RestoreAttempt, &attempt),
                owner == SUBJECT
            );
        }
    }
}

#[test]
fn teacher_review_is_only_for_legacy_student_attempts_not_all_content() {
    let projection = LegacyWorkspaceProjection::new(scope());
    for role in [AuthRole::Admin, AuthRole::Teacher, AuthRole::Student] {
        let preview = projection.preview(&binding(), role).unwrap();
        for target_role in [AuthRole::Admin, AuthRole::Teacher, AuthRole::Student] {
            let target = LegacyResource::Attempt {
                workspace: scope(),
                owner_id: PEER.parse().unwrap(),
                owner_role: target_role,
            };
            assert_eq!(
                preview.permits(actor(), LegacyAction::ReviewStudentAttempt, &target),
                role != AuthRole::Student && target_role == AuthRole::Student
            );
        }
        let private = LegacyResource::PrivateArtifact {
            workspace: scope(),
            owner_id: PEER.parse().unwrap(),
        };
        assert!(!preview.permits(actor(), LegacyAction::ReviewStudentAttempt, &private));
    }
}

#[test]
fn grants_cannot_be_replayed_by_another_principal_or_another_instance() {
    let preview = LegacyWorkspaceProjection::new(scope())
        .preview(&binding(), AuthRole::Teacher)
        .unwrap();
    let targets = [
        (
            LegacyAction::ReadPrivateArtifact,
            LegacyResource::PrivateArtifact {
                workspace: scope(),
                owner_id: SUBJECT.parse().unwrap(),
            },
        ),
        (
            LegacyAction::RestoreAttempt,
            LegacyResource::Attempt {
                workspace: scope(),
                owner_id: SUBJECT.parse().unwrap(),
                owner_role: AuthRole::Teacher,
            },
        ),
        (
            LegacyAction::ReviewStudentAttempt,
            LegacyResource::Attempt {
                workspace: scope(),
                owner_id: PEER.parse().unwrap(),
                owner_role: AuthRole::Student,
            },
        ),
        (
            LegacyAction::ManageDeployment,
            LegacyResource::Deployment {
                authority_id: scope().authority_id,
            },
        ),
    ];
    for (action, target) in targets {
        assert!(preview.permits(actor(), action, &target));
        let mut impostor = actor();
        impostor.principal_id = PEER.parse().unwrap();
        assert!(!preview.permits(impostor, action, &target));
        impostor = actor();
        impostor.authority_id = OTHER_INSTANCE.parse().unwrap();
        assert!(!preview.permits(impostor, action, &target));
    }
}

#[test]
fn same_ids_in_another_workspace_or_instance_confer_no_access() {
    let preview = LegacyWorkspaceProjection::new(scope())
        .preview(&binding(), AuthRole::Admin)
        .unwrap();
    for foreign in [
        WorkspaceRef {
            authority_id: OTHER_INSTANCE.parse().unwrap(),
            ..scope()
        },
        WorkspaceRef {
            workspace_id: OTHER_WORKSPACE.parse().unwrap(),
            ..scope()
        },
    ] {
        let private = LegacyResource::PrivateArtifact {
            workspace: foreign,
            owner_id: SUBJECT.parse().unwrap(),
        };
        let attempt = LegacyResource::Attempt {
            workspace: foreign,
            owner_id: SUBJECT.parse().unwrap(),
            owner_role: AuthRole::Student,
        };
        assert!(!preview.permits(actor(), LegacyAction::ReadPrivateArtifact, &private));
        assert!(!preview.permits(actor(), LegacyAction::RestoreAttempt, &attempt));
        assert!(!preview.permits(actor(), LegacyAction::ReviewStudentAttempt, &attempt));
    }
    assert!(!preview.permits(
        actor(),
        LegacyAction::ManageDeployment,
        &LegacyResource::Deployment {
            authority_id: OTHER_INSTANCE.parse().unwrap()
        }
    ));
}

#[test]
fn incompatible_action_resource_pairs_fail_closed() {
    let preview = LegacyWorkspaceProjection::new(scope())
        .preview(&binding(), AuthRole::Teacher)
        .unwrap();
    let private = LegacyResource::PrivateArtifact {
        workspace: scope(),
        owner_id: SUBJECT.parse().unwrap(),
    };
    let deployment = LegacyResource::Deployment {
        authority_id: scope().authority_id,
    };
    assert!(!preview.permits(actor(), LegacyAction::ManageDeployment, &private));
    assert!(!preview.permits(actor(), LegacyAction::RestoreAttempt, &private));
    assert!(!preview.permits(actor(), LegacyAction::ReadPrivateArtifact, &deployment));
    assert!(!preview.permits(actor(), LegacyAction::ReviewStudentAttempt, &deployment));
}

#[test]
fn preview_reuses_supplied_ids_and_rejects_foreign_identity_bindings() {
    let projection = LegacyWorkspaceProjection::new(scope());
    let first = projection.preview(&binding(), AuthRole::Teacher).unwrap();
    let repeated = projection.preview(&binding(), AuthRole::Teacher).unwrap();
    assert_eq!(first, repeated);
    let mut foreign = binding();
    foreign.authority_id = OTHER_INSTANCE.parse().unwrap();
    assert!(projection.preview(&foreign, AuthRole::Teacher).is_err());
}
