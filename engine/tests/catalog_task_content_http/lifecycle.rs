use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_http_current_sessions_membership_and_recreated_owner_never_inherit_content(
) {
    for mode in [
        "invalid",
        "unbound",
        "logout",
        "expired",
        "suspended",
        "retired",
    ] {
        let (f, workspace, route, task, old, _) = pair().await;
        let token = match mode {
            "invalid" => TOKEN.to_string(),
            "unbound" => f.auth.target_token.clone(),
            "logout" => {
                f.auth.source.logout(&f.auth.learner_token).await.unwrap();
                f.auth.learner_token.clone()
            }
            "expired" => {
                query("UPDATE sessions SET expires_at = clock_timestamp() - INTERVAL '1 second' WHERE token = $1").bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
                f.auth.learner_token.clone()
            }
            "suspended" => {
                let current = f.auth.current(f.auth.learner.binding.principal_id).await;
                let mut desired = current.policy;
                desired.membership.status = MembershipStatus::Suspended;
                f.auth
                    .source
                    .apply_session_policy_command(
                        &f.auth.manager_token,
                        &SessionPolicyCommand {
                            command_id: id(),
                            expected_revision: Some(current.revision),
                            desired,
                        },
                    )
                    .await
                    .unwrap();
                f.auth.learner_token.clone()
            }
            "retired" => {
                let learner = &f.auth.learner;
                f.auth
                    .store
                    .apply_legacy_identity_command(&LegacyIdentityCommand {
                        command_id: id(),
                        actor: f.auth.manager.principal.reference,
                        workspace: scope(),
                        action: LegacyIdentityAction::Retire {
                            username: learner.binding.username.clone(),
                            account_id: learner.account_id,
                            expected_principal: learner.binding.principal_id,
                        },
                    })
                    .await
                    .unwrap();
                f.auth.learner_token.clone()
            }
            _ => unreachable!(),
        };
        let (status, code) = if matches!(mode, "unbound" | "suspended" | "retired") {
            (StatusCode::FORBIDDEN, "access_denied")
        } else {
            (StatusCode::UNAUTHORIZED, "invalid_session")
        };
        for req in saved_requests(&task, &token).into_iter().chain(
            mutations(
                &task,
                &token,
                &manifest("Denied create", vec![]),
                &values()[0],
            )
            .into_iter()
            .take(1),
        ) {
            denied(&f, &route, req, status, code).await;
        }
        if mode == "unbound" {
            f.auth.bind_target().await;
            for req in saved_requests(&task, &token) {
                denied(&f, &route, req, StatusCode::FORBIDDEN, "access_denied").await;
            }
        }
        if mode == "retired" {
            let (replacement, principal) = f.replace_retired_learner().await;
            assert_ne!(principal, task.task.owner);
            for req in saved_requests(&task, &replacement) {
                denied(&f, &route, req, StatusCode::NOT_FOUND, "not_found").await;
            }
            denied(
                &f,
                &route,
                request(
                    "POST",
                    COLLECTION,
                    &replacement,
                    create_body(
                        id(),
                        &values()[0].reference,
                        &manifest("Cannot inherit old owner bytes", vec![old.reference]),
                    ),
                ),
                StatusCode::NOT_FOUND,
                "not_found",
            )
            .await;
        }
        f.assert_pool_restored().await;
        drop((route, workspace));
        f.cleanup().await;
    }
}
