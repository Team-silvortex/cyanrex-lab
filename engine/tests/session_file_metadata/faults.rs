use super::{fixture::*, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_upload_trigger_drift_and_deferred_faults_roll_back_exactly() {
    for mode in [
        "foreign-ledger",
        "wrong-owner",
        "expired-session",
        "deferred-error",
        "path-shadow",
    ] {
        let f = Fixture::ready().await;
        let foreign = reserve(&f, &f.auth.manager_token, 1, &manifest(17, 1))
            .await
            .unwrap();
        let (body,deferred)=match mode {
            "foreign-ledger"=>(format!("UPDATE {}.{UPLOAD_ROWS} SET account_id='{}' WHERE upload_id='{}';",
                f.upload_schema,Uuid::new_v4(),upload(1)),true),
            "wrong-owner"=>(format!("NEW.owner_id='{}';",f.auth.manager.binding.principal_id),false),
            "expired-session"=>(format!("UPDATE {}.sessions SET expires_at=clock_timestamp()-INTERVAL '1 second' WHERE token='{}';",
                f.auth.base.schema,source_fixture::token_hash(&f.auth.learner_token)),true),
            "deferred-error"=>("RAISE EXCEPTION 'fixture deferred commit rejection';".into(),true),
            _=>(format!("PERFORM set_config('search_path','{}',true);",f.publication_schema),false),
        };
        fault(&f, &body, deferred).await;
        let before = snapshot(&f).await;
        let result = f
            .auth
            .source
            .reserve_session_file_upload(
                &f.auth.learner_token,
                &f.upload_workspace,
                upload(2),
                &manifest(17, 2),
            )
            .await;
        let reached = hit(&f).await;
        let after = snapshot(&f).await;
        remove_fault(&f).await;
        let healthy = reserve(&f, &f.auth.learner_token, 3, &manifest(17, 3)).await;
        let retained = get_upload(&f, &f.auth.manager_token, 1).await;
        f.pool_restored().await;
        f.cleanup().await;
        assert!(
            reached,
            "{mode} must reach the real source-owned INSERT/deferred flush"
        );
        assert_eq!(
            after, before,
            "{mode}: source/target legal mutation must roll back with the new row"
        );
        match mode {
            "foreign-ledger" | "wrong-owner" => assert_eq!(
                result,
                Err(SessionFileMetadataError::Upload(
                    FileUploadStoreError::InvalidRecord
                ))
            ),
            "expired-session" => assert_eq!(
                result,
                Err(SessionFileMetadataError::Session(
                    SessionCommandError::InvalidSession
                ))
            ),
            "deferred-error" => assert_eq!(
                result,
                Err(SessionFileMetadataError::Session(
                    SessionCommandError::Authentication(DurableAuthError::StorageUnavailable)
                ))
            ),
            _ => assert!(
                result.is_err(),
                "an INSERT trigger cannot select a shadow namespace"
            ),
        }
        assert!(healthy.is_ok());
        assert_eq!(retained.unwrap(), Some(foreign));
    }
}
