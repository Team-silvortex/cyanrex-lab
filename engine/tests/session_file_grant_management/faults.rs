use super::{
    fixture::*,
    publisher_fixture::{grant, CHANGES, HEADS},
    support::*,
    *,
};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_file_grant_immediate_and_deferred_source_subject_ledger_faults_roll_back()
{
    for deferred in [false, true] {
        for mode in [
            "expired-session",
            "deleted-target",
            "target-kind",
            "target-policy",
            "actor-kind",
            "actor-policy",
            "foreign-ledger",
            "sql-error",
            "path",
        ] {
            let f = Fixture::ready().await;
            let target = active_target(&f).await;
            let old = issue(&f, 1, f.target()).await.unwrap();
            let (body,expected)=match mode {
            "expired-session"=>(format!("UPDATE {}.sessions SET expires_at=clock_timestamp()-INTERVAL '1 second' WHERE token='{}';",f.base.auth.base.schema,source_fixture::token_hash(&f.base.auth.learner_token)),Some(session_error(SessionCommandError::InvalidSession))),
            "deleted-target"=>(format!("DELETE FROM {}.users WHERE account_id='{}';",f.base.auth.base.schema,target.account_id()),Some(session_error(SessionCommandError::AccountMismatch))),
            "target-kind"=>(format!("UPDATE {}.collaboration_principals SET kind='agent' WHERE principal_id='{}';",f.base.auth.base.schema,target.principal().principal_id),Some(identity_error(IdentityStoreError::InvalidRecord))),
            "target-policy"=>(format!("UPDATE {}.collaboration_memberships SET role_refs=ARRAY['cyanrex.workspace.owner']::text[] WHERE principal_id='{}';",f.base.auth.base.schema,target.principal().principal_id),Some(identity_error(IdentityStoreError::InvalidRecord))),
            "actor-kind"=>(format!("UPDATE {}.collaboration_principals SET kind='agent' WHERE principal_id='{}';",f.base.auth.base.schema,f.manager().principal().principal_id),Some(identity_error(IdentityStoreError::InvalidRecord))),
            "actor-policy"=>(format!("UPDATE {}.collaboration_memberships SET role_refs=ARRAY['cyanrex.workspace.owner']::text[] WHERE principal_id='{}';",f.base.auth.base.schema,f.manager().principal().principal_id),Some(identity_error(IdentityStoreError::InvalidRecord))),
            "foreign-ledger"=>{let principal=Uuid::new_v4();let account=Uuid::new_v4();(format!("UPDATE {}.{HEADS} SET issuer_id='{principal}',issuer_account_id='{account}' WHERE grant_id='{}'; UPDATE {}.{CHANGES} SET actor_id='{principal}',actor_account_id='{account}' WHERE grant_id='{}' AND revision=1;",f.base.schema,grant(1),f.base.schema,grant(1)),Some(grant_error(FilePublisherGrantStoreError::InvalidRecord)))},
            "sql-error"=>("RAISE EXCEPTION 'synthetic management rejection';".into(),Some(if deferred{session_error(SessionCommandError::Authentication(DurableAuthError::StorageUnavailable))}else{grant_error(FilePublisherGrantStoreError::StorageUnavailable)})),
            _=>("PERFORM set_config('search_path','pg_catalog',true);".into(),None),
        };
            fault(
                &f,
                HEADS,
                "INSERT",
                &format!("{body} RETURN NEW;"),
                deferred,
            )
            .await;
            let before = f.base.snapshot().await;
            let result = f
                .base
                .auth
                .source
                .issue_session_file_publisher_grant(
                    &f.base.auth.learner_token,
                    &f.management,
                    grant(2),
                    target,
                    publisher_fixture::validity(),
                )
                .await;
            let after = f.base.snapshot().await;
            let reached = hit(&f).await;
            remove_fault(&f, HEADS).await;
            let healthy = issue(&f, 2, target).await;
            let retained = direct_get(&f, 1, f.target()).await;
            restored(&f).await;
            f.cleanup().await;
            assert!(reached, "{mode} deferred={deferred}");
            assert_eq!(after, before);
            if let Some(expected) = expected {
                assert_eq!(result, Err(expected), "{mode} deferred={deferred}");
            } else {
                assert!(result.is_err());
            }
            assert!(healthy.is_ok());
            assert_eq!(retained.unwrap(), Some(old));
        }
    }
    for revoke_case in [false, true] {
        for table in [HEADS, CHANGES] {
            let f = Fixture::ready().await;
            if revoke_case {
                issue(&f, 1, f.target()).await.unwrap();
            }
            let event = if revoke_case && table == HEADS {
                "UPDATE"
            } else {
                "INSERT"
            };
            fault(&f, table, event, "RETURN NULL;", false).await;
            let result = if revoke_case {
                revoke(&f, 1, revision(1)).await
            } else {
                issue(&f, 1, f.target()).await
            };
            let reached = hit(&f).await;
            remove_fault(&f, table).await;
            let healthy = if revoke_case {
                revoke(&f, 1, revision(1)).await
            } else {
                issue(&f, 1, f.target()).await
            };
            f.cleanup().await;
            assert!(reached);
            assert_eq!(
                result,
                Err(grant_error(FilePublisherGrantStoreError::InvalidRecord))
            );
            assert!(healthy.is_ok());
        }
    }
    // Revocation also rechecks the actor/session after a tentative history append.
    let f = Fixture::ready().await;
    issue(&f, 1, f.target()).await.unwrap();
    fault(
        &f,
        CHANGES,
        "INSERT",
        &format!(
            "DELETE FROM {}.sessions WHERE token='{}'; RETURN NEW;",
            f.base.auth.base.schema,
            source_fixture::token_hash(&f.base.auth.learner_token)
        ),
        true,
    )
    .await;
    let result = revoke(&f, 1, revision(1)).await;
    let reached = hit(&f).await;
    remove_fault(&f, CHANGES).await;
    let healthy = revoke(&f, 1, revision(1)).await;
    f.cleanup().await;
    assert!(reached);
    assert_eq!(
        result,
        Err(session_error(SessionCommandError::InvalidSession))
    );
    assert!(healthy.is_ok());
}
