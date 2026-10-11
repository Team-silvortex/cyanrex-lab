use super::*;
use cyanrex_engine::services::collaboration_identity_store::{
    LegacyIdentityAction, LegacyIdentityCommand,
};

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_index_http_lifecycle_and_account_recreation_reauthorize() {
    for mode in ["logout", "expired", "suspended", "retired"] {
        let f = Fixture::ready().await;
        let route = app(&f, &index(&f));
        let task = f.create(vec![]).await;
        assert_page(
            &read(&f, &route, &f.auth.learner_token, COLLECTION).await,
            &[task.clone()],
            None,
        );
        let (status, code) = match mode {
            "logout" => {
                f.auth.source.logout(&f.auth.learner_token).await.unwrap();
                (StatusCode::UNAUTHORIZED, "invalid_session")
            }
            "expired" => {
                assert_eq!(query("UPDATE sessions SET expires_at = clock_timestamp() - INTERVAL '1 second' WHERE token = $1")
                    .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap().rows_affected(), 1);
                (StatusCode::UNAUTHORIZED, "invalid_session")
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
                (StatusCode::FORBIDDEN, "access_denied")
            }
            _ => {
                f.auth
                    .store
                    .apply_legacy_identity_command(&LegacyIdentityCommand {
                        command_id: id(),
                        actor: f.auth.manager.principal.reference,
                        workspace: scope(),
                        action: LegacyIdentityAction::Retire {
                            username: f.auth.learner.binding.username.clone(),
                            account_id: f.auth.learner.account_id,
                            expected_principal: f.auth.learner.binding.principal_id,
                        },
                    })
                    .await
                    .unwrap();
                (StatusCode::FORBIDDEN, "access_denied")
            }
        };
        denied(
            &f,
            &route,
            request_with_token("GET", COLLECTION, &f.auth.learner_token, Body::empty()),
            status,
            code,
            "unconfirmed",
        )
        .await;
        if mode == "retired" {
            let (token, owner) = f.replace_retired_learner().await;
            assert_ne!(owner, task.task.owner);
            assert_page(&read(&f, &route, &token, COLLECTION).await, &[], None);
            let replacement = f
                .request(&token, id(), manifest("Replacement owner's task", vec![]))
                .await
                .unwrap();
            assert_eq!(replacement.task.owner, owner);
            assert_page(
                &read(&f, &route, &token, COLLECTION).await,
                &[replacement],
                None,
            );
        }
        let before = index_sql_fixture::snapshot(&f).await;
        assert_eq!(
            f.tasks
                .get(task.task.reference, task.task.owner)
                .await
                .unwrap(),
            Some(task)
        );
        assert_eq!(index_sql_fixture::snapshot(&f).await, before);
        f.assert_pool_restored().await;
        drop(route);
        f.cleanup().await;
    }
}
