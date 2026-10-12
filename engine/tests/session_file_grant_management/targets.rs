use super::{fixture::*, support::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_file_issue_requires_current_audited_human_target_but_revoke_allows_retired_or_deleted_subject(
) {
    for mode in ["suspended", "retired", "deleted", "agent", "rawpolicy"] {
        let f = Fixture::ready().await;
        let target = active_target(&f).await;
        let old = issue(&f, 1, target).await.unwrap();
        let expected = match mode {
            "suspended" => {
                policy_status(
                    &f,
                    target.principal().principal_id,
                    MembershipStatus::Suspended,
                )
                .await;
                identity_error(IdentityStoreError::AccessDenied)
            }
            "retired" => {
                let bound = f
                    .base
                    .auth
                    .store
                    .lookup_legacy_account(
                        scope(),
                        &f.base.auth.target.username,
                        target.account_id(),
                    )
                    .await
                    .unwrap()
                    .unwrap();
                retire(&f, &bound).await;
                identity_error(IdentityStoreError::AccessDenied)
            }
            "deleted" => {
                query("DELETE FROM users WHERE account_id=$1")
                    .bind(target.account_id().as_uuid())
                    .execute(&f.base.auth.base.pool)
                    .await
                    .unwrap();
                session_error(SessionCommandError::AccountMismatch)
            }
            "agent" => {
                query("UPDATE collaboration_principals SET kind='agent' WHERE principal_id=$1")
                    .bind(target.principal().principal_id.as_uuid())
                    .execute(&f.base.auth.base.pool)
                    .await
                    .unwrap();
                identity_error(IdentityStoreError::InvalidRecord)
            }
            _ => {
                query("UPDATE collaboration_memberships SET role_refs=ARRAY['cyanrex.workspace.owner']::text[] WHERE principal_id=$1").bind(target.principal().principal_id.as_uuid()).execute(&f.base.auth.base.pool).await.unwrap();
                identity_error(IdentityStoreError::InvalidRecord)
            }
        };
        assert_eq!(
            issue(&f, 2, target).await,
            Err(expected),
            "{mode} cannot be treated as an active target"
        );
        if matches!(mode, "agent" | "rawpolicy") {
            let restore = if mode == "agent" {
                "UPDATE collaboration_principals SET kind='human' WHERE principal_id=$1"
            } else {
                "UPDATE collaboration_memberships SET role_refs=ARRAY[]::text[] WHERE principal_id=$1"
            };
            query(restore)
                .bind(target.principal().principal_id.as_uuid())
                .execute(&f.base.auth.base.pool)
                .await
                .unwrap();
            issue(&f, 2, target).await.unwrap();
        }
        let revoked = revoke(&f, 1, revision(1)).await.unwrap();
        assert_eq!(revoked.grant().subject(), target);
        assert_eq!(revoked.issuer(), old.issuer());
        assert_eq!(direct_get(&f, 1, target).await.unwrap(), Some(revoked));
        f.cleanup().await;
    }
    let f = Fixture::ready().await;
    let unbound = FileAccountRef::new(
        PrincipalRef {
            authority_id: authority(),
            principal_id: id(),
        },
        f.base.auth.target.account_id,
    );
    assert_eq!(
        issue(&f, 1, unbound).await,
        Err(identity_error(IdentityStoreError::StaleIdentity))
    );
    let bound = f.base.auth.bind_target().await;
    let target = FileAccountRef::new(bound.principal.reference, bound.account_id);
    assert_eq!(
        issue(&f, 1, target).await,
        Err(identity_error(IdentityStoreError::AccessDenied)),
        "binding does not imply active audited membership"
    );
    let target = active_target_from_bound(&f, &bound).await;
    let wrong = FileAccountRef::new(target.principal(), f.manager().account_id());
    assert_eq!(
        issue(&f, 1, wrong).await,
        Err(session_error(SessionCommandError::AccountMismatch))
    );
    let foreign = FileAccountRef::new(
        PrincipalRef {
            authority_id: id(),
            ..target.principal()
        },
        target.account_id(),
    );
    assert_eq!(
        issue(&f, 1, foreign).await,
        Err(session_error(SessionCommandError::AccountMismatch))
    );
    let self_grant = issue(&f, 1, f.manager()).await.unwrap();
    assert_eq!(self_grant.grant().subject(), f.manager());
    issue(&f, 2, target).await.unwrap();
    f.cleanup().await;
}
async fn active_target_from_bound(f: &Fixture, bound: &StoredLegacyIdentity) -> FileAccountRef {
    f.base
        .auth
        .source
        .apply_session_policy_command(
            &f.base.auth.manager_token,
            &SessionPolicyCommand {
                command_id: id(),
                expected_revision: None,
                desired: LegacyAccessPolicy {
                    membership: Membership {
                        workspace: scope(),
                        principal_id: bound.binding.principal_id,
                        role_refs: vec![],
                        status: MembershipStatus::Active,
                    },
                    deployment_granted: false,
                },
            },
        )
        .await
        .unwrap();
    FileAccountRef::new(bound.principal.reference, bound.account_id)
}
