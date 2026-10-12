use super::support::*;
use crate::{fixture::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_sharing_immediate_and_deferred_authority_and_record_faults_roll_back() {
    for mode in [
        "head_suppressed",
        "history_suppressed",
        "session_immediate",
        "session_deferred",
        "target",
        "actor",
        "publication",
        "publisher",
        "foreign_ledger",
        "revoke_session",
    ] {
        checked(Fixture::ready().await,|f|Box::pin(async move{
            let is_revoke=mode=="revoke_session";if is_revoke||mode=="foreign_ledger"{issue(f,1).await.unwrap();}
            let(table,event,body,deferred,expected)=match mode{
                "head_suppressed"=>(HEADS,"INSERT","RETURN NULL;".into(),false,recipient(FileRecipientGrantStoreError::InvalidRecord)),
                "history_suppressed"=>(CHANGES,"INSERT","RETURN NULL;".into(),false,recipient(FileRecipientGrantStoreError::InvalidRecord)),
                "session_immediate"|"session_deferred"|"revoke_session"=>(if is_revoke{CHANGES}else{HEADS},"INSERT",format!("DELETE FROM {}.sessions WHERE token='{}'; RETURN NEW;",f.base.grants.auth.base.schema,source_fixture::token_hash(&f.base.grants.auth.learner_token)),mode!="session_immediate",session(SessionCommandError::InvalidSession)),
                "target"=>(CHANGES,"INSERT",format!("DELETE FROM {}.users WHERE account_id='{}'; RETURN NEW;",f.base.grants.auth.base.schema,f.recipient.account_id()),true,session(SessionCommandError::AccountMismatch)),
                "actor"=>(CHANGES,"INSERT",format!("UPDATE {}.collaboration_memberships SET status='suspended' WHERE principal_id='{}'; RETURN NEW;",f.base.grants.auth.base.schema,f.actor().principal().principal_id),true,identity(IdentityStoreError::InvalidRecord)),
                "publication"=>(CHANGES,"INSERT",format!("UPDATE {}.{PUB_ROWS} SET nonce='{}' WHERE upload_id='{}'; RETURN NEW;",f.base.publication_schema,Uuid::new_v4(),publication_fixture::upload(1)),true,publication(FileUploadPublicationStoreError::InvalidRecord)),
                "publisher"=>(CHANGES,"INSERT",format!("UPDATE {}.{} SET recorded_at=recorded_at-1 WHERE grant_id='{}'; RETURN NEW;",f.base.grants.schema,grant_fixture::CHANGES,grant_fixture::grant(1)),true,publisher(FilePublisherGrantStoreError::InvalidRecord)),
                _=>{
                    let mut file=serde_json::to_value(&f.expected_file).unwrap();file["version_id"]=serde_json::json!(Uuid::new_v4());let file:FileVersionRef=serde_json::from_value(file).unwrap();let g=FileRecipientGrant::new(recipient_grant(1),revision(1),file,f.actor(),f.recipient,grant_fixture::validity(),FileGrantState::Active).unwrap();let hex=serde_json::to_vec(&g).unwrap().iter().map(|b|format!("{b:02x}")).collect::<String>();
                    (CHANGES,"INSERT",format!("IF NEW.grant_id='{}'::uuid THEN UPDATE {}.{HEADS} SET grant_payload=decode('{hex}','hex') WHERE grant_id='{}'; UPDATE {}.{CHANGES} SET grant_payload=decode('{hex}','hex') WHERE grant_id='{}'; END IF; RETURN NEW;",recipient_grant(2),f.schema,recipient_grant(1),f.schema,recipient_grant(1)),true,recipient(FileRecipientGrantStoreError::InvalidRecord))
                }
            };
            fault(f,table,event,&body,deferred).await;let before=f.snapshot().await;let result=if is_revoke{revoke(f,1,revision(1)).await}else{issue(f,if mode=="foreign_ledger"{2}else{1}).await};let reached=hit(f).await;assert_eq!(f.snapshot().await,before);remove_fault(f,table).await;let healthy=if is_revoke{revoke(f,1,revision(1)).await}else{issue(f,if mode=="foreign_ledger"{2}else{1}).await};assert!(healthy.is_ok());assert!(reached,"actual {mode} trigger must be reached");assert_eq!(result,Err(expected),"{mode}");
        })).await;
    }
    checked(Fixture::ready().await,|f|Box::pin(async move{
        // Observe sync policy at deferred flush, not power loss/fsync or COMMIT acknowledgement.
        fault(f,CHANGES,"INSERT","IF current_setting('synchronous_commit') <> 'on' THEN RAISE EXCEPTION 'synthetic sync probe'; END IF; RETURN NEW;",true).await;configure(f,false).await;issue(f,1).await.unwrap();assert!(hit(f).await);settings(f,false).await;remove_fault(f,CHANGES).await;
    })).await;
}
