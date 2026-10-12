use super::{fixture::*, support::*, *};
use cyanrex_engine::services::file_upload_publication::FileUploadPublicationStoreError;

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_file_publication_immediate_and_deferred_final_authority_or_ledger_faults_roll_back(
) {
    for deferred in [false, true] {
        for mode in [
            "session",
            "grant",
            "actor",
            "foreign-ledger",
            "sql-error",
            "suppressed",
            "sync-probe",
            "extra-path",
        ] {
            if deferred && mode == "suppressed" {
                continue;
            }
            let f = Fixture::ready(17).await;
            f.grants.issue(1, f.actor()).await;
            healthy(&f).await;
            let manifest = f.stage_additional(2, 17, f.actor()).await;
            let mut a = prepare(&f, 2).await;
            let body=match mode {
                "session"=>format!("UPDATE {}.sessions SET expires_at=clock_timestamp()-INTERVAL '1 second' WHERE token='{}'; RETURN NEW;",f.grants.auth.base.schema,source_fixture::token_hash(&f.grants.auth.learner_token)),
                "grant"=>format!("UPDATE {}.{} SET issuer_account_id='{}' WHERE grant_id='{}'; RETURN NEW;",f.grants.schema,grant_fixture::HEADS,Uuid::new_v4(),grant_fixture::grant(1)),
                "actor"=>format!("UPDATE {}.collaboration_principals SET kind='agent' WHERE principal_id='{}'; RETURN NEW;",f.grants.auth.base.schema,f.actor().principal().principal_id),
                "foreign-ledger"=>format!("UPDATE {}.{PUBLICATIONS} SET nonce='{}' WHERE upload_id='{}'; RETURN NEW;",f.publication_schema,Uuid::new_v4(),upload(1)),
                "sql-error"=>"RAISE EXCEPTION 'synthetic publication final rejection';".into(),
                "suppressed"=>"RETURN NULL;".into(),
                "extra-path"=>format!("PERFORM set_config('search_path','{},nonexistent_publication_shadow',true); RETURN NEW;",f.publication_schema),
                _=>"IF current_setting('synchronous_commit')<>'on' THEN RAISE EXCEPTION 'publication flush must be synchronous'; END IF; PERFORM set_config('synchronous_commit','off',true); RETURN NEW;".into(),
            };
            fault(&f, &body, deferred).await;
            let before = f.snapshot().await;
            let result = publish(&f, &mut a).await;
            let state = a.state();
            let after = f.snapshot().await;
            let reached = hit(&f).await;
            remove_fault(&f).await;
            let (healthy_result, expected) = if mode == "sync-probe" {
                if result.is_ok() {
                    delta(&f, &before, &after, 2);
                }
                let row = f
                    .publication
                    .get(upload(2), f.actor().principal(), f.actor().account_id())
                    .await
                    .unwrap();
                (row.is_some(), None)
            } else {
                let expected = match mode {
                    "session" => session_error(SessionCommandError::InvalidSession),
                    "grant" => grant_error(FilePublisherGrantStoreError::InvalidRecord),
                    "actor" => identity_error(IdentityStoreError::InvalidRecord),
                    "foreign-ledger" | "suppressed" => {
                        publisher_error(FileUploadPublicationError::Publication(
                            FileUploadPublicationStoreError::InvalidRecord,
                        ))
                    }
                    "extra-path" => publisher_error(FileUploadPublicationError::Publication(
                        FileUploadPublicationStoreError::UnsupportedSchema,
                    )),
                    _ => {
                        if deferred {
                            session_error(SessionCommandError::Authentication(
                                DurableAuthError::StorageUnavailable,
                            ))
                        } else {
                            publisher_error(FileUploadPublicationError::Unconfirmed)
                        }
                    }
                };
                let mut fresh = prepare(&f, 2).await;
                let healthy = publish(&f, &mut fresh).await;
                let good = healthy.is_ok();
                if let Ok(r) = healthy {
                    receipt(&r, 2, f.actor(), &manifest);
                }
                (good, Some(expected))
            };
            let retained = f
                .publication
                .get(upload(1), f.actor().principal(), f.actor().account_id())
                .await
                .unwrap();
            f.cleanup().await;
            assert!(reached, "actual final trigger {mode} deferred={deferred}");
            assert!(healthy_result);
            assert!(retained.is_some());
            if let Some(expected) = expected {
                assert_eq!(result.err(), Some(expected), "{mode} deferred={deferred}");
                assert_eq!(state, FileUploadPublicationState::Unknown);
                assert_eq!(after,before,"final source/grant/ledger failure must roll back the INSERT and every injected mutation");
            } else {
                assert!(result.is_ok());
                assert_eq!(state, FileUploadPublicationState::Confirmed);
            }
        }
    }
}
