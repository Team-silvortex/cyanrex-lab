use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_dispatch_owner_and_account_generation_are_rechecked_in_each_phase() {
    for phase_b in [false, true] {
        for column in ["owner_id", "account_id"] {
            let f = Fixture::ready().await;
            let mut attempt = f.journaled(ITEMS).await;
            let replacement = if column == "owner_id" {
                f.auth.manager.principal.reference.principal_id.as_uuid()
            } else {
                f.auth.target.account_id.as_uuid()
            };
            let mutation = format!(
                "UPDATE {}.collaboration_draft_intents SET {column} = '{replacement}';",
                f.journal_schema
            );
            if phase_b {
                // Phase A has already checked the original row. Its controlled deferred mutation
                // commits before phase B, which must not reuse that earlier ownership decision.
                f.step_fault("INSERT", &mutation, true).await;
            } else {
                f.sql(&mutation).await;
            }
            let result = attempt.advance(&f.auth.learner_token).await;
            f.auth.base.drain().await;
            let hit = !phase_b || f.step_hit().await;
            let steps = f.steps().await;
            let effects = counts(&f).await;
            let terminal = unknown(&attempt);
            let retry = attempt.advance(&f.auth.learner_token).await;
            f.cleanup().await;
            assert!(
                result.is_err() && hit && terminal,
                "phase_b={phase_b}, {column}"
            );
            assert_eq!(retry, Err(stopped()));
            assert_eq!(steps.len(), usize::from(phase_b));
            assert!(steps.iter().all(|step| !step.2));
            assert_eq!(effects, [0; 6]);
            // This isolates exact stored owner/account checks, not full account deletion/recreation.
        }
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_dispatch_current_session_logout_and_revocation_stop_each_phase() {
    for mode in [
        "logout_before_a",
        "membership_before_a",
        "logout_between_a_b",
    ] {
        for task in [false, true] {
            let f = Fixture::ready().await;
            let mut attempt = f.journaled(if task { &[] } else { ITEMS }).await;
            let phase_b = mode == "logout_between_a_b";
            match mode {
                "logout_before_a" => f.auth.source.logout(&f.auth.learner_token).await.unwrap(),
                "membership_before_a" => {
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
                }
                _ => {
                    // Test-owned deferred trigger removes the actual Session at A's COMMIT;
                    // B must authenticate again. This is not a public logout API in a trigger.
                    f.step_fault(
                        "INSERT",
                        &format!(
                            "DELETE FROM {}.sessions WHERE token = '{}';",
                            f.auth.base.schema,
                            source_fixture::token_hash(&f.auth.learner_token)
                        ),
                        true,
                    )
                    .await;
                }
            }
            let result = attempt.advance(&f.auth.learner_token).await;
            f.auth.base.drain().await;
            let hit = !phase_b || f.step_hit().await;
            let steps = f.steps().await;
            let effects = counts(&f).await;
            let terminal = unknown(&attempt);
            let retry = attempt.advance(&f.auth.learner_token).await;
            f.cleanup().await;
            assert!(result.is_err() && hit && terminal, "{mode}, task={task}");
            assert_eq!(retry, Err(stopped()));
            assert_eq!(steps.len(), usize::from(phase_b));
            assert!(steps.iter().all(|step| !step.2));
            assert_eq!(effects, [0; 6]);
        }
    }
}
