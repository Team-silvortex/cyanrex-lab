use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_access_privacy_review_and_deployment_are_distinct() {
    let f = ready().await;
    let teacher = member(&f, "teacher-one", "cyanrex.teaching.teacher", true)
        .await
        .policy
        .membership
        .principal_id;
    let student = member(&f, "student-one", "cyanrex.teaching.learner", false)
        .await
        .policy
        .membership
        .principal_id;
    let other = member(&f, "student-two", "cyanrex.teaching.learner", false)
        .await
        .policy
        .membership
        .principal_id;
    let owner = member(&f, "workspace-owner", "cyanrex.workspace.owner", false)
        .await
        .policy
        .membership
        .principal_id;
    let teacher_without_grant = member(&f, "teacher-two", "cyanrex.teaching.teacher", false)
        .await
        .policy
        .membership
        .principal_id;
    for who in [teacher, student, other, owner, teacher_without_grant] {
        for target in [teacher, student, other, owner] {
            let private = LegacyPolicyResource::PrivateArtifact {
                workspace: scope(),
                owner_id: target,
            };
            let attempt = LegacyPolicyResource::Attempt {
                workspace: scope(),
                owner_id: target,
            };
            assert_eq!(
                f.store
                    .preview_legacy_access(
                        scope(),
                        actor(who),
                        LegacyAction::ReadPrivateArtifact,
                        &private
                    )
                    .await
                    .unwrap(),
                who == target
            );
            assert_eq!(
                f.store
                    .preview_legacy_access(
                        scope(),
                        actor(who),
                        LegacyAction::RestoreAttempt,
                        &attempt
                    )
                    .await
                    .unwrap(),
                who == target
            );
            assert_eq!(
                f.store
                    .preview_legacy_access(
                        scope(),
                        actor(who),
                        LegacyAction::ReviewStudentAttempt,
                        &attempt
                    )
                    .await
                    .unwrap(),
                [teacher, teacher_without_grant].contains(&who)
                    && [student, other].contains(&target)
            );
            assert!(!f
                .store
                .preview_legacy_access(
                    scope(),
                    actor(who),
                    LegacyAction::ManageDeployment,
                    &private
                )
                .await
                .unwrap());
        }
        assert_eq!(
            f.store
                .preview_legacy_access(
                    scope(),
                    actor(who),
                    LegacyAction::ManageDeployment,
                    &deployment()
                )
                .await
                .unwrap(),
            who == teacher
        );
    }
    // Review also checks the target's current membership, not just the teacher's role.
    let current = f
        .store
        .legacy_access(scope(), student)
        .await
        .unwrap()
        .unwrap();
    let mut paused = current.policy.clone();
    paused.membership.status = MembershipStatus::Suspended;
    let suspended = f
        .store
        .replace_legacy_access(Some(current.revision), &paused)
        .await
        .unwrap();
    assert!(!f
        .store
        .preview_legacy_access(
            scope(),
            actor(teacher),
            LegacyAction::ReviewStudentAttempt,
            &LegacyPolicyResource::Attempt {
                workspace: scope(),
                owner_id: student
            }
        )
        .await
        .unwrap());
    f.store
        .replace_legacy_access(Some(suspended.revision), &current.policy)
        .await
        .unwrap();
    // A role change is read from storage, not a caller-provided "student" claim.
    let current = f
        .store
        .legacy_access(scope(), student)
        .await
        .unwrap()
        .unwrap();
    f.store
        .replace_legacy_access(
            Some(current.revision),
            &policy(student, "cyanrex.teaching.teacher", false),
        )
        .await
        .unwrap();
    assert!(!f
        .store
        .preview_legacy_access(
            scope(),
            actor(teacher),
            LegacyAction::ReviewStudentAttempt,
            &LegacyPolicyResource::Attempt {
                workspace: scope(),
                owner_id: student
            }
        )
        .await
        .unwrap());
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_access_suspension_archival_and_explicit_revocation_take_effect() {
    let f = ready().await;
    let original = member(&f, "teacher-one", "cyanrex.teaching.teacher", true).await;
    let principal = original.policy.membership.principal_id;
    let private = LegacyPolicyResource::PrivateArtifact {
        workspace: scope(),
        owner_id: principal,
    };
    // Archival denies workspace content even while membership itself is still active.
    f.sql("UPDATE collaboration_workspaces SET status = 'archived'")
        .await;
    assert!(!f
        .store
        .preview_legacy_access(
            scope(),
            actor(principal),
            LegacyAction::ReadPrivateArtifact,
            &private
        )
        .await
        .unwrap());
    assert!(f
        .store
        .preview_legacy_access(
            scope(),
            actor(principal),
            LegacyAction::ManageDeployment,
            &deployment()
        )
        .await
        .unwrap());
    f.sql("UPDATE collaboration_workspaces SET status = 'active'")
        .await;
    let mut suspended = original.policy.clone();
    suspended.membership.status = MembershipStatus::Suspended;
    let changed = f
        .store
        .replace_legacy_access(Some(original.revision), &suspended)
        .await
        .unwrap();
    assert!(!f
        .store
        .preview_legacy_access(
            scope(),
            actor(principal),
            LegacyAction::ReadPrivateArtifact,
            &private
        )
        .await
        .unwrap());
    // Instance deployment grants are independent of workspace membership/archival.
    assert!(f
        .store
        .preview_legacy_access(
            scope(),
            actor(principal),
            LegacyAction::ManageDeployment,
            &deployment()
        )
        .await
        .unwrap());
    f.sql("UPDATE collaboration_workspaces SET status = 'archived'")
        .await;
    assert_eq!(
        f.store
            .replace_legacy_access(Some(changed.revision), &original.policy)
            .await,
        Err(IdentityStoreError::ScopeInactive)
    );
    assert!(!f
        .store
        .preview_legacy_access(
            scope(),
            actor(principal),
            LegacyAction::ReadPrivateArtifact,
            &private
        )
        .await
        .unwrap());
    suspended.deployment_granted = false;
    f.store
        .replace_legacy_access(Some(changed.revision), &suspended)
        .await
        .unwrap();
    assert!(!f
        .store
        .preview_legacy_access(
            scope(),
            actor(principal),
            LegacyAction::ManageDeployment,
            &deployment()
        )
        .await
        .unwrap());
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_access_retirement_and_same_name_recreation_do_not_inherit_grants() {
    let f = ready().await;
    let identity = f.bind().await;
    let principal = identity.binding.principal_id;
    let original = f
        .store
        .replace_legacy_access(None, &policy(principal, "cyanrex.teaching.teacher", true))
        .await
        .unwrap();
    f.store
        .retire_legacy_account(scope(), &username(), account(), principal)
        .await
        .unwrap();
    assert!(!f
        .store
        .preview_legacy_access(
            scope(),
            actor(principal),
            LegacyAction::ManageDeployment,
            &deployment()
        )
        .await
        .unwrap());
    assert_eq!(
        f.store
            .replace_legacy_access(Some(original.revision), &original.policy)
            .await,
        Err(IdentityStoreError::IdentityRetired)
    );
    let recreated = f
        .store
        .bind_legacy_account(scope(), &username(), id())
        .await
        .unwrap()
        .binding
        .principal_id;
    assert_ne!(recreated, principal);
    assert_eq!(
        f.store.legacy_access(scope(), recreated).await.unwrap(),
        None
    );
    assert!(!f
        .store
        .preview_legacy_access(
            scope(),
            actor(recreated),
            LegacyAction::ManageDeployment,
            &deployment()
        )
        .await
        .unwrap());
    let mut revoked = original.policy.clone();
    revoked.membership.status = MembershipStatus::Suspended;
    revoked.deployment_granted = false;
    f.store
        .replace_legacy_access(Some(original.revision), &revoked)
        .await
        .unwrap();
    assert_eq!(f.count("collaboration_memberships").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_access_unknown_duplicate_or_ambiguous_roles_are_rejected() {
    let f = ready().await;
    let principal = f.bind().await.binding.principal_id;
    for roles in [
        vec!["cyanrex.unknown.owner"],
        vec!["cyanrex.teaching.teacher", "cyanrex.teaching.teacher"],
        vec!["cyanrex.teaching.teacher", "cyanrex.teaching.learner"],
    ] {
        let mut proposed = policy(principal, "cyanrex.workspace.owner", false);
        proposed.membership.role_refs = roles.iter().map(|role| role.parse().unwrap()).collect();
        assert_eq!(
            f.store.replace_legacy_access(None, &proposed).await,
            Err(IdentityStoreError::InvalidRecord)
        );
    }
    assert_eq!(f.count("collaboration_memberships").await, 0);
    let mut proposed = policy(principal, "cyanrex.workspace.owner", false);
    proposed
        .membership
        .role_refs
        .push("cyanrex.teaching.learner".parse().unwrap());
    let stored = f
        .store
        .replace_legacy_access(None, &proposed)
        .await
        .unwrap();
    proposed.membership.role_refs.reverse();
    assert_eq!(
        f.store
            .replace_legacy_access(Some(stored.revision), &proposed)
            .await
            .unwrap(),
        stored
    );
    f.cleanup().await;
}
